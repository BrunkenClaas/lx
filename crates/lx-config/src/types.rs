#![forbid(unsafe_code)]

use lx_core::exit::LxError;

// ── Provider ──────────────────────────────────────────────────────────────────

/// LLM provider selection.
///
/// Each variant carries a default base URL and default model for out-of-the-box
/// use. Both can be overridden via `base_url` / `model` in config or env vars.
/// Model defaults will rot as providers release newer generations — treat them
/// as a starting point, not a guarantee.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Provider {
    // ── Local inference ──────────────────────────────────────────────────────
    /// Ollama local inference server (default when nothing is configured).
    Ollama,
    /// LM Studio local inference server.
    LmStudio,
    // ── Cloud — Anthropic wire ───────────────────────────────────────────────
    /// Anthropic Claude API (native /v1/messages wire format).
    Anthropic,
    // ── Cloud — OpenAI-compatible wire ──────────────────────────────────────
    /// OpenAI API.
    Openai,
    /// Google Gemini via its OpenAI-compatible endpoint.
    Gemini,
    /// Groq cloud inference (LPU hardware).
    Groq,
    /// OpenRouter — aggregator for many providers.
    OpenRouter,
    /// Mistral AI API.
    Mistral,
    /// DeepSeek API.
    DeepSeek,
    /// Azure OpenAI (requires explicit base_url; uses api-key header).
    Azure,
}

impl Provider {
    pub fn as_str(&self) -> &'static str {
        match self {
            Provider::Ollama => "ollama",
            Provider::LmStudio => "lmstudio",
            Provider::Anthropic => "anthropic",
            Provider::Openai => "openai",
            Provider::Gemini => "gemini",
            Provider::Groq => "groq",
            Provider::OpenRouter => "openrouter",
            Provider::Mistral => "mistral",
            Provider::DeepSeek => "deepseek",
            Provider::Azure => "azure",
        }
    }

    /// Default base URL for this provider. Empty string for Azure (must be set
    /// explicitly by the user).
    pub fn default_base_url(&self) -> &'static str {
        match self {
            Provider::Ollama => "http://localhost:11434/v1",
            Provider::LmStudio => "http://localhost:1234/v1",
            Provider::Anthropic => "https://api.anthropic.com/v1",
            Provider::Openai => "https://api.openai.com/v1",
            Provider::Gemini => "https://generativelanguage.googleapis.com/v1beta/openai/",
            Provider::Groq => "https://api.groq.com/openai/v1",
            Provider::OpenRouter => "https://openrouter.ai/api/v1",
            Provider::Mistral => "https://api.mistral.ai/v1",
            Provider::DeepSeek => "https://api.deepseek.com/v1",
            Provider::Azure => "",
        }
    }

    /// Default model for this provider. Empty string for Azure (must be set
    /// explicitly). These defaults may become stale as providers release new
    /// generations — override via `model` in config or `LX_MODEL` env var.
    pub fn default_model(&self) -> &'static str {
        match self {
            Provider::Ollama => "llama3.1:8b",
            Provider::LmStudio => "llama3.1-8b-instruct",
            Provider::Anthropic => "claude-haiku-4-5",
            Provider::Openai => "gpt-4o-mini",
            Provider::Gemini => "gemini-2.5-flash-lite",
            Provider::Groq => "llama-3.1-8b-instant",
            Provider::OpenRouter => "meta-llama/llama-3.1-8b-instruct:free",
            Provider::Mistral => "mistral-small-latest",
            Provider::DeepSeek => "deepseek-chat",
            Provider::Azure => "",
        }
    }

    /// Local providers (Ollama, LM Studio) do not require an API key.
    pub fn is_local(&self) -> bool {
        matches!(self, Provider::Ollama | Provider::LmStudio)
    }

    /// Only Anthropic uses the /v1/messages wire format; all others are
    /// OpenAI-compatible (/v1/chat/completions).
    pub fn uses_anthropic_wire(&self) -> bool {
        matches!(self, Provider::Anthropic)
    }

    /// Azure uses an `api-key:` request header instead of `Authorization: Bearer`.
    pub fn uses_api_key_header(&self) -> bool {
        matches!(self, Provider::Azure)
    }

    pub fn parse(s: &str) -> Result<Self, LxError> {
        match s.to_ascii_lowercase().as_str() {
            "ollama" => Ok(Provider::Ollama),
            "lmstudio" | "lm-studio" | "lm_studio" => Ok(Provider::LmStudio),
            "anthropic" => Ok(Provider::Anthropic),
            "openai" => Ok(Provider::Openai),
            "gemini" | "google" => Ok(Provider::Gemini),
            "groq" => Ok(Provider::Groq),
            "openrouter" | "open-router" => Ok(Provider::OpenRouter),
            "mistral" => Ok(Provider::Mistral),
            "deepseek" | "deep-seek" => Ok(Provider::DeepSeek),
            "azure" => Ok(Provider::Azure),
            other => Err(LxError::ConfigAuth(format!(
                "invalid provider '{other}'; expected one of: \
                 ollama, lmstudio, anthropic, openai, gemini, groq, \
                 openrouter, mistral, deepseek, azure"
            ))),
        }
    }
}

impl std::fmt::Display for Provider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ── RedactLevel ───────────────────────────────────────────────────────────────

/// Secret-redaction strictness applied before every LLM call on flagged tools.
/// `Off` is intentionally not constructible from config — only from an explicit
/// `--no-redact` flag at the call site.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RedactLevel {
    /// Redact secrets and PII (API keys, tokens, emails).
    #[default]
    Standard,
    /// Additionally redact file paths, hostnames, and IP addresses.
    Strict,
}

impl RedactLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            RedactLevel::Standard => "standard",
            RedactLevel::Strict => "strict",
        }
    }

    /// Parse from string; returns `Standard` for unknown values with a warning.
    pub fn parse(s: &str) -> Result<Self, LxError> {
        match s.to_ascii_lowercase().as_str() {
            "standard" => Ok(RedactLevel::Standard),
            "strict" => Ok(RedactLevel::Strict),
            "off" => Err(LxError::ConfigAuth(
                "'redact.level = off' is not allowed in config files; \
                 use --no-redact flag per invocation instead"
                    .to_string(),
            )),
            other => Err(LxError::ConfigAuth(format!(
                "invalid redact level '{other}'; expected 'standard' or 'strict'"
            ))),
        }
    }
}

impl std::fmt::Display for RedactLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ── ColorMode ─────────────────────────────────────────────────────────────────

/// ANSI colour output preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorMode {
    /// Enable colours only when stdout is a TTY.
    #[default]
    Auto,
    /// Always emit ANSI colour codes.
    Always,
    /// Never emit ANSI colour codes.
    Never,
}

impl ColorMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            ColorMode::Auto => "auto",
            ColorMode::Always => "always",
            ColorMode::Never => "never",
        }
    }

    pub fn parse(s: &str) -> Result<Self, LxError> {
        match s.to_ascii_lowercase().as_str() {
            "auto" => Ok(ColorMode::Auto),
            "always" => Ok(ColorMode::Always),
            "never" => Ok(ColorMode::Never),
            other => Err(LxError::ConfigAuth(format!(
                "invalid color mode '{other}'; expected 'auto', 'always', or 'never'"
            ))),
        }
    }
}

impl std::fmt::Display for ColorMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ── ConfigOverrides ───────────────────────────────────────────────────────────

/// CLI-supplied overrides. Each field is `None` unless the flag was actually
/// passed. Applied after all file/env loading (highest priority).
#[derive(Debug, Default)]
pub struct ConfigOverrides {
    pub provider: Option<String>,
    pub base_url: Option<String>,
    pub model: Option<String>,
    pub timeout_secs: Option<u64>,
    pub max_retries: Option<u32>,
    pub max_input_bytes: Option<usize>,
    pub max_output_tokens: Option<u32>,
    pub redact_level: Option<String>,
    pub lang: Option<String>,
    pub color: Option<String>,
    pub shell: Option<String>,
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_roundtrip() {
        assert_eq!(Provider::parse("anthropic").unwrap(), Provider::Anthropic);
        assert_eq!(Provider::parse("OPENAI").unwrap(), Provider::Openai);
        assert_eq!(Provider::parse("ollama").unwrap(), Provider::Ollama);
        assert_eq!(Provider::parse("lm-studio").unwrap(), Provider::LmStudio);
        assert_eq!(Provider::parse("lm_studio").unwrap(), Provider::LmStudio);
        assert_eq!(Provider::parse("google").unwrap(), Provider::Gemini);
        assert_eq!(Provider::parse("groq").unwrap(), Provider::Groq);
        assert_eq!(Provider::parse("openrouter").unwrap(), Provider::OpenRouter);
        assert_eq!(Provider::parse("mistral").unwrap(), Provider::Mistral);
        assert_eq!(Provider::parse("deepseek").unwrap(), Provider::DeepSeek);
        assert_eq!(Provider::parse("azure").unwrap(), Provider::Azure);
        assert!(Provider::parse("unknown").is_err());
    }

    #[test]
    fn provider_defaults() {
        assert_eq!(
            Provider::Ollama.default_base_url(),
            "http://localhost:11434/v1"
        );
        assert_eq!(
            Provider::Anthropic.default_base_url(),
            "https://api.anthropic.com/v1"
        );
        assert_eq!(Provider::Azure.default_base_url(), "");
        assert_eq!(Provider::Ollama.default_model(), "llama3.1:8b");
        assert_eq!(Provider::Anthropic.default_model(), "claude-haiku-4-5");
        assert_eq!(Provider::Azure.default_model(), "");
    }

    #[test]
    fn provider_flags() {
        assert!(Provider::Ollama.is_local());
        assert!(Provider::LmStudio.is_local());
        assert!(!Provider::Openai.is_local());
        assert!(Provider::Anthropic.uses_anthropic_wire());
        assert!(!Provider::Openai.uses_anthropic_wire());
        assert!(Provider::Azure.uses_api_key_header());
        assert!(!Provider::Openai.uses_api_key_header());
    }

    #[test]
    fn redact_level_rejects_off() {
        assert!(matches!(
            RedactLevel::parse("off"),
            Err(LxError::ConfigAuth(_))
        ));
    }

    #[test]
    fn redact_level_roundtrip() {
        assert_eq!(RedactLevel::parse("strict").unwrap(), RedactLevel::Strict);
        assert_eq!(
            RedactLevel::parse("STANDARD").unwrap(),
            RedactLevel::Standard
        );
    }

    #[test]
    fn color_mode_roundtrip() {
        assert_eq!(ColorMode::parse("always").unwrap(), ColorMode::Always);
        assert_eq!(ColorMode::parse("Never").unwrap(), ColorMode::Never);
        assert!(ColorMode::parse("yes").is_err());
    }

    #[test]
    fn provider_display() {
        assert_eq!(Provider::Anthropic.to_string(), "anthropic");
        assert_eq!(Provider::Openai.to_string(), "openai");
        assert_eq!(Provider::Ollama.to_string(), "ollama");
        assert_eq!(Provider::LmStudio.to_string(), "lmstudio");
        assert_eq!(Provider::Gemini.to_string(), "gemini");
    }
}
