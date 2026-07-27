use lx_config::api_key::provider_key_hint;
use lx_config::{Config, Provider};
use lx_core::error::LxError;

use crate::LlmClient;

/// The provider-specific JSON body fragment that disables reasoning, for the
/// OpenAI-compatible (`/chat/completions`) clients.
///
/// Returns `Some(object)` **only** where the field is verified to actually STOP
/// reasoning (not merely hide it) and is safe to send (honoured or ignored, never a
/// 400 except on mandatory-reasoning models). Fields checked against provider docs
/// 2026-07-27:
///   - OpenRouter → `{"reasoning": {"effort": "none"}}` — *"Disables reasoning
///     entirely."* NOTE: **not** `exclude: true`, which only hides the chain-of-thought
///     while the model keeps reasoning and burning completion tokens — that would not
///     fix the output-budget truncation this feature exists to prevent. (OpenRouter
///     flags some models `"mandatory": true` and rejects `effort:"none"` on them; that
///     400 is inherent and covered by the best-effort contract.)
///   - Gemini (OpenAI-compat) → `{"reasoning_effort": "none"}` (2.5 Flash → budget 0;
///     2.5 Pro can't fully disable but ignores/floors the field)
///   - DeepSeek → `{"thinking": {"type": "disabled"}}` — genuine non-thinking mode
///
/// Returns `None` for every other provider — crucially Anthropic and Groq, which
/// **reject** a disable field with HTTP 400. Sending nothing there never breaks a
/// working request; "reasoning off" is therefore best-effort per provider.
/// (Ollama is not handled here — it uses the native `OllamaClient`, which sends
/// `think: false` directly.) Only called when `config.llm.reasoning == false`.
fn openai_reasoning_off_fragment(provider: &Provider) -> Option<serde_json::Value> {
    match provider {
        Provider::OpenRouter => Some(serde_json::json!({"reasoning": {"effort": "none"}})),
        Provider::Gemini => Some(serde_json::json!({"reasoning_effort": "none"})),
        Provider::DeepSeek => Some(serde_json::json!({"thinking": {"type": "disabled"}})),
        // Anthropic/Groq 400 on a disable field; OpenAI's floor is "minimal" (not
        // truly off); Mistral/Azure/LM Studio have no safe field. Send nothing.
        _ => None,
    }
}

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

    // Global output-token ceiling (config `limits.max_output_tokens`). Each
    // client clamps every request's per-tool max_tokens to min(max_tokens, ceiling).
    let max_output_ceiling = config.limits.max_output_tokens;

    if provider.uses_anthropic_wire() {
        let client = crate::anthropic::AnthropicClient::new(
            api_key,
            base_url,
            model,
            config.llm.timeout_secs,
            config.llm.max_retries,
            verbose,
            max_output_ceiling,
        );
        Ok(Box::new(client))
    } else if matches!(provider, Provider::Ollama) {
        // Ollama needs its NATIVE /api/chat endpoint: its OpenAI-compat /v1
        // layer silently ignores num_ctx and truncates the prompt to ~2048
        // tokens. Only the native endpoint honours num_ctx (under `options`),
        // so the Ollama provider gets a dedicated client. See ollama.rs.
        let client = crate::ollama::OllamaClient::new(
            base_url,
            model,
            config.llm.timeout_secs,
            config.llm.max_retries,
            verbose,
            config.llm.num_ctx,
            max_output_ceiling,
            // reasoning=false → send `think: false` on the native endpoint.
            !config.llm.reasoning,
        );
        Ok(Box::new(client))
    } else {
        // All other OpenAI-compatible providers, hosted and local (LM Studio,
        // llama.cpp, vLLM). num_ctx is NOT sent here: hosted providers manage
        // context themselves and may 400 on unknown fields, and LM Studio
        // ignores num_ctx in the body entirely (its context is fixed by the GUI
        // "Context Length" slider when the model loads — set it to >=32k there).
        // reasoning=false → send the provider's disable-reasoning field, but only
        // where it's known safe (see openai_reasoning_off_fragment).
        let reasoning_off = if config.llm.reasoning {
            None
        } else {
            openai_reasoning_off_fragment(&provider)
        };
        let client = crate::openai::OpenAiClient::new(
            api_key,
            base_url,
            model,
            config.llm.timeout_secs,
            config.llm.max_retries,
            verbose,
            None,
            max_output_ceiling,
            reasoning_off,
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
    fn reasoning_off_fragment_only_for_safe_providers() {
        // Providers with a verified-safe disable field.
        assert!(openai_reasoning_off_fragment(&Provider::OpenRouter).is_some());
        assert!(openai_reasoning_off_fragment(&Provider::Gemini).is_some());
        assert!(openai_reasoning_off_fragment(&Provider::DeepSeek).is_some());
        // OpenRouter's exact shape: effort:"none" (fully disables reasoning), NOT
        // exclude:true (which only hides it while the model keeps burning tokens).
        let f = openai_reasoning_off_fragment(&Provider::OpenRouter).unwrap();
        assert_eq!(f, serde_json::json!({"reasoning": {"effort": "none"}}));

        // Providers that 400 on a disable field, or have no safe one → None.
        // This is the guard against breaking a working request.
        for p in [
            Provider::Anthropic, // 400: thinking:{type:disabled} unsupported
            Provider::Groq,      // 400: rejects reasoning_effort:"none"
            Provider::Openai,    // floor is "minimal", not truly off
            Provider::Mistral,
            Provider::Azure,
            Provider::LmStudio,
        ] {
            assert!(
                openai_reasoning_off_fragment(&p).is_none(),
                "provider {p:?} must not receive a reasoning-off field"
            );
        }
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
