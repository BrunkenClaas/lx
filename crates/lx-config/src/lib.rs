#![forbid(unsafe_code)]

pub mod api_key;
pub mod types;

pub use types::{ColorMode, ConfigOverrides, Provider, RedactLevel};

use lx_core::exit::LxError;
use serde::{Deserialize, Serialize};

// ── Top-level Config struct ───────────────────────────────────────────────────
//
// The nested layout ([llm], [limits], [redact], [output]) matches the
// TOML file structure and is used throughout all tool binaries. The typed enum
// helpers in `types.rs` sit alongside the String fields rather than replacing
// them, keeping the 169 tool scaffolds compile-compatible.

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub llm: LlmConfig,
    pub limits: LimitsConfig,
    pub redact: RedactConfig,
    pub output: OutputConfig,
}

// ── [llm] ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LlmConfig {
    /// Provider name — see `Provider` enum for valid values.
    pub provider: String,
    /// Base URL override. Empty string means "use the provider's built-in
    /// default URL". Non-empty always overrides (enables Bedrock, Vertex, etc.).
    pub base_url: String,
    /// Model identifier override. Empty string means "use the provider's
    /// built-in default model". Non-empty always overrides.
    pub model: String,
    /// HTTP request timeout in seconds.
    pub timeout_secs: u64,
    /// Maximum retry attempts on transient errors (429, 5xx, network).
    pub max_retries: u32,
    /// Context window (in tokens) requested from local providers only.
    ///
    /// Sent as `num_ctx` to Ollama / LM Studio so the model sees the full
    /// prompt instead of Ollama's small default (~2–4k), which silently
    /// truncates the input and produces malformed output. Ignored for hosted
    /// providers (they manage context themselves and reject unknown fields).
    pub num_ctx: u32,
    /// API key — resolved from env/credential-store, never read from files.
    #[serde(skip)]
    pub api_key: Option<String>,
}

impl Default for LlmConfig {
    fn default() -> Self {
        LlmConfig {
            // Default to Ollama so the suite works out-of-the-box for local
            // inference without any configuration.
            provider: "ollama".to_string(),
            base_url: String::new(), // empty = use provider default
            model: String::new(),    // empty = use provider default
            timeout_secs: 30,
            max_retries: 3,
            // 32k covers every tool's system prompt + a large piped input on a
            // local model. Only sent to local providers (see `num_ctx` doc).
            num_ctx: 32_768,
            api_key: None,
        }
    }
}

// ── [limits] ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LimitsConfig {
    pub max_input_bytes: usize,
    pub max_output_tokens: u32,
}

impl Default for LimitsConfig {
    fn default() -> Self {
        LimitsConfig {
            max_input_bytes: 524_288, // 512 KiB
            // Global ceiling: min(per-tool MAX_TOKENS, this) wins. Set at the
            // suite's highest per-tool budget (4096) so it never caps a tool by
            // default; lower it to reduce every tool's output globally.
            max_output_tokens: 4096,
        }
    }
}

// ── [redact] ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RedactConfig {
    /// "standard" or "strict". "off" is never accepted from config files.
    pub level: String,
}

impl Default for RedactConfig {
    fn default() -> Self {
        RedactConfig {
            level: "standard".to_string(),
        }
    }
}

// ── [output] ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct OutputConfig {
    /// BCP-47 language tag, or "auto" to detect from environment.
    pub lang: String,
    /// "auto", "always", or "never".
    pub color: String,
    /// Detected shell: bash, zsh, sh, fish, powershell, cmd. "auto" is resolved at load time
    /// via lx_core::platform::detect_shell(). Not persisted to config files; override via
    /// --shell flag or the LX_SHELL env var (read inside detect_shell, not apply_env_vars).
    #[serde(skip)]
    pub shell: String,
}

impl Default for OutputConfig {
    fn default() -> Self {
        OutputConfig {
            lang: "auto".to_string(),
            color: "auto".to_string(),
            shell: "auto".to_string(),
        }
    }
}

// ── Config loading ────────────────────────────────────────────────────────────

impl Config {
    /// Load configuration from all sources in priority order, then apply
    /// `overrides` (CLI flags — highest priority).
    ///
    /// Priority (highest first):
    ///   1. `overrides` (CLI flags passed by the tool's `main.rs`)
    ///   2. `LX_*` environment variables
    ///   3. `./.lx.toml` (project-local) — secret keys filtered out with warning
    ///   4. `$XDG_CONFIG_HOME/lx/config.toml` or `%APPDATA%\lx\config.toml`
    ///   5. Compiled-in defaults
    pub fn load() -> Result<Self, LxError> {
        Self::load_with_overrides(ConfigOverrides::default())
    }

    /// Like `load()` but also applies caller-supplied CLI overrides at the end.
    pub fn load_with_overrides(overrides: ConfigOverrides) -> Result<Self, LxError> {
        let mut cfg = Config::default();

        // Layer 4 — user config file
        let user_path = lx_core::platform::config_dir().join("config.toml");
        if user_path.exists() {
            merge_toml_file(&mut cfg, &user_path, false)?;
        }

        // Layer 3 — project-local .lx.toml (secrets filtered)
        if let Ok(cwd) = std::env::current_dir() {
            let local = cwd.join(".lx.toml");
            if local.exists() {
                merge_toml_file(&mut cfg, &local, true)?;
            }
        }

        // Layer 2 — environment variables
        apply_env_vars(&mut cfg);

        // Layer 1 — CLI overrides
        apply_overrides(&mut cfg, overrides);

        // Resolve "auto" language now that all layers have been applied.
        if cfg.output.lang == "auto" {
            cfg.output.lang = lx_core::platform::locale();
        }

        // Resolve "auto" shell now that all layers have been applied.
        if cfg.output.shell == "auto" {
            cfg.output.shell = lx_core::platform::detect_shell();
        }

        // Validate typed fields — reject bad values with clear messages.
        validate(&cfg)?;

        Ok(cfg)
    }

    /// Resolve the effective base URL: explicit `llm.base_url` if non-empty,
    /// otherwise the provider's built-in default.
    pub fn effective_base_url(&self) -> &str {
        if !self.llm.base_url.is_empty() {
            return &self.llm.base_url;
        }
        Provider::parse(&self.llm.provider)
            .map(|p| p.default_base_url())
            .unwrap_or("")
    }

    /// Resolve the effective model: explicit `llm.model` if non-empty,
    /// otherwise the provider's built-in default.
    pub fn effective_model(&self) -> &str {
        if !self.llm.model.is_empty() {
            return &self.llm.model;
        }
        Provider::parse(&self.llm.provider)
            .map(|p| p.default_model())
            .unwrap_or("")
    }

    /// Resolve the API key: `LX_API_KEY` env → OS credential store.
    /// Returns `None` if neither source has a key (tools call `api_key::api_key()`
    /// directly when they need an error; this variant is used by lx-llm which
    /// already has its own error path).
    pub fn resolve_api_key(&self) -> Option<String> {
        std::env::var("LX_API_KEY")
            .ok()
            .filter(|k| !k.trim().is_empty())
            .map(|k| k.trim().to_string())
            .or_else(|| self.llm.api_key.clone())
    }
}

// ── TOML file merging ─────────────────────────────────────────────────────────

/// Known top-level TOML section names — anything else is warned about.
const KNOWN_SECTIONS: &[&str] = &["llm", "limits", "redact", "output"];

/// Patterns that must never appear in `.lx.toml` (project-local config).
const SECRET_KEY_PATTERNS: &[&str] = &["api_key", "secret", "token", "password", "key"];

fn merge_toml_file(
    cfg: &mut Config,
    path: &std::path::Path,
    filter_secrets: bool,
) -> Result<(), LxError> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| LxError::ConfigAuth(format!("cannot read {}: {e}", path.display())))?;

    let mut value: toml::Value = toml::from_str(&content)
        .map_err(|e| LxError::ConfigAuth(format!("invalid TOML in {}: {e}", path.display())))?;

    let toml::Value::Table(ref mut table) = value else {
        return Ok(());
    };

    // Warn about unknown top-level keys.
    for key in table.keys() {
        if !KNOWN_SECTIONS.contains(&key.as_str()) {
            eprintln!(
                "warning: unknown config key '{}' in {} (ignored)",
                key,
                path.display()
            );
        }
    }

    // For project-local .lx.toml: scan all nested keys for secret patterns and
    // remove them with a warning rather than aborting.
    if filter_secrets {
        filter_secret_keys(table, path);
    }

    // Re-serialise the (possibly filtered) table and deserialise into our struct.
    let filtered_toml = toml::to_string(&value)
        .map_err(|e| LxError::ConfigAuth(format!("internal TOML re-serialise error: {e}")))?;

    let overlay: Config = toml::from_str(&filtered_toml).unwrap_or_default();
    merge_into(cfg, overlay);

    Ok(())
}

/// Walk the TOML table and remove any key whose name matches a secret pattern.
fn filter_secret_keys(table: &mut toml::map::Map<String, toml::Value>, path: &std::path::Path) {
    // Collect section names first to satisfy the borrow checker.
    let section_names: Vec<String> = table.keys().cloned().collect();

    for section_name in section_names {
        if let Some(toml::Value::Table(section)) = table.get_mut(&section_name) {
            let secret_keys: Vec<String> = section
                .keys()
                .filter(|k| {
                    let lower = k.to_ascii_lowercase();
                    SECRET_KEY_PATTERNS.iter().any(|p| lower.contains(p))
                })
                .cloned()
                .collect();

            for key in secret_keys {
                section.remove(&key);
                eprintln!(
                    "warning: secret key '{}' found in {} — ignored \
                     (secrets must not be stored in project config files)",
                    key,
                    path.display()
                );
            }
        }
    }
}

// ── Field-by-field overlay merge ──────────────────────────────────────────────
//
// Only overrides fields that differ from the compiled default — this lets a
// lower-priority file set a value that a higher-priority source hasn't touched.

fn merge_into(base: &mut Config, overlay: Config) {
    let d = Config::default();

    if overlay.llm.provider != d.llm.provider {
        base.llm.provider = overlay.llm.provider;
    }
    if overlay.llm.base_url != d.llm.base_url {
        base.llm.base_url = overlay.llm.base_url;
    }
    if overlay.llm.model != d.llm.model {
        base.llm.model = overlay.llm.model;
    }
    if overlay.llm.timeout_secs != d.llm.timeout_secs {
        base.llm.timeout_secs = overlay.llm.timeout_secs;
    }
    if overlay.llm.max_retries != d.llm.max_retries {
        base.llm.max_retries = overlay.llm.max_retries;
    }
    if overlay.llm.num_ctx != d.llm.num_ctx {
        base.llm.num_ctx = overlay.llm.num_ctx;
    }

    if overlay.limits.max_input_bytes != d.limits.max_input_bytes {
        base.limits.max_input_bytes = overlay.limits.max_input_bytes;
    }
    if overlay.limits.max_output_tokens != d.limits.max_output_tokens {
        base.limits.max_output_tokens = overlay.limits.max_output_tokens;
    }

    if overlay.redact.level != d.redact.level {
        base.redact.level = overlay.redact.level;
    }

    if overlay.output.lang != d.output.lang {
        base.output.lang = overlay.output.lang;
    }
    if overlay.output.color != d.output.color {
        base.output.color = overlay.output.color;
    }
    if overlay.output.shell != d.output.shell {
        base.output.shell = overlay.output.shell;
    }
}

// ── Environment variable layer ─────────────────────────────────────────────────

fn apply_env_vars(cfg: &mut Config) {
    if let Ok(v) = std::env::var("LX_PROVIDER") {
        cfg.llm.provider = v;
    }
    if let Ok(v) = std::env::var("LX_BASE_URL") {
        cfg.llm.base_url = v;
    }
    if let Ok(v) = std::env::var("LX_MODEL") {
        cfg.llm.model = v;
    }
    if let Ok(v) = std::env::var("LX_TIMEOUT_SECS") {
        if let Ok(n) = v.parse() {
            cfg.llm.timeout_secs = n;
        } else {
            warn_parse("LX_TIMEOUT_SECS", &v);
        }
    }
    if let Ok(v) = std::env::var("LX_MAX_RETRIES") {
        if let Ok(n) = v.parse() {
            cfg.llm.max_retries = n;
        } else {
            warn_parse("LX_MAX_RETRIES", &v);
        }
    }
    if let Ok(v) = std::env::var("LX_NUM_CTX") {
        if let Ok(n) = v.parse() {
            cfg.llm.num_ctx = n;
        } else {
            warn_parse("LX_NUM_CTX", &v);
        }
    }
    if let Ok(v) = std::env::var("LX_MAX_INPUT_BYTES") {
        if let Ok(n) = v.parse() {
            cfg.limits.max_input_bytes = n;
        } else {
            warn_parse("LX_MAX_INPUT_BYTES", &v);
        }
    }
    if let Ok(v) = std::env::var("LX_MAX_OUTPUT_TOKENS") {
        if let Ok(n) = v.parse() {
            cfg.limits.max_output_tokens = n;
        } else {
            warn_parse("LX_MAX_OUTPUT_TOKENS", &v);
        }
    }
    if let Ok(v) = std::env::var("LX_REDACT_LEVEL") {
        cfg.redact.level = v;
    }
    if let Ok(v) = std::env::var("LX_LANG") {
        cfg.output.lang = v;
    }
    if let Ok(v) = std::env::var("LX_COLOR") {
        cfg.output.color = v;
    }
}

fn warn_parse(var: &str, val: &str) {
    eprintln!("warning: invalid value for {var}='{val}' (ignored)");
}

// ── CLI override layer ─────────────────────────────────────────────────────────

fn apply_overrides(cfg: &mut Config, o: ConfigOverrides) {
    if let Some(v) = o.provider {
        cfg.llm.provider = v;
    }
    if let Some(v) = o.base_url {
        cfg.llm.base_url = v;
    }
    if let Some(v) = o.model {
        cfg.llm.model = v;
    }
    if let Some(v) = o.timeout_secs {
        cfg.llm.timeout_secs = v;
    }
    if let Some(v) = o.max_retries {
        cfg.llm.max_retries = v;
    }
    if let Some(v) = o.max_input_bytes {
        cfg.limits.max_input_bytes = v;
    }
    if let Some(v) = o.max_output_tokens {
        cfg.limits.max_output_tokens = v;
    }
    if let Some(v) = o.redact_level {
        cfg.redact.level = v;
    }
    if let Some(v) = o.lang {
        cfg.output.lang = v;
    }
    if let Some(v) = o.color {
        cfg.output.color = v;
    }
    if let Some(v) = o.shell {
        cfg.output.shell = v;
    }
}

// ── Validation ────────────────────────────────────────────────────────────────

fn validate(cfg: &Config) -> Result<(), LxError> {
    // Provider must be a known value.
    Provider::parse(&cfg.llm.provider)?;

    // Redact level must be standard or strict (not off).
    RedactLevel::parse(&cfg.redact.level)?;

    // Color mode must be a known value.
    ColorMode::parse(&cfg.output.color)?;

    // Sanity-check numeric limits.
    if cfg.llm.timeout_secs == 0 {
        return Err(LxError::ConfigAuth(
            "llm.timeout_secs must be > 0".to_string(),
        ));
    }
    if cfg.limits.max_input_bytes == 0 {
        return Err(LxError::ConfigAuth(
            "limits.max_input_bytes must be > 0".to_string(),
        ));
    }
    if cfg.limits.max_output_tokens == 0 {
        return Err(LxError::ConfigAuth(
            "limits.max_output_tokens must be > 0".to_string(),
        ));
    }
    if cfg.llm.num_ctx == 0 {
        return Err(LxError::ConfigAuth("llm.num_ctx must be > 0".to_string()));
    }

    Ok(())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_valid() {
        validate(&Config::default()).expect("default config must pass validation");
    }

    #[test]
    fn default_values() {
        let cfg = Config::default();
        assert_eq!(cfg.llm.provider, "ollama");
        assert_eq!(cfg.llm.base_url, ""); // empty = use provider default
        assert_eq!(cfg.llm.model, ""); // empty = use provider default
        assert_eq!(cfg.llm.timeout_secs, 30);
        assert_eq!(cfg.llm.max_retries, 3);
        assert_eq!(cfg.llm.num_ctx, 32_768);
        assert_eq!(cfg.limits.max_input_bytes, 524_288);
        assert_eq!(cfg.limits.max_output_tokens, 4096);
        assert_eq!(cfg.redact.level, "standard");
        assert_eq!(cfg.output.lang, "auto");
        assert_eq!(cfg.output.color, "auto");
    }

    #[test]
    fn effective_url_and_model_use_provider_defaults() {
        let cfg = Config::default(); // provider=ollama, base_url="", model=""
        assert_eq!(cfg.effective_base_url(), "http://localhost:11434/v1");
        assert_eq!(cfg.effective_model(), "llama3.1:8b");
    }

    #[test]
    fn explicit_base_url_overrides_provider_default() {
        let mut cfg = Config::default();
        cfg.llm.provider = "anthropic".to_string();
        cfg.llm.base_url = "https://bedrock.example.com/v1".to_string();
        assert_eq!(cfg.effective_base_url(), "https://bedrock.example.com/v1");
    }

    #[test]
    fn explicit_model_overrides_provider_default() {
        let mut cfg = Config::default();
        cfg.llm.provider = "openai".to_string();
        cfg.llm.model = "gpt-4o".to_string();
        assert_eq!(cfg.effective_model(), "gpt-4o");
    }

    #[test]
    fn overrides_applied_last() {
        let overrides = ConfigOverrides {
            model: Some("gpt-4o-mini".to_string()),
            timeout_secs: Some(60),
            ..ConfigOverrides::default()
        };
        let cfg = Config::load_with_overrides(overrides).unwrap();
        assert_eq!(cfg.llm.model, "gpt-4o-mini");
        assert_eq!(cfg.llm.timeout_secs, 60);
    }

    #[test]
    fn invalid_provider_rejected() {
        let mut cfg = Config::default();
        cfg.llm.provider = "unknown-provider".to_string();
        assert!(validate(&cfg).is_err());
    }

    #[test]
    fn invalid_redact_level_rejected() {
        let mut cfg = Config::default();
        cfg.redact.level = "off".to_string();
        assert!(validate(&cfg).is_err());
    }

    #[test]
    fn zero_timeout_rejected() {
        let mut cfg = Config::default();
        cfg.llm.timeout_secs = 0;
        assert!(validate(&cfg).is_err());
    }

    #[test]
    fn toml_roundtrip() {
        let toml_str = r#"
[llm]
provider = "openai"
model = "gpt-4o-mini"
timeout_secs = 15

[limits]
max_input_bytes = 1024

[output]
lang = "de"
color = "always"
"#;
        let cfg: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.llm.provider, "openai");
        assert_eq!(cfg.llm.model, "gpt-4o-mini");
        assert_eq!(cfg.llm.timeout_secs, 15);
        assert_eq!(cfg.limits.max_input_bytes, 1024);
        assert_eq!(cfg.output.lang, "de");
        assert_eq!(cfg.output.color, "always");
    }

    #[test]
    fn secret_key_filter_removes_sensitive_keys() {
        // Write a temp .lx.toml with a key field and verify it is stripped.
        use std::io::Write;
        let dir = std::env::temp_dir();
        let path = dir.join("lx_config_test_secret.toml");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, "[llm]").unwrap();
        writeln!(f, "model = \"gpt-4o\"").unwrap();
        writeln!(f, "api_key = \"sk-secret\"").unwrap();

        let mut cfg = Config::default();
        // filter_secrets = true (project-local path)
        merge_toml_file(&mut cfg, &path, true).unwrap();

        // model should be applied, api_key must be absent (filtered).
        assert_eq!(cfg.llm.model, "gpt-4o");
        assert!(cfg.llm.api_key.is_none());

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn unknown_toml_key_does_not_abort() {
        let toml_str = r#"
[llm]
model = "gpt-4o-mini"

[future_feature]
some_setting = true
"#;
        let dir = std::env::temp_dir();
        let path = dir.join("lx_config_test_unknown.toml");
        std::fs::write(&path, toml_str).unwrap();

        let mut cfg = Config::default();
        // Should not return Err even though [future_feature] is unknown.
        assert!(merge_toml_file(&mut cfg, &path, false).is_ok());
        assert_eq!(cfg.llm.model, "gpt-4o-mini");

        std::fs::remove_file(&path).ok();
    }
}
