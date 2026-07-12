use thiserror::Error;

/// All errors that can occur during an LLM call.
#[derive(Debug, Error)]
pub enum LlmError {
    /// Transient network failure (DNS, TCP, TLS, timeout).
    #[error("network error: {0}")]
    Network(String),

    /// Exhausted all retry attempts due to 429 rate-limiting.
    #[error("rate limited after {0} retries")]
    RateLimited(u32),

    /// Provider returned a non-retried error HTTP status.
    #[error("provider error {status}: {message}")]
    Provider { status: u16, message: String },

    /// Provider response could not be parsed into the expected shape.
    #[error("response parse error: {0}")]
    Parse(String),

    /// No API key was found in environment or configuration.
    #[error("missing API key — set LX_API_KEY or configure the OS credential store")]
    MissingApiKey,
}

impl From<LlmError> for lx_core::error::LxError {
    fn from(e: LlmError) -> Self {
        match e {
            LlmError::MissingApiKey => lx_core::error::LxError::ConfigAuth(e.to_string()),
            other => lx_core::error::LxError::NetworkLlm(other.to_string()),
        }
    }
}
