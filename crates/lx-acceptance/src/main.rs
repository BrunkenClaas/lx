#![forbid(unsafe_code)]

//! Extended, data-driven acceptance harness for LX Coreutils (dev-only).
//!
//! Runs many intents per tool from `intents/intents.toml`, grading each tool's
//! `--json` output with deterministic assertions (and optional execution
//! oracles). Unlike the human-graded smoke test in `acceptance/run.{sh,ps1}`,
//! this is self-grading and CI-gateable: it exits non-zero if any intent fails.
//!
//! Usage:
//!   cargo run -p lx-acceptance --release -- --yes
//!   cargo run -p lx-acceptance --release -- --yes --tool lxdockercmd
//!   cargo run -p lx-acceptance --release -- --yes --target linux
//!   cargo run -p lx-acceptance --release -- --yes --extended
//!   cargo run -p lx-acceptance --release -- --yes --judge \
//!     --judge-provider anthropic --judge-model claude-opus-4-8
//!
//! Or after building: target/release/lx-acceptance --yes
//!
//! Judge model configuration (required when using --judge):
//!   LX_JUDGE_PROVIDER  provider name (e.g. "anthropic", "openai")
//!   LX_JUDGE_MODEL     model identifier (e.g. "claude-opus-4-8")
//!   LX_JUDGE_BASE_URL  optional base URL override
//!   LX_JUDGE_API_KEY   API key for cloud providers (falls back to LX_API_KEY)

mod engine;
mod fewshot;
mod fieldmap;
mod intents;
mod judge;
mod oracle;
mod report;

use clap::Parser;
use std::path::PathBuf;
use std::process::Command;

use judge::JudgeConfig;
use report::{Report, ReportHeader};

/// Path to `acceptance/fixtures`, resolved relative to the workspace root.
pub fn fixtures_dir() -> PathBuf {
    // CARGO_MANIFEST_DIR = <root>/crates/lx-acceptance
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop(); // crates
    p.pop(); // root
    p.push("acceptance");
    p.push("fixtures");
    p
}

/// Path to the bundled intents file.
fn intents_path() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("intents");
    p.push("intents.toml");
    p
}

#[derive(Parser, Debug)]
#[command(
    name = "lx-acceptance",
    about = "Extended, data-driven acceptance harness for LX Coreutils (dev-only)"
)]
struct Args {
    /// Skip the confirmation prompt and run immediately (e.g. in CI).
    /// Without this flag, the harness asks for confirmation before making LLM calls.
    #[arg(long, short = 'y')]
    yes: bool,
    /// Only run intents for this tool (repeatable). Mirrors run.sh TOOLS filter.
    #[arg(long)]
    tool: Vec<String>,
    /// Target OS substituted into {target} placeholders in intents (advisory;
    /// each intent still carries its own --target where relevant).
    #[arg(long, default_value = "auto")]
    target: String,
    /// Also run the advisory LLM sanity-gate over prose intents (non-gating).
    /// Requires --judge-model and --judge-provider (or LX_JUDGE_MODEL /
    /// LX_JUDGE_PROVIDER). For cloud providers also set LX_JUDGE_API_KEY.
    #[arg(long)]
    judge: bool,
    /// Cap the number of prose intents judged (0 = all).
    #[arg(long, default_value_t = 0)]
    judge_limit: usize,
    /// Model to use for judging (overrides LX_JUDGE_MODEL). Required with --judge.
    #[arg(long)]
    judge_model: Option<String>,
    /// Provider to use for judging (overrides LX_JUDGE_PROVIDER). Required with --judge.
    #[arg(long)]
    judge_provider: Option<String>,
    /// Also run intents tagged `extended = true` (broader, slower coverage).
    /// In normal mode these intents are skipped to save cost and time.
    #[arg(long)]
    extended: bool,
}

fn main() {
    // Force English output so assertions are language-stable and model-comparable,
    // exactly like acceptance/run.sh. Child tool processes inherit this env.
    std::env::set_var("LX_LANG", "en");

    let args = Args::parse();
    if !args.yes {
        let tool_hint = if args.tool.is_empty() {
            "all tools".to_string()
        } else {
            args.tool.join(", ")
        };
        eprint!("lxacceptance: this will make real LLM calls for {tool_hint}. Continue? [y/N] ");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).ok();
        if !input.trim().eq_ignore_ascii_case("y") {
            eprintln!("Aborted. Pass --yes / -y to skip this prompt (e.g. in CI).");
            std::process::exit(0);
        }
    }

    // Load + validate intents.
    let path = intents_path();
    let src = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: cannot read {}: {e}", path.display());
            std::process::exit(1);
        }
    };
    let mut all = match intents::parse(&src) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    };

    // Tool filter.
    if !args.tool.is_empty() {
        all.retain(|i| args.tool.iter().any(|t| t == &i.tool));
        if all.is_empty() {
            eprintln!("error: no intents match --tool {:?}", args.tool);
            std::process::exit(2);
        }
    }

    // Extended filter: drop extended-only intents unless --extended is set.
    if !args.extended {
        all.retain(|i| !i.extended);
    }

    // Resolve judge config early; fail fast before any LLM calls if misconfigured.
    let judge_config = if args.judge {
        let jm = args
            .judge_model
            .clone()
            .or_else(|| std::env::var("LX_JUDGE_MODEL").ok())
            .filter(|s| !s.is_empty());
        let jp = args
            .judge_provider
            .clone()
            .or_else(|| std::env::var("LX_JUDGE_PROVIDER").ok())
            .filter(|s| !s.is_empty());
        match (jm, jp) {
            (Some(model), Some(provider)) => {
                let base_url = std::env::var("LX_JUDGE_BASE_URL").unwrap_or_default();
                let api_key = std::env::var("LX_JUDGE_API_KEY").ok();
                Some(JudgeConfig {
                    model,
                    provider,
                    base_url,
                    api_key,
                })
            }
            _ => {
                eprintln!(
                    "error: --judge requires a dedicated judge model. \
                     Set --judge-model and --judge-provider, or \
                     LX_JUDGE_MODEL / LX_JUDGE_PROVIDER \
                     (and LX_JUDGE_API_KEY for cloud providers)."
                );
                std::process::exit(2);
            }
        }
    } else {
        None
    };

    let target = resolve_target(&args.target);
    let (model, provider) = resolve_model_provider();

    eprintln!(
        "running {} intents (target={target}, model={model})...",
        all.len()
    );

    let mut rep = Report::default();
    for intent in &all {
        let outcome = engine::evaluate(intent);
        rep.push(&intent.tool, &intent.name, outcome);
    }

    let exit = rep.render(&ReportHeader {
        model,
        provider,
        target,
    });

    // Optional advisory judge — prints its own section, never changes `exit`.
    if args.judge {
        judge::run_judge(&all, args.judge_limit, &judge_config.unwrap());
    }

    std::process::exit(exit);
}

/// Resolves "auto" to the host OS string; otherwise passes through.
fn resolve_target(t: &str) -> String {
    if t != "auto" {
        return t.to_string();
    }
    if cfg!(target_os = "windows") {
        "windows".into()
    } else if cfg!(target_os = "macos") {
        "macos".into()
    } else {
        "linux".into()
    }
}

/// Resolves the effective model/provider by shelling to the umbrella binary
/// `lx model --no-verify --json` (renamed from `lxsuite` in the 2026-06-08
/// rebrand). Falls back to "unknown" if unavailable — this only labels the
/// report and never affects grading.
fn resolve_model_provider() -> (String, String) {
    let bin = lx_binary_path();
    let out = Command::new(&bin)
        .args(["model", "--no-verify", "--json"])
        .output();
    if let Ok(out) = out {
        if out.status.success() {
            if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&out.stdout) {
                let model = v.get("model").and_then(|m| m.as_str()).unwrap_or("unknown");
                let provider = v
                    .get("provider")
                    .and_then(|p| p.as_str())
                    .unwrap_or("unknown");
                return (model.to_string(), provider.to_string());
            }
        }
    }
    ("unknown".into(), "unknown".into())
}

fn lx_binary_path() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop(); // crates
    p.pop(); // root
    p.push("target");
    p.push("release");
    p.push(if cfg!(windows) { "lx.exe" } else { "lx" });
    p
}
