use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::{ImageData, LlmClient, LlmError, Request, Response};

/// OpenAI-compatible HTTP client.
///
/// Supports: OpenAI, Azure OpenAI, Gemini (OpenAI-compat), DeepSeek,
/// OpenRouter, Ollama, llama.cpp, vLLM, LM Studio — any provider that speaks
/// the `/v1/chat/completions` wire format.
///
/// **Azure detection**: when `base_url` contains `.openai.azure.com` the client
/// automatically switches to the `api-key:` header instead of `Authorization: Bearer`.
pub struct OpenAiClient {
    api_key: String,
    base_url: String,
    model: String,
    timeout: Duration,
    max_retries: u32,
    verbose: bool,
}

impl OpenAiClient {
    pub fn new(
        api_key: String,
        base_url: String,
        model: String,
        timeout_secs: u64,
        max_retries: u32,
        verbose: bool,
    ) -> Self {
        OpenAiClient {
            api_key,
            base_url: base_url.trim_end_matches('/').to_string(),
            model,
            timeout: Duration::from_secs(timeout_secs),
            max_retries,
            verbose,
        }
    }

    fn is_azure(&self) -> bool {
        self.base_url.contains(".openai.azure.com")
    }
}

// ── Wire types ────────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<serde_json::Value>,
    max_tokens: u32,
    temperature: f32,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
    usage: Option<Usage>,
}

#[derive(Deserialize)]
struct Choice {
    message: AssistantMessage,
}

#[derive(Deserialize)]
struct AssistantMessage {
    content: String,
}

#[derive(Deserialize)]
struct Usage {
    prompt_tokens: Option<u32>,
    completion_tokens: Option<u32>,
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn build_user_content(user_text: &str, image: Option<&ImageData>) -> serde_json::Value {
    match image {
        None => serde_json::Value::String(user_text.to_string()),
        Some(img) => serde_json::json!([
            {
                "type": "image_url",
                "image_url": {
                    "url": format!("data:{};base64,{}", img.media_type, img.base64)
                }
            },
            {
                "type": "text",
                "text": user_text
            }
        ]),
    }
}

// ── Client implementation ─────────────────────────────────────────────────────

impl LlmClient for OpenAiClient {
    fn complete(&self, req: &Request<'_>) -> Result<Response, LlmError> {
        let url = format!("{}/chat/completions", self.base_url);

        let user_content = build_user_content(req.user, req.image.as_ref());

        let body = ChatRequest {
            model: self.model.clone(),
            messages: vec![
                serde_json::json!({"role": "system", "content": req.system}),
                serde_json::json!({"role": "user",   "content": user_content}),
            ],
            max_tokens: req.max_tokens,
            temperature: req.temperature,
        };

        // Serialise once; reuse for every retry.
        let body_json = serde_json::to_string(&body)
            .map_err(|e| LlmError::Parse(format!("request serialisation failed: {e}")))?;

        // Build the ureq agent once.
        let agent = ureq::AgentBuilder::new().timeout(self.timeout).build();

        let mut last_err = LlmError::Network("no attempts made".to_string());

        for attempt in 0..=self.max_retries {
            if attempt > 0 && self.verbose {
                eprintln!("[lx-llm] retry {attempt}/{}", self.max_retries);
            }

            let mut request = agent.post(&url).set("Content-Type", "application/json");

            request = if self.is_azure() {
                request.set("api-key", &self.api_key)
            } else {
                request.set("Authorization", &format!("Bearer {}", self.api_key))
            };

            match request.send_string(&body_json) {
                Ok(resp) => {
                    let parsed: ChatResponse = resp
                        .into_json()
                        .map_err(|e| LlmError::Parse(e.to_string()))?;

                    let content = parsed
                        .choices
                        .into_iter()
                        .next()
                        .map(|c| c.message.content)
                        .unwrap_or_default();

                    let (pt, ct) = parsed
                        .usage
                        .map(|u| (u.prompt_tokens, u.completion_tokens))
                        .unwrap_or((None, None));

                    if self.verbose {
                        if let (Some(p), Some(c)) = (pt, ct) {
                            eprintln!("[lx-llm] tokens: prompt={p} completion={c}");
                        }
                    }

                    return Ok(Response {
                        content,
                        prompt_tokens: pt,
                        completion_tokens: ct,
                    });
                }

                Err(ureq::Error::Status(429, resp)) => {
                    let wait = resp
                        .header("retry-after")
                        .and_then(|v| v.parse::<u64>().ok())
                        .unwrap_or(1u64 << attempt);
                    if attempt < self.max_retries {
                        eprintln!("[lx-llm] rate limited; waiting {wait}s before retry");
                        std::thread::sleep(Duration::from_secs(wait));
                        last_err = LlmError::RateLimited(attempt + 1);
                        continue;
                    }
                    return Err(LlmError::RateLimited(self.max_retries));
                }

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

                // 4xx (except 429) — not retryable
                Err(ureq::Error::Status(status, resp)) => {
                    let message = resp.into_string().unwrap_or_default();
                    return Err(LlmError::Provider { status, message });
                }

                // Network / IO errors — retryable
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
    fn is_azure_detection() {
        let client = OpenAiClient::new(
            "key".into(),
            "https://my-dep.openai.azure.com/openai/deployments/gpt-4".into(),
            "gpt-4".into(),
            30,
            3,
            false,
        );
        assert!(client.is_azure());
    }

    #[test]
    fn non_azure_url() {
        let client = OpenAiClient::new(
            "key".into(),
            "https://api.openai.com/v1".into(),
            "gpt-4o-mini".into(),
            30,
            3,
            false,
        );
        assert!(!client.is_azure());
    }

    #[test]
    fn trailing_slash_stripped() {
        let client = OpenAiClient::new(
            "key".into(),
            "https://api.openai.com/v1/".into(),
            "gpt-4o-mini".into(),
            30,
            3,
            false,
        );
        assert!(!client.base_url.ends_with('/'));
    }
}
