use lx_config::api_key::provider_key_hint;
use lx_config::{Config, Provider};
use lx_core::error::LxError;

use crate::LlmClient;

/// Construct the correct LLM client from the loaded configuration.
///
/// The provider is determined by `config.llm.provider` (already resolved from
/// env vars and config files by `lx-config`). Both wire-format clients are
/// compiled in; no rebuild is needed when switching providers at runtime.
///
/// `base_url` and `model` fall back to per-provider defaults when empty.
/// Local providers (Ollama, LM Studio) do not require an API key.
///
/// Pass `verbose = true` (from the tool's `--verbose` flag) to enable token
/// count logging and retry diagnostics on stderr.
///
/// # Errors
/// Returns `LxError::ConfigAuth` when the provider name is unrecognised.
/// Returns `LxError::ConfigAuth` when no API key can be resolved for a
/// non-local provider.
pub fn client_from_config(config: &Config, verbose: bool) -> Result<Box<dyn LlmClient>, LxError> {
    let provider = Provider::parse(&config.llm.provider)?;
    let base_url = config.effective_base_url().to_string();
    let model = config.effective_model().to_string();

    let api_key = if provider.is_local() {
        // Local providers accept any string; use the provider name as a
        // placeholder so the HTTP layer has something non-empty to send.
        config
            .resolve_api_key()
            .unwrap_or_else(|| provider.as_str().to_string())
    } else {
        config
            .resolve_api_key()
            .ok_or_else(|| LxError::ConfigAuth(provider_key_hint(&provider)))?
    };

    if provider.uses_anthropic_wire() {
        let client = crate::anthropic::AnthropicClient::new(
            api_key,
            base_url,
            model,
            config.llm.timeout_secs,
            config.llm.max_retries,
            verbose,
        );
        Ok(Box::new(client))
    } else {
        let client = crate::openai::OpenAiClient::new(
            api_key,
            base_url,
            model,
            config.llm.timeout_secs,
            config.llm.max_retries,
            verbose,
        );
        Ok(Box::new(client))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lx_config::Config;

    #[test]
    fn missing_api_key_non_local_returns_config_error() {
        let mut cfg = Config::default();
        cfg.llm.provider = "openai".to_string();
        cfg.llm.api_key = None;

        if std::env::var("LX_API_KEY").is_err() {
            let result = client_from_config(&cfg, false);
            assert!(
                matches!(result, Err(LxError::ConfigAuth(_))),
                "expected ConfigAuth error"
            );
        }
    }

    #[test]
    fn local_provider_needs_no_api_key() {
        let mut cfg = Config::default(); // provider=ollama
        cfg.llm.api_key = None;

        // Ollama is local — should construct without an API key.
        if std::env::var("LX_API_KEY").is_err() {
            assert!(client_from_config(&cfg, false).is_ok());
        }
    }

    #[test]
    fn openai_provider_selected() {
        let mut cfg = Config::default();
        cfg.llm.provider = "openai".to_string();
        cfg.llm.api_key = Some("sk-test".to_string());

        assert!(client_from_config(&cfg, false).is_ok());
    }

    #[test]
    fn anthropic_provider_selected() {
        let mut cfg = Config::default();
        cfg.llm.provider = "anthropic".to_string();
        cfg.llm.api_key = Some("sk-ant-test".to_string());

        assert!(client_from_config(&cfg, false).is_ok());
    }

    #[test]
    fn all_named_providers_construct_ok() {
        let providers = [
            ("ollama", None),
            ("lmstudio", None),
            ("openai", Some("sk-test")),
            ("anthropic", Some("sk-ant-test")),
            ("gemini", Some("AIza-test")),
            ("groq", Some("gsk_test")),
            ("openrouter", Some("sk-or-test")),
            ("mistral", Some("msk-test")),
            ("deepseek", Some("dsk-test")),
        ];
        for (provider, key) in providers {
            let mut cfg = Config::default();
            cfg.llm.provider = provider.to_string();
            cfg.llm.api_key = key.map(String::from);
            assert!(
                client_from_config(&cfg, false).is_ok(),
                "provider '{provider}' failed to construct"
            );
        }
    }
}
