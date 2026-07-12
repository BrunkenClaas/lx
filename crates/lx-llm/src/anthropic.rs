use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::{ImageData, LlmClient, LlmError, Request, Response};

const ANTHROPIC_VERSION: &str = "2023-06-01";
const ANTHROPIC_DEFAULT_BASE: &str = "https://api.anthropic.com/v1";

/// Anthropic-native HTTP client using the `/v1/messages` endpoint.
///
/// `base_url` defaults to `https://api.anthropic.com/v1` but can be overridden
/// to target AWS Bedrock, Google Vertex, or any compatible proxy.
pub struct AnthropicClient {
    api_key: String,
    model: String,
    /// Fully-resolved messages endpoint URL, e.g. `…/v1/messages`.
    endpoint: String,
    timeout: Duration,
    max_retries: u32,
    verbose: bool,
}

impl AnthropicClient {
    pub fn new(
        api_key: String,
        base_url: String,
        model: String,
        timeout_secs: u64,
        max_retries: u32,
        verbose: bool,
    ) -> Self {
        let base = if base_url.is_empty() {
            ANTHROPIC_DEFAULT_BASE.to_string()
        } else {
            base_url.trim_end_matches('/').to_string()
        };
        let endpoint = format!("{base}/messages");
        AnthropicClient {
            api_key,
            model,
            endpoint,
            timeout: Duration::from_secs(timeout_secs),
            max_retries,
            verbose,
        }
    }
}

fn is_zero(v: &f32) -> bool {
    *v == 0.0
}

// ── Wire types ────────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct MessagesRequest {
    model: String,
    system: String,
    messages: Vec<serde_json::Value>,
    max_tokens: u32,
    // Omitted when 0.0: Opus 4.8+ rejects the field entirely; older models and
    // 0.0 are equivalent (0.0 is the API default). We never send non-zero values.
    #[serde(skip_serializing_if = "is_zero")]
    temperature: f32,
}

#[derive(Deserialize)]
struct MessagesResponse {
    content: Vec<ContentBlock>,
    usage: Option<AnthropicUsage>,
}

#[derive(Deserialize)]
struct ContentBlock {
    #[serde(rename = "type")]
    block_type: String,
    text: Option<String>,
}

#[derive(Deserialize)]
struct AnthropicUsage {
    input_tokens: Option<u32>,
    output_tokens: Option<u32>,
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn build_user_message(user_text: &str, image: Option<&ImageData>) -> serde_json::Value {
    match image {
        None => serde_json::json!({"role": "user", "content": user_text}),
        Some(img) => serde_json::json!({
            "role": "user",
            "content": [
                {
                    "type": "image",
                    "source": {
                        "type": "base64",
                        "media_type": img.media_type,
                        "data": img.base64
                    }
                },
                {
                    "type": "text",
                    "text": user_text
                }
            ]
        }),
    }
}

// ── Client implementation ─────────────────────────────────────────────────────

impl LlmClient for AnthropicClient {
    fn complete(&self, req: &Request<'_>) -> Result<Response, LlmError> {
        let body = MessagesRequest {
            model: self.model.clone(),
            system: req.system.to_string(),
            messages: vec![build_user_message(req.user, req.image.as_ref())],
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

            let request = agent
                .post(&self.endpoint)
                .set("x-api-key", &self.api_key)
                .set("anthropic-version", ANTHROPIC_VERSION)
                .set("content-type", "application/json");

            match request.send_string(&body_json) {
                Ok(resp) => {
                    let parsed: MessagesResponse = resp
                        .into_json()
                        .map_err(|e| LlmError::Parse(e.to_string()))?;

                    let content = parsed
                        .content
                        .into_iter()
                        .find(|b| b.block_type == "text")
                        .and_then(|b| b.text)
                        .unwrap_or_default();

                    let (pt, ct) = parsed
                        .usage
                        .map(|u| (u.input_tokens, u.output_tokens))
                        .unwrap_or((None, None));

                    if self.verbose {
                        if let (Some(p), Some(c)) = (pt, ct) {
                            eprintln!("[lx-llm] tokens: input={p} output={c}");
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
    fn client_construction_default_base_url() {
        let client = AnthropicClient::new(
            "sk-ant-api-test".into(),
            String::new(), // empty = use default
            "claude-haiku-4-5".into(),
            30,
            3,
            false,
        );
        assert_eq!(client.model, "claude-haiku-4-5");
        assert_eq!(client.endpoint, "https://api.anthropic.com/v1/messages");
        assert_eq!(client.max_retries, 3);
    }

    #[test]
    fn client_construction_custom_base_url() {
        let client = AnthropicClient::new(
            "sk-ant-api-test".into(),
            "https://bedrock.example.com/v1".into(),
            "claude-haiku-4-5".into(),
            30,
            3,
            false,
        );
        assert_eq!(client.endpoint, "https://bedrock.example.com/v1/messages");
    }

    #[test]
    fn trailing_slash_stripped_from_base_url() {
        let client = AnthropicClient::new(
            "key".into(),
            "https://proxy.example.com/v1/".into(),
            "claude-haiku-4-5".into(),
            30,
            3,
            false,
        );
        assert_eq!(client.endpoint, "https://proxy.example.com/v1/messages");
    }

    #[test]
    fn request_serialises_correctly() {
        let body = MessagesRequest {
            model: "claude-haiku-4-5".to_string(),
            system: "You are a helpful assistant.".to_string(),
            messages: vec![serde_json::json!({"role": "user", "content": "hello"})],
            max_tokens: 256,
            temperature: 0.0,
        };
        let json = serde_json::to_string(&body).unwrap();
        assert!(json.contains("\"model\":\"claude-haiku-4-5\""));
        assert!(json.contains("\"system\":\"You are a helpful assistant.\""));
        assert!(json.contains("\"max_tokens\":256"));
    }

    #[test]
    fn image_message_contains_base64_block() {
        let img = crate::ImageData {
            base64: "abc123".to_string(),
            media_type: "image/png".to_string(),
        };
        let msg = build_user_message("describe this", Some(&img));
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"image\""));
        assert!(json.contains("\"data\":\"abc123\""));
        assert!(json.contains("\"media_type\":\"image/png\""));
    }
}
