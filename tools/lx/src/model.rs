//! `lx model` — resolve and report the *effective* LLM model.
//!
//! This is a **diagnostic** command, not a productive content tool. It exists so
//! scripts (e.g. the acceptance harness) can learn which model the suite will
//! actually use, rather than guessing from `LX_MODEL` — which may be unset (a
//! provider default is used) or overridden by config files.
//!
//! Resolution mirrors what every real tool does at startup: load config via
//! `lx-config`, then read `effective_model()` / the resolved provider. Unlike
//! the productive tools, `lx` itself produces no LLM content; here the LLM
//! is contacted only to *verify* that the resolved model actually responds.

use lx_config::{Config, Provider};
use lx_core::error::LxError;
use lx_llm::{client_from_config, Request};
use serde::Serialize;

/// Resolved model/provider, plus optional live-reachability result.
#[derive(Debug, Serialize)]
pub struct ModelInfo {
    /// Effective model name (explicit config value, else provider default).
    pub model: String,
    /// Canonical provider name (e.g. "anthropic", "ollama").
    pub provider: String,
    /// `Some(true)` if a verification call succeeded, `Some(false)` if it
    /// failed, `None` if verification was skipped (`--no-verify`).
    pub reachable: Option<bool>,
    /// Error message from a failed verification call, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Resolve the effective model/provider from config. Never calls the network.
pub fn resolve(config: &Config) -> Result<(String, String), LxError> {
    let model = config.effective_model().to_string();
    // `config.llm.provider` is always populated by lx-config (defaults to
    // ollama); normalise it to the canonical spelling.
    let provider = Provider::parse(&config.llm.provider)?.as_str().to_string();
    Ok((model, provider))
}

/// Resolve the effective model and (unless `skip_verify`) make a minimal LLM
/// call to confirm the model actually answers.
///
/// The verification request is deliberately tiny: a fixed system prompt and a
/// one-token reply ceiling, `temperature = 0.0`. We only care whether the call
/// succeeds — the response text is discarded.
pub fn probe(config: &Config, skip_verify: bool, verbose: bool) -> Result<ModelInfo, LxError> {
    let (model, provider) = resolve(config)?;

    if skip_verify {
        return Ok(ModelInfo {
            model,
            provider,
            reachable: None,
            error: None,
        });
    }

    let client = client_from_config(config, verbose)?;
    let req = Request {
        system: "You are a connectivity probe. Reply with the single word: ok.",
        user: "ping",
        max_tokens: 5,
        temperature: 0.0,
        image: None,
    };

    match client.complete(&req) {
        Ok(_) => Ok(ModelInfo {
            model,
            provider,
            reachable: Some(true),
            error: None,
        }),
        Err(e) => Ok(ModelInfo {
            model,
            provider,
            reachable: Some(false),
            error: Some(e.to_string()),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_uses_effective_model_default() {
        // Default config => ollama provider, its default model.
        let cfg = Config::default();
        let (model, provider) = resolve(&cfg).unwrap();
        assert!(!model.is_empty(), "effective model must not be empty");
        assert_eq!(provider, "ollama");
    }

    #[test]
    fn resolve_uses_explicit_model() {
        let mut cfg = Config::default();
        cfg.llm.provider = "openai".to_string();
        cfg.llm.model = "gpt-4o".to_string();
        let (model, provider) = resolve(&cfg).unwrap();
        assert_eq!(model, "gpt-4o");
        assert_eq!(provider, "openai");
    }

    #[test]
    fn probe_skip_verify_reports_none() {
        let cfg = Config::default();
        let info = probe(&cfg, true, false).unwrap();
        assert_eq!(info.reachable, None);
        assert!(info.error.is_none());
    }
}
