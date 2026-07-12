#![forbid(unsafe_code)]

use clap::Parser;
use lx_config::Config;
use lx_core::{error::print_error, exit};
use std::path::PathBuf;
use std::process;

mod run;

fn version_string() -> String {
    lx_core::version::build_version_string("lxjson", env!("CARGO_PKG_VERSION"))
}

#[derive(Parser)]
#[command(
    name = "lxjson",
    about = "Repair or clean malformed JSON",
    disable_version_flag = true
)]
struct Cli {
    /// Broken JSON to repair (reads from stdin if omitted)
    input: Option<String>,

    /// Output as JSON envelope (includes method and changes)
    #[arg(long)]
    json: bool,

    /// Disable ANSI colours and formatting
    #[arg(long)]
    plain: bool,

    /// Show what would be sent to the LLM without actually sending it
    #[arg(long)]
    dry_run: bool,

    /// Suppress diagnostic messages on stderr
    #[arg(short, long)]
    quiet: bool,

    /// Output language (BCP-47, e.g. 'en', 'de', 'fr')
    #[arg(long, default_value = "auto")]
    lang: String,

    /// Show token usage and estimated cost on stderr
    #[arg(long)]
    verbose: bool,

    /// Maximum bytes to read from stdin (0 = no limit; truncation disabled by default)
    #[arg(long)]
    max_input_bytes: Option<usize>,

    /// Read input from file instead of stdin
    #[arg(long, value_name = "PATH")]
    file: Option<PathBuf>,

    /// Print version information
    #[arg(short = 'V', long = "version")]
    version: bool,
}

fn main() {
    let cli = Cli::parse();

    lx_core::platform::enable_ansi();
    lx_core::output::set_quiet(cli.quiet);

    // --version: canonical suite-aware format, then exit 0.
    if cli.version {
        println!("{}", version_string());
        process::exit(exit::SUCCESS);
    }

    let mut config = Config::load().unwrap_or_else(|e| {
        print_error(&e, cli.json);
        process::exit(exit::LOGICAL_ERROR);
    });

    // CLI flags override config.
    if cli.lang != "auto" {
        config.output.lang = cli.lang.clone();
    }
    if config.output.lang == "auto" {
        config.output.lang = lx_core::locale::detect_lang();
    }

    if cli.verbose {
        eprintln!(
            "[verbose] config: model={} provider={} lang={}",
            config.effective_model(),
            config.llm.provider,
            config.output.lang
        );
    }

    // Collect input: positional arg > --file > stdin.
    // truncation = false: if input exceeds max_input_bytes, abort (do NOT truncate).
    let input = if let Some(s) = cli.input {
        s
    } else {
        let max = cli.max_input_bytes.unwrap_or(config.limits.max_input_bytes);
        let raw = lx_core::io::resolve_input(cli.file.as_deref(), max).unwrap_or_else(|e| {
            print_error(&e, cli.json);
            process::exit(e.exit_code());
        });

        // truncation = false: abort if the raw bytes hit the limit.
        // resolve_input truncates silently; we detect it by checking byte length.
        if raw.len() >= max {
            let e = lx_core::error::LxError::BadUsage("input too large".to_string());
            print_error(&e, cli.json);
            if cli.json {
                // hint already printed by print_error, but for JSON mode add it.
            } else {
                eprintln!(
                    "  hint: increase --max-input-bytes (current limit: {} bytes)",
                    max
                );
            }
            process::exit(exit::BAD_USAGE);
        }

        raw
    };

    if input.trim().is_empty() {
        let e = lx_core::error::LxError::BadUsage("no input provided".to_string());
        print_error(&e, cli.json);
        process::exit(exit::BAD_USAGE);
    }

    // --dry-run: show what would be sent, then exit.
    if cli.dry_run {
        if !cli.quiet {
            eprintln!("[dry-run] input ({} bytes):", input.len());
            eprintln!("{}", input.trim());
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

    let client = lx_llm::client_from_config(&config, cli.verbose).unwrap_or_else(|e| {
        print_error(&e, cli.json);
        process::exit(e.exit_code());
    });

    match run::run(&input, &config, client.as_ref()) {
        Ok(output) => {
            if cli.json {
                // Full envelope (json + method + changes) → stdout
                println!("{}", serde_json::to_string_pretty(&output).unwrap());
            } else {
                // Repaired JSON only → stdout (pipe-safe)
                println!("{}", output.to_plain());
                if lx_core::output::show_narration(cli.quiet, cli.verbose)
                    && !output.changes.is_empty()
                {
                    for change in &output.changes {
                        eprintln!("# fixed: {change}");
                    }
                }
                if lx_core::output::show_narration(cli.quiet, cli.verbose)
                    && output.changes.is_empty()
                {
                    eprintln!("# input was already valid JSON");
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
