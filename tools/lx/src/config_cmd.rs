//! `lx config` — interactive wizard to create/update the user config file.
//!
//! Walks the user through choosing a provider, notes API-key handling (never
//! written to the file), and optionally advanced knobs. Writes the result to
//! the user config path returned by `lx_core::platform::config_dir()`.
//!
//! Design constraints:
//! - Secrets (`api_key`) are `#[serde(skip)]` in `Config` and cannot leak via
//!   serialization. We print instructions for `LX_API_KEY` instead.
//! - All prompts go to **stderr**; only `--print` output goes to **stdout**.
//! - Non-TTY stdin without `--yes` exits with a helpful error.

use lx_config::api_key::provider_key_hint;
use lx_config::{Config, LimitsConfig, LlmConfig, OutputConfig, Provider, RedactConfig};
use lx_core::error::LxError;
use lx_core::{exit, platform};
use std::io::{self, BufRead, Write};

/// All 10 named providers in display order (local first, then cloud).
#[allow(dead_code)]
const PROVIDERS: &[Provider] = &[
    Provider::Ollama,
    Provider::LmStudio,
    Provider::Anthropic,
    Provider::Openai,
    Provider::Gemini,
    Provider::Groq,
    Provider::OpenRouter,
    Provider::Mistral,
    Provider::DeepSeek,
    Provider::Azure,
];

pub struct ConfigArgs {
    /// Accept all defaults non-interactively (writes without prompting).
    pub yes: bool,
    /// Print the resulting TOML to stdout; do not write a file.
    pub print: bool,
    /// Skip the overwrite confirmation when the file already exists.
    pub force: bool,
}

pub fn run(args: &ConfigArgs) -> i32 {
    // Guard: if stdin is not a TTY and we're not in --yes mode, bail early.
    if !args.yes && !platform::is_tty(platform::Fd::Stdin) {
        eprintln!("error: stdin is not a terminal; pass --yes to run non-interactively");
        eprintln!("  hint: lx config --yes [--print]");
        return exit::BAD_USAGE;
    }

    let result = if args.yes {
        Ok(build_defaults())
    } else {
        run_interactive()
    };

    let (config, api_key_instr) = match result {
        Ok(pair) => pair,
        Err(e) => {
            eprintln!("error: {e}");
            return exit::LOGICAL_ERROR;
        }
    };

    // Serialize — api_key and shell are #[serde(skip)] so they cannot appear.
    let toml_str = match render_toml(&config) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: failed to serialize config: {e}");
            return exit::LOGICAL_ERROR;
        }
    };

    if args.print {
        // --print: TOML goes to stdout, instructions to stderr.
        println!("{toml_str}");
        if let Some(instr) = api_key_instr {
            eprintln!("{instr}");
        }
        return exit::SUCCESS;
    }

    // Determine write path.
    let config_path = platform::config_dir().join("config.toml");

    // Check for existing file, confirm overwrite unless --force.
    if config_path.exists() && !args.force {
        eprintln!("\nconfig file already exists: {}", config_path.display());
        // Peek at the existing provider line for context.
        if let Ok(existing) = std::fs::read_to_string(&config_path) {
            if let Some(line) = existing.lines().find(|l| l.trim().starts_with("provider")) {
                eprintln!("  current setting: {}", line.trim());
            }
        }
        eprintln!("\nOverwrite? [y/N] ");
        let _ = io::stderr().flush();
        let mut buf = String::new();
        if io::stdin().lock().read_line(&mut buf).is_err() || !buf.trim().eq_ignore_ascii_case("y")
        {
            eprintln!("Aborted — no changes made.");
            return exit::SUCCESS;
        }
    }

    // Show preview.
    eprintln!("\n--- config.toml preview ---\n{toml_str}--- end ---\n");

    if !args.yes && !args.force {
        eprintln!("Write to {}? [Y/n] ", config_path.display());
        let _ = io::stderr().flush();
        let mut buf = String::new();
        if io::stdin().lock().read_line(&mut buf).is_ok() {
            let answer = buf.trim().to_ascii_lowercase();
            if answer == "n" || answer == "no" {
                eprintln!("Aborted — no changes made.");
                return exit::SUCCESS;
            }
        }
    }

    // Create parent dir and write.
    if let Err(e) = std::fs::create_dir_all(platform::config_dir()) {
        eprintln!("error: could not create config directory: {e}");
        return exit::LOGICAL_ERROR;
    }
    if let Err(e) = std::fs::write(&config_path, &toml_str) {
        eprintln!("error: could not write {}: {e}", config_path.display());
        return exit::LOGICAL_ERROR;
    }

    eprintln!("Config written to {}", config_path.display());

    if let Some(instr) = api_key_instr {
        eprintln!("\n{instr}");
    }

    eprintln!("\nRun `lx model` to verify the configuration.");
    exit::SUCCESS
}

// ── Interactive wizard ────────────────────────────────────────────────────────

fn run_interactive() -> Result<(Config, Option<String>), LxError> {
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let mut cfg = Config::default();

    // ── Step 1: Provider ──────────────────────────────────────────────────────
    eprintln!("\n=== lx config — setup wizard ===\n");
    eprintln!("Provider (choose LLM backend):\n");
    eprintln!("  Local (no API key needed):");
    eprintln!("    [1] ollama       — local inference server (DEFAULT)");
    eprintln!("    [2] lmstudio     — LM Studio");
    eprintln!("  Cloud — Anthropic wire format:");
    eprintln!("    [3] anthropic    — Anthropic Claude API");
    eprintln!("  Cloud — OpenAI-compatible:");
    eprintln!("    [4] openai       — OpenAI API");
    eprintln!("    [5] gemini       — Google Gemini");
    eprintln!("    [6] groq         — Groq cloud inference");
    eprintln!("    [7] openrouter   — OpenRouter aggregator");
    eprintln!("    [8] mistral      — Mistral AI API");
    eprintln!("    [9] deepseek     — DeepSeek API");
    eprintln!("   [10] azure        — Azure OpenAI (requires base_url)");
    eprintln!("   [11] other        — custom OpenAI-compatible endpoint");

    let provider_choice = prompt_line(&mut input, "Provider [1]", "1")?;
    let (provider, is_other) = parse_provider_choice(&provider_choice)?;
    cfg.llm.provider = provider.as_str().to_string();

    // ── Step 2: API key instructions ─────────────────────────────────────────
    let api_key_instr = if !provider.is_local() {
        let hint = provider_key_hint(&provider);
        let env_set = std::env::var("LX_API_KEY")
            .map(|k| !k.trim().is_empty())
            .unwrap_or(false);
        if env_set {
            eprintln!("\nLX_API_KEY is already set in the environment — you're covered.");
        } else {
            eprintln!("\n{hint}");
        }
        // Return as reminder at the end regardless.
        Some(hint)
    } else {
        None
    };

    // ── Step 3: Model ─────────────────────────────────────────────────────────
    let default_model = if is_other {
        String::new()
    } else {
        provider.default_model().to_string()
    };
    let model_prompt_default = if default_model.is_empty() {
        "(required)".to_string()
    } else {
        default_model.clone()
    };
    eprintln!("\nModel override (empty = use provider default):");
    let model_input = loop {
        let v = prompt_line(
            &mut input,
            &format!("Model [{model_prompt_default}]"),
            &default_model,
        )?;
        if is_other && v.is_empty() {
            eprintln!("  A model name is required for custom endpoints.");
            continue;
        }
        break v;
    };
    cfg.llm.model = model_input;

    // ── Step 4: Base URL ──────────────────────────────────────────────────────
    let default_url = if is_other {
        String::new()
    } else {
        provider.default_base_url().to_string()
    };
    let url_required = is_other || matches!(provider, Provider::Azure);
    let url_prompt_default = if default_url.is_empty() {
        "(required)".to_string()
    } else {
        default_url.clone()
    };
    eprintln!("\nBase URL override (empty = use provider default):");
    let url_input = loop {
        let v = prompt_line(
            &mut input,
            &format!("Base URL [{url_prompt_default}]"),
            &default_url,
        )?;
        if url_required && v.is_empty() {
            eprintln!("  A base URL is required for this provider.");
            continue;
        }
        break v;
    };
    cfg.llm.base_url = url_input;

    // ── Step 5: Advanced knobs ────────────────────────────────────────────────
    eprintln!();
    let adv = prompt_line(&mut input, "Configure advanced settings? [y/N]", "n")?;
    if adv.eq_ignore_ascii_case("y") || adv.eq_ignore_ascii_case("yes") {
        cfg.output.lang = prompt_validated(
            &mut input,
            "Output language (BCP-47 or 'auto')",
            "auto",
            |v| {
                if v.is_empty() {
                    Err("cannot be empty".to_string())
                } else {
                    Ok(v.to_string())
                }
            },
        )?;

        cfg.output.color = prompt_validated(
            &mut input,
            "Color output (auto | always | never)",
            "auto",
            |v| {
                lx_config::ColorMode::parse(v)
                    .map(|m| m.as_str().to_string())
                    .map_err(|e| e.to_string())
            },
        )?;

        cfg.redact.level = prompt_validated(
            &mut input,
            "Redact level (standard | strict)",
            "standard",
            |v| {
                lx_config::RedactLevel::parse(v)
                    .map(|l| l.as_str().to_string())
                    .map_err(|e| e.to_string())
            },
        )?;

        cfg.limits.max_input_bytes =
            prompt_validated(&mut input, "Max input bytes", "524288", |v| {
                v.parse::<usize>().map_err(|e| e.to_string())
            })?;

        cfg.limits.max_output_tokens =
            prompt_validated(&mut input, "Max output tokens", "1024", |v| {
                v.parse::<u32>().map_err(|e| e.to_string())
            })?;

        cfg.llm.timeout_secs = prompt_validated(&mut input, "HTTP timeout (seconds)", "30", |v| {
            v.parse::<u64>().map_err(|e| e.to_string())
        })?;

        cfg.llm.max_retries = prompt_validated(&mut input, "Max retries", "3", |v| {
            v.parse::<u32>().map_err(|e| e.to_string())
        })?;
    }

    Ok((cfg, api_key_instr))
}

// ── Non-interactive defaults ──────────────────────────────────────────────────

fn build_defaults() -> (Config, Option<String>) {
    (Config::default(), None)
}

// ── TOML serialization ────────────────────────────────────────────────────────

fn render_toml(config: &Config) -> Result<String, String> {
    // Serialize via the toml crate; api_key and shell are #[serde(skip)].
    let body = toml::to_string_pretty(config).map_err(|e| e.to_string())?;
    let header = "# LX Coreutils configuration file\n\
                  # Generated by: lx config\n\
                  #\n\
                  # Location:\n\
                  #   Linux/macOS: $XDG_CONFIG_HOME/lx/config.toml  (default: ~/.config/lx/config.toml)\n\
                  #   Windows:     %APPDATA%\\lx\\config.toml\n\
                  #\n\
                  # All values are optional; missing values fall back to the compiled default.\n\
                  # Environment variables (LX_*) override this file.\n\
                  #\n\
                  # NEVER store API keys here. Use the environment variable:\n\
                  #   export LX_API_KEY=sk-...   (Linux/macOS)\n\
                  #   $env:LX_API_KEY='sk-...'   (PowerShell)\n\
                  # or the OS credential store — see: lx config --help\n\n";
    Ok(format!("{header}{body}"))
}

// ── Prompt helpers ────────────────────────────────────────────────────────────

/// Print a prompt to stderr, read a line from stdin.
/// Returns `default_val` if the user just presses Enter.
fn prompt_line(
    input: &mut impl BufRead,
    prompt: &str,
    default_val: &str,
) -> Result<String, LxError> {
    eprint!("{prompt}: ");
    let _ = io::stderr().flush();
    let mut buf = String::new();
    input
        .read_line(&mut buf)
        .map_err(|e| LxError::LogicalError(format!("read error: {e}")))?;
    let trimmed = buf.trim().to_string();
    if trimmed.is_empty() {
        Ok(default_val.to_string())
    } else {
        Ok(trimmed)
    }
}

/// Prompt until the user enters a value that validates. Returns the
/// parsed/normalized value. Re-prompts with an inline error on bad input.
fn prompt_validated<T>(
    input: &mut impl BufRead,
    prompt: &str,
    default_val: &str,
    validate: impl Fn(&str) -> Result<T, String>,
) -> Result<T, LxError> {
    loop {
        let raw = prompt_line(input, &format!("{prompt} [{default_val}]"), default_val)?;
        match validate(&raw) {
            Ok(v) => return Ok(v),
            Err(msg) => eprintln!("  invalid input: {msg}"),
        }
    }
}

/// Map a user's menu number or provider name to a `(Provider, is_other)` pair.
fn parse_provider_choice(choice: &str) -> Result<(Provider, bool), LxError> {
    // Accept numeric menu choices 1–11.
    match choice.trim() {
        "1" => return Ok((Provider::Ollama, false)),
        "2" => return Ok((Provider::LmStudio, false)),
        "3" => return Ok((Provider::Anthropic, false)),
        "4" => return Ok((Provider::Openai, false)),
        "5" => return Ok((Provider::Gemini, false)),
        "6" => return Ok((Provider::Groq, false)),
        "7" => return Ok((Provider::OpenRouter, false)),
        "8" => return Ok((Provider::Mistral, false)),
        "9" => return Ok((Provider::DeepSeek, false)),
        "10" => return Ok((Provider::Azure, false)),
        "11" | "other" => return Ok((Provider::Openai, true)),
        _ => {}
    }
    // Accept named provider strings (e.g. "anthropic", "openai").
    match Provider::parse(choice) {
        Ok(p) => Ok((p, false)),
        Err(_) => Err(LxError::LogicalError(format!(
            "invalid choice '{choice}'; enter a number 1–11 or a provider name"
        ))),
    }
}

// ── Re-export types that tests need ──────────────────────────────────────────

/// Build a `Config` from raw field values. Used by tests to verify serialization
/// without running the interactive wizard.
#[allow(dead_code, clippy::too_many_arguments)]
pub fn build_config(
    provider: &str,
    model: &str,
    base_url: &str,
    lang: &str,
    color: &str,
    redact_level: &str,
    max_input_bytes: usize,
    max_output_tokens: u32,
    timeout_secs: u64,
    max_retries: u32,
) -> Config {
    Config {
        llm: LlmConfig {
            provider: provider.to_string(),
            base_url: base_url.to_string(),
            model: model.to_string(),
            timeout_secs,
            max_retries,
            api_key: None,
        },
        limits: LimitsConfig {
            max_input_bytes,
            max_output_tokens,
        },
        redact: RedactConfig {
            level: redact_level.to_string(),
        },
        output: OutputConfig {
            lang: lang.to_string(),
            color: color.to_string(),
            shell: "auto".to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeric_choices_map_to_correct_providers() {
        assert_eq!(parse_provider_choice("1").unwrap().0, Provider::Ollama);
        assert_eq!(parse_provider_choice("3").unwrap().0, Provider::Anthropic);
        assert_eq!(parse_provider_choice("10").unwrap().0, Provider::Azure);
        let (p, is_other) = parse_provider_choice("11").unwrap();
        assert_eq!(p, Provider::Openai);
        assert!(is_other);
    }

    #[test]
    fn named_provider_strings_accepted() {
        assert_eq!(
            parse_provider_choice("anthropic").unwrap().0,
            Provider::Anthropic
        );
        assert_eq!(parse_provider_choice("gemini").unwrap().0, Provider::Gemini);
    }

    #[test]
    fn invalid_choice_returns_error() {
        assert!(parse_provider_choice("99").is_err());
        assert!(parse_provider_choice("notaprovider").is_err());
    }

    #[test]
    fn render_toml_contains_no_api_key() {
        let cfg = build_config(
            "openai", "gpt-4o", "", "auto", "auto", "standard", 524288, 1024, 30, 3,
        );
        let toml_str = render_toml(&cfg).unwrap();
        assert!(
            !toml_str.contains("api_key"),
            "api_key must never appear in serialized config, got:\n{toml_str}"
        );
        assert!(
            !toml_str.contains("shell"),
            "shell must never appear in serialized config"
        );
    }

    #[test]
    fn render_toml_roundtrips() {
        let cfg = build_config(
            "anthropic",
            "claude-sonnet-4-6",
            "",
            "de",
            "always",
            "strict",
            1048576,
            2048,
            60,
            5,
        );
        let toml_str = render_toml(&cfg).unwrap();
        // Round-trip: re-parse must produce equivalent values.
        let parsed: Config = toml::from_str(
            &toml_str
                .lines()
                .filter(|l| !l.starts_with('#'))
                .collect::<Vec<_>>()
                .join("\n"),
        )
        .expect("round-trip parse failed");
        assert_eq!(parsed.llm.provider, "anthropic");
        assert_eq!(parsed.llm.model, "claude-sonnet-4-6");
        assert_eq!(parsed.output.lang, "de");
        assert_eq!(parsed.output.color, "always");
        assert_eq!(parsed.redact.level, "strict");
        assert_eq!(parsed.limits.max_input_bytes, 1048576);
    }

    #[test]
    fn render_toml_contains_chosen_provider() {
        let cfg = build_config(
            "groq", "", "", "auto", "auto", "standard", 524288, 1024, 30, 3,
        );
        let toml_str = render_toml(&cfg).unwrap();
        assert!(
            toml_str.contains("groq"),
            "provider name must appear in TOML"
        );
    }

    #[test]
    fn providers_array_covers_all_10() {
        assert_eq!(PROVIDERS.len(), 10);
        assert!(PROVIDERS.contains(&Provider::Ollama));
        assert!(PROVIDERS.contains(&Provider::Azure));
    }
}
