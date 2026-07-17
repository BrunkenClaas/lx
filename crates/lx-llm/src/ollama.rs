use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::{ImageData, LlmClient, LlmError, Request, Response};

/// Native Ollama client, speaking the `/api/chat` endpoint.
///
/// **Why this exists separately from [`OpenAiClient`](crate::openai::OpenAiClient).**
/// Ollama also exposes an OpenAI-compatible `/v1/chat/completions` endpoint, but
/// that layer **silently ignores `num_ctx`** and clamps the context window to
/// Ollama's small default (~2048 tokens), truncating any larger prompt and
/// causing malformed / prose output. Ollama only honours `num_ctx` on its native
/// `/api/chat` endpoint, under the `options` object. Since a working context
/// window is essential for the suite's larger inputs (diffs, logs), the Ollama
/// provider talks native `/api/chat` instead of `/v1`.
///
/// This is the one deliberate break from the "uniform request body across all
/// providers" principle (see design doc §7.3.1). It is justified as a bug fix,
/// not a feature: `num_ctx` is a runtime context-window parameter, **not**
/// constrained decoding — the no-constrained-decoding decision is unaffected.
/// Every other OpenAI-compatible local backend (LM Studio, llama.cpp, vLLM)
/// stays on `OpenAiClient`.
pub struct OllamaClient {
    /// Endpoint URL, e.g. `http://localhost:11434/api/chat`. Derived from the
    /// configured base URL by stripping a trailing `/v1` and appending `/api/chat`.
    url: String,
    model: String,
    timeout: Duration,
    max_retries: u32,
    verbose: bool,
    /// Context window sent as `options.num_ctx` — the whole reason this client
    /// exists. Ollama truncates silently to ~2048 without it on large prompts.
    num_ctx: u32,
    /// Global output-token ceiling from config (`limits.max_output_tokens`),
    /// applied as `min(per-tool max_tokens, ceiling)` and sent as
    /// `options.num_predict`.
    max_output_ceiling: u32,
}

impl OllamaClient {
    pub fn new(
        base_url: String,
        model: String,
        timeout_secs: u64,
        max_retries: u32,
        verbose: bool,
        num_ctx: u32,
        max_output_ceiling: u32,
    ) -> Self {
        OllamaClient {
            url: Self::chat_url(&base_url),
            model,
            timeout: Duration::from_secs(timeout_secs),
            max_retries,
            verbose,
            num_ctx,
            max_output_ceiling,
        }
    }

    /// Derive the native `/api/chat` URL from a configured base URL.
    ///
    /// The provider default base URL is `http://localhost:11434/v1` (the
    /// OpenAI-compat path). Ollama's native API lives at the host root under
    /// `/api/chat`, so strip a trailing `/v1` (and any trailing slash) before
    /// appending. A custom base URL that already points at `/api` is left as-is
    /// apart from the `/chat` suffix.
    fn chat_url(base_url: &str) -> String {
        let trimmed = base_url.trim_end_matches('/');
        let root = trimmed.strip_suffix("/v1").unwrap_or(trimmed);
        let root = root.trim_end_matches('/');
        format!("{root}/api/chat")
    }

    /// Effective output cap: the smaller of the per-tool `max_tokens` and the
    /// configured global ceiling. Mirrors `OpenAiClient::effective_max_tokens`.
    fn effective_max_tokens(&self, requested: u32) -> u32 {
        requested.min(self.max_output_ceiling)
    }
}

// ── Wire types ────────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<Message>,
    /// Always `false` — the tools make a single blocking call and want the whole
    /// response at once.
    stream: bool,
    options: Options,
}

#[derive(Serialize)]
struct Message {
    role: &'static str,
    content: String,
    /// Base64 image data (no `data:` URI prefix), Ollama's native multimodal
    /// format. Omitted entirely for text-only messages.
    #[serde(skip_serializing_if = "Option::is_none")]
    images: Option<Vec<String>>,
}

#[derive(Serialize)]
struct Options {
    /// The context window — the field Ollama's `/v1` endpoint ignores.
    num_ctx: u32,
    /// Output-token limit (Ollama's name for `max_tokens`).
    num_predict: u32,
    temperature: f32,
}

#[derive(Deserialize)]
struct ChatResponse {
    message: ResponseMessage,
    /// Prompt tokens evaluated, reported natively by Ollama.
    prompt_eval_count: Option<u32>,
    /// Response tokens generated.
    eval_count: Option<u32>,
}

#[derive(Deserialize)]
struct ResponseMessage {
    content: String,
}

// ── Client implementation ─────────────────────────────────────────────────────

impl LlmClient for OllamaClient {
    fn complete(&self, req: &Request<'_>) -> Result<Response, LlmError> {
        let user_message = match req.image.as_ref() {
            None => Message {
                role: "user",
                content: req.user.to_string(),
                images: None,
            },
            Some(ImageData { base64, .. }) => Message {
                // Ollama takes raw base64 in a per-message `images` array, not a
                // data URI embedded in the content.
                role: "user",
                content: req.user.to_string(),
                images: Some(vec![base64.clone()]),
            },
        };

        let body = ChatRequest {
            model: self.model.clone(),
            messages: vec![
                Message {
                    role: "system",
                    content: req.system.to_string(),
                    images: None,
                },
                user_message,
            ],
            stream: false,
            options: Options {
                num_ctx: self.num_ctx,
                num_predict: self.effective_max_tokens(req.max_tokens),
                temperature: req.temperature,
            },
        };

        // Serialise once; reuse for every retry.
        let body_json = serde_json::to_string(&body)
            .map_err(|e| LlmError::Parse(format!("request serialisation failed: {e}")))?;

        let agent = ureq::AgentBuilder::new().timeout(self.timeout).build();

        let mut last_err = LlmError::Network("no attempts made".to_string());

        for attempt in 0..=self.max_retries {
            if attempt > 0 && self.verbose {
                eprintln!("[lx-llm] retry {attempt}/{}", self.max_retries);
            }

            let request = agent
                .post(&self.url)
                .set("Content-Type", "application/json");

            match request.send_string(&body_json) {
                Ok(resp) => {
                    let parsed: ChatResponse = resp
                        .into_json()
                        .map_err(|e| LlmError::Parse(e.to_string()))?;

                    let pt = parsed.prompt_eval_count;
                    let ct = parsed.eval_count;

                    if self.verbose {
                        if let (Some(p), Some(c)) = (pt, ct) {
                            eprintln!("[lx-llm] tokens: prompt={p} completion={c}");
                        }
                    }

                    return Ok(Response {
                        content: parsed.message.content,
                        prompt_tokens: pt,
                        completion_tokens: ct,
                    });
                }

                // Ollama is local; it doesn't rate-limit, but keep the 5xx retry
                // path for robustness (a proxy or a model still loading can 5xx).
                Err(ureq::Error::Status(status, resp)) if status >= 500 => {
                    let msg = resp.into_string().unwrap_or_default();
                    last_err = LlmError::Provider {
                        status,
                        message: msg,
                    };
                    if attempt < self.max_retries {
                        let wait = 1u64 << attempt;
                        eprintln!("[lx-llm] server error {status}; waiting {wait}s before retry");
                        std::thread::sleep(Duration::from_secs(wait));
                        continue;
                    }
                    return Err(last_err);
                }

                // 4xx — not retryable (bad request, model not found, context
                // overflow if num_ctx is somehow still too small, etc.).
                Err(ureq::Error::Status(status, resp)) => {
                    let message = resp.into_string().unwrap_or_default();
                    return Err(LlmError::Provider { status, message });
                }

                // Network / IO errors — retryable.
                Err(e) => {
                    last_err = LlmError::Network(e.to_string());
                    if attempt < self.max_retries {
                        let wait = 1u64 << attempt;
                        eprintln!("[lx-llm] network error; waiting {wait}s before retry");
                        std::thread::sleep(Duration::from_secs(wait));
                        continue;
                    }
                    return Err(last_err);
                }
            }
        }

        Err(last_err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_url_strips_v1_suffix() {
        assert_eq!(
            OllamaClient::chat_url("http://localhost:11434/v1"),
            "http://localhost:11434/api/chat"
        );
    }

    #[test]
    fn chat_url_handles_trailing_slash() {
        assert_eq!(
            OllamaClient::chat_url("http://localhost:11434/v1/"),
            "http://localhost:11434/api/chat"
        );
    }

    #[test]
    fn chat_url_without_v1() {
        // A bare host (custom base URL) gets /api/chat appended directly.
        assert_eq!(
            OllamaClient::chat_url("http://localhost:11434"),
            "http://localhost:11434/api/chat"
        );
    }

    #[test]
    fn chat_url_remote_host() {
        assert_eq!(
            OllamaClient::chat_url("https://ollama.example.com:11434/v1"),
            "https://ollama.example.com:11434/api/chat"
        );
    }

    #[test]
    fn request_body_has_num_ctx_and_num_predict() {
        // The whole point: options.num_ctx must be present in the serialised body.
        let body = ChatRequest {
            model: "qwen2.5:7b".into(),
            messages: vec![Message {
                role: "user",
                content: "hi".into(),
                images: None,
            }],
            stream: false,
            options: Options {
                num_ctx: 32_768,
                num_predict: 768,
                temperature: 0.0,
            },
        };
        let json = serde_json::to_string(&body).unwrap();
        assert!(json.contains("\"num_ctx\":32768"), "got: {json}");
        assert!(json.contains("\"num_predict\":768"), "got: {json}");
        assert!(json.contains("\"stream\":false"), "got: {json}");
        // Text-only message must not carry an `images` key.
        assert!(!json.contains("images"), "got: {json}");
    }

    #[test]
    fn effective_max_tokens_clamps_to_ceiling() {
        let c = OllamaClient::new(
            "http://localhost:11434/v1".into(),
            "qwen2.5:7b".into(),
            30,
            3,
            false,
            32_768,
            4096,
        );
        assert_eq!(c.effective_max_tokens(768), 768); // under ceiling
        assert_eq!(c.effective_max_tokens(8192), 4096); // clamped to ceiling
    }

    #[test]
    fn image_message_serializes_images_array() {
        let msg = Message {
            role: "user",
            content: "describe".into(),
            images: Some(vec!["QUJD".into()]),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"images\":[\"QUJD\"]"), "got: {json}");
    }
}
