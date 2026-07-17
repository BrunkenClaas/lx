#![forbid(unsafe_code)]

pub mod anthropic;
pub mod client;
pub mod error;
pub mod fragments;
pub mod lang;
pub mod ollama;
pub mod openai;
pub mod schema;

pub use client::client_from_config;
pub use error::LlmError;

pub use fragments::{
    render, DANGEROUS_COMMAND_INSTRUCTION, JSON_ONLY_INSTRUCTION, UNTRUSTED_DATA_INSTRUCTION,
};
pub use lang::{inject_lang, inject_os, strip_lang_fallback};
pub use schema::{extract_text, parse_response, validate_json};

/// Base64-encoded image data for multimodal requests.
#[derive(Debug, Clone)]
pub struct ImageData {
    /// Base64-encoded image bytes.
    pub base64: String,
    /// MIME type: `"image/jpeg"`, `"image/png"`, `"image/gif"`, or `"image/webp"`.
    pub media_type: String,
}

/// A single, blocking LLM completion request.
pub struct Request<'a> {
    /// Static system prompt (embedded via `include_str!` in the tool).
    pub system: &'a str,
    /// User message — already redacted if the tool has the `redact` flag.
    pub user: &'a str,
    /// Hard token limit for the response. Set tight per-tool.
    pub max_tokens: u32,
    /// Always 0.0 for deterministic output.
    pub temperature: f32,
    /// Optional image to include in the user message (multimodal).
    pub image: Option<ImageData>,
}

/// The LLM's response to a single completion request.
pub struct Response {
    /// The text content of the first choice / first message block.
    pub content: String,
    /// Prompt / input tokens consumed, if reported by the provider.
    pub prompt_tokens: Option<u32>,
    /// Completion / output tokens consumed, if reported by the provider.
    pub completion_tokens: Option<u32>,
}

/// Provider-agnostic LLM client.
///
/// Implementations must be `Send + Sync` so they can be passed across threads
/// (e.g. stored in a `Box<dyn LlmClient>` and referenced from multiple call
/// sites without wrapping in a Mutex).
pub trait LlmClient: Send + Sync {
    fn complete(&self, req: &Request<'_>) -> Result<Response, LlmError>;
}
