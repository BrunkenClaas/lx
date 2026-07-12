#![forbid(unsafe_code)]

use clap::Parser;
use lx_config::Config;
use lx_core::{error::print_error, exit};
use lx_redact::RedactLevel;
use std::path::PathBuf;
use std::process;

mod run;
use run::run;

fn version_string() -> String {
    lx_core::version::build_version_string("lxredact", env!("CARGO_PKG_VERSION"))
}

#[derive(Parser)]
#[command(
    name = "lxredact",
    about = "Mask secrets and PII in a data stream",
    disable_version_flag = true
)]
struct Cli {
    /// Output as JSON (redaction summary with count and item list)
    #[arg(long)]
    json: bool,

    /// Disable ANSI colours and formatting
    #[arg(long)]
    plain: bool,

    /// Show what would happen without actually performing redaction
    #[arg(long)]
    dry_run: bool,

    /// Suppress diagnostic messages on stderr
    #[arg(short, long)]
    quiet: bool,

    /// Output language for --explain mode (BCP-47, e.g. 'en', 'de', 'fr')
    #[arg(long, default_value = "auto")]
    lang: String,

    /// Show verbose diagnostics on stderr
    #[arg(long)]
    verbose: bool,

    /// Maximum bytes to read from stdin
    #[arg(long)]
    max_input_bytes: Option<usize>,

    /// Read input from file instead of stdin
    #[arg(long, value_name = "PATH")]
    file: Option<PathBuf>,

    /// Disable redaction (NOT RECOMMENDED — prominent warning is printed)
    #[arg(long)]
    no_redact: bool,

    /// Use aggressive redaction: masks more service-specific token formats and also
    /// applies strict PII masking (IPv4, hostnames, home-directory paths)
    #[arg(long)]
    strict: bool,

    /// Use LLM to explain what was redacted (never sends actual secret values)
    #[arg(long)]
    explain: bool,

    /// Use LLM to anonymise PII by replacing names/orgs/locations with role placeholders
    #[arg(long)]
    anon: bool,

    /// Print version information
    #[arg(short = 'V', long = "version")]
    version: bool,
}

fn main() {
    let cli = Cli::parse();

    lx_core::platform::enable_ansi();
    lx_core::output::set_quiet(cli.quiet);

    if cli.version {
        println!("{}", version_string());
        process::exit(exit::SUCCESS);
    }

    if cli.no_redact {
        eprintln!(
            "WARNING: --no-redact is set. \
             Secrets and PII will NOT be masked. \
             Use only in controlled environments."
        );
    }

    let mut config = Config::load().unwrap_or_else(|e| {
        print_error(&e, cli.json);
        process::exit(exit::LOGICAL_ERROR);
    });

    if cli.lang != "auto" {
        config.output.lang = cli.lang.clone();
    }
    if config.output.lang == "auto" {
        config.output.lang = lx_core::locale::detect_lang();
    }

    if cli.verbose {
        eprintln!("[verbose] redact strict: {}", cli.strict);
        eprintln!("[verbose] explain: {}", cli.explain);
        if cli.explain {
            eprintln!(
                "[verbose] config: model={} provider={} lang={}",
                config.effective_model(),
                config.llm.provider,
                config.output.lang
            );
        }
    }

    let max = cli.max_input_bytes.unwrap_or(config.limits.max_input_bytes);
    let input = lx_core::io::resolve_input(cli.file.as_deref(), max).unwrap_or_else(|e| {
        print_error(&e, cli.json);
        process::exit(e.exit_code());
    });

    if cli.dry_run {
        if !cli.quiet {
            eprintln!("[dry-run] input ({} bytes)", input.len());
            eprintln!("[dry-run] redact strict: {}", cli.strict);
        }
        if !cli.quiet {
            eprintln!("[dry-run] system prompt:");
            eprintln!(
                "{}",
                lx_llm::inject_lang(run::SYSTEM_TEMPLATE, &config.output.lang)
            );
        }
        process::exit(exit::SUCCESS);
    }

    // Honour --no-redact: pass through unchanged (no LLM call, no redaction).
    if cli.no_redact {
        if cli.json {
            let out = serde_json::json!({
                "redacted_text": input,
                "redacted_count": 0,
                "items": []
            });
            println!("{}", serde_json::to_string_pretty(&out).unwrap());
        } else {
            print!("{}", input);
        }
        process::exit(exit::SUCCESS);
    }

    let level = if cli.strict {
        RedactLevel::Aggressive
    } else {
        RedactLevel::Standard
    };

    // Build an LLM client only if --explain or --anon is requested; otherwise use a
    // no-op placeholder so the pure run() signature stays uniform.
    let client: Box<dyn lx_llm::LlmClient> = if cli.explain || cli.anon {
        lx_llm::client_from_config(&config, cli.verbose).unwrap_or_else(|e| {
            print_error(&e, cli.json);
            process::exit(e.exit_code());
        })
    } else {
        // Provide a dummy client that panics if called — it will never be
        // invoked because explain=false prevents any LLM call in run().
        Box::new(NoopClient)
    };

    match run(
        &input,
        level,
        cli.explain,
        cli.anon,
        &config,
        client.as_ref(),
    ) {
        Ok(output) => {
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&output).unwrap());
            } else {
                // Plain mode: anonymised text takes precedence over redacted text when --anon.
                if let Some(ref anon) = output.anon {
                    print!("{}", anon.text);
                    if lx_core::output::show_narration(cli.quiet, cli.verbose) {
                        eprintln!(
                            "# lxredact --anon: {} replacement(s) made",
                            anon.replacements.len()
                        );
                    }
                } else {
                    // Plain mode: redacted text → stdout (pipe-safe result)
                    print!("{}", output.to_plain());

                    if lx_core::output::show_narration(cli.quiet, cli.verbose) {
                        if output.redacted_count == 0 {
                            eprintln!("# lxredact: no secrets detected");
                        } else {
                            eprintln!(
                                "# lxredact: {} secret{} redacted",
                                output.redacted_count,
                                if output.redacted_count == 1 { "" } else { "s" }
                            );
                            for item in &output.items {
                                eprintln!("#   {} at {}", item.kind, item.location);
                            }
                        }
                        if let Some(ref ex) = output.explanation {
                            eprintln!("# explain: {}", ex.summary);
                            eprintln!("#   risk: {}", ex.risk_level);
                            if !ex.notes.is_empty() {
                                eprintln!("#   notes: {}", ex.notes);
                            }
                        }
                    }
                }
            }
            process::exit(exit::SUCCESS);
        }
        Err(e) => {
            print_error(&e, cli.json);
            process::exit(e.exit_code());
        }
    }
}

// ── NoopClient ────────────────────────────────────────────────────────────────

/// A placeholder LLM client used when `--explain` is NOT requested.
/// `run()` never calls `complete()` in that case, so this never panics
/// in normal operation.
struct NoopClient;

impl lx_llm::LlmClient for NoopClient {
    fn complete(&self, _req: &lx_llm::Request<'_>) -> Result<lx_llm::Response, lx_llm::LlmError> {
        panic!("NoopClient::complete called — this is a bug; --explain was not set");
    }
}
