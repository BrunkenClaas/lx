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
pub use schema::{
    extract_text, parse_response, parse_response_checked, validate_json, validate_json_checked,
    Completeness,
};

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

/// Why the provider stopped generating, when it says.
///
/// [`StopReason::Length`] is the only variant with a correctness consequence:
/// the response is a *prefix* of what the model intended to write, so any
/// collection in it is incomplete. Providers that report nothing usable yield
/// [`StopReason::Unknown`], in which case the JSON parser's own salvage signal
/// ([`Completeness`]) is the only evidence available.
///
/// A stop reason we do not recognise maps to [`StopReason::Complete`]: only an
/// explicit length-stop is treated as truncation, so a provider-specific value
/// such as `tool_use` never raises a spurious warning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StopReason {
    /// The model finished on its own.
    Complete,
    /// The provider cut generation at the token cap.
    Length,
    /// The provider reported no usable stop reason.
    #[default]
    Unknown,
}

impl StopReason {
    /// Map a provider's raw stop-reason string onto [`StopReason`].
    ///
    /// `length_value` is the provider's own spelling of a token-cap stop:
    /// `"length"` for OpenAI-compatible and Ollama, `"max_tokens"` for
    /// Anthropic. Every other non-empty value is [`StopReason::Complete`] —
    /// only an explicit length-stop counts as truncation. A missing field is
    /// [`StopReason::Unknown`].
    pub fn from_provider(raw: Option<&str>, length_value: &str) -> Self {
        match raw {
            None => StopReason::Unknown,
            Some(v) if v == length_value => StopReason::Length,
            Some(_) => StopReason::Complete,
        }
    }
}

/// The LLM's response to a single completion request.
pub struct Response {
    /// The text content of the first choice / first message block.
    pub content: String,
    /// Prompt / input tokens consumed, if reported by the provider.
    pub prompt_tokens: Option<u32>,
    /// Completion / output tokens consumed, if reported by the provider.
    pub completion_tokens: Option<u32>,
    /// Why generation stopped, when the provider reports it.
    ///
    /// Checked alongside [`Completeness`] to decide whether a result is a
    /// truncated prefix: the provider signal catches a reply cut on a token
    /// boundary that still parses as valid JSON, which salvage cannot see.
    pub stop_reason: StopReason,
}

/// Provider-agnostic LLM client.
///
/// Implementations must be `Send + Sync` so they can be passed across threads
/// (e.g. stored in a `Box<dyn LlmClient>` and referenced from multiple call
/// sites without wrapping in a Mutex).
pub trait LlmClient: Send + Sync {
    fn complete(&self, req: &Request<'_>) -> Result<Response, LlmError>;
}
