#![forbid(unsafe_code)]

use clap::Parser;
use lx_config::Config;
use lx_core::{error::print_error, exit};
use std::path::PathBuf;
use std::process;

mod run;

fn version_string() -> String {
    lx_core::version::build_version_string("lxcsv", env!("CARGO_PKG_VERSION"))
}

#[derive(Parser)]
#[command(
    name = "lxcsv",
    about = "Answer questions about CSV data",
    disable_version_flag = true
)]
struct Cli {
    /// Question to answer about the CSV data
    question: Option<String>,

    /// CSV file to analyse
    #[arg(long, value_name = "PATH")]
    file: Option<PathBuf>,

    /// Root directory for fsbound checks (default: current directory)
    #[arg(long, value_name = "PATH")]
    root: Option<PathBuf>,

    /// Output as JSON
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

    /// Maximum bytes to read from stdin or file
    #[arg(long)]
    max_input_bytes: Option<usize>,

    /// Disable secret redaction (NOT recommended — CSV data will reach the LLM provider unmasked)
    #[arg(long)]
    no_redact: bool,

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

    // --no-redact: prominent warning.
    if cli.no_redact && !cli.quiet {
        eprintln!(
            "warning: --no-redact is set. Sensitive data in your CSV will be sent to \
             the LLM provider unmasked. Proceed only if you have audited the content."
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
        eprintln!(
            "[verbose] config: model={} provider={} lang={} redact={}",
            config.effective_model(),
            config.llm.provider,
            config.output.lang,
            if cli.no_redact { "off" } else { "on" }
        );
    }

    // Require the question positional argument.
    let question = match cli.question {
        Some(q) => q,
        None => {
            let e = lx_core::error::LxError::BadUsage(
                "missing required argument: <question>".to_string(),
            );
            print_error(&e, cli.json);
            process::exit(exit::BAD_USAGE);
        }
    };

    let max_bytes = cli.max_input_bytes.unwrap_or(config.limits.max_input_bytes);

    // Read CSV content — from --file (with fsbound) or stdin.
    let csv_content = if let Some(ref file_path) = cli.file {
        // Determine the fsbound root.
        let root = if let Some(ref r) = cli.root {
            r.clone()
        } else {
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
        };

        // fsbound check: resolve the file path and verify it stays within root.
        let canonical_file = match std::fs::canonicalize(file_path) {
            Ok(p) => p,
            Err(e) => {
                let err = lx_core::error::LxError::BadUsage(format!(
                    "cannot resolve {}: {e}",
                    file_path.display()
                ));
                print_error(&err, cli.json);
                process::exit(exit::BAD_USAGE);
            }
        };
        let canonical_root = match std::fs::canonicalize(&root) {
            Ok(p) => p,
            Err(e) => {
                let err = lx_core::error::LxError::BadUsage(format!(
                    "cannot resolve root {}: {e}",
                    root.display()
                ));
                print_error(&err, cli.json);
                process::exit(exit::BAD_USAGE);
            }
        };
        if !canonical_file.starts_with(&canonical_root) {
            let err = lx_core::error::LxError::SecurityAbort(format!(
                "path {} escapes allowed root {}",
                canonical_file.display(),
                canonical_root.display()
            ));
            print_error(&err, cli.json);
            process::exit(exit::SECURITY_ABORT);
        }

        lx_core::io::read_file_limited(file_path, max_bytes).unwrap_or_else(|e| {
            print_error(&e, cli.json);
            process::exit(e.exit_code());
        })
    } else {
        lx_core::io::resolve_input(None, max_bytes).unwrap_or_else(|e| {
            print_error(&e, cli.json);
            process::exit(e.exit_code());
        })
    };

    if csv_content.trim().is_empty() {
        let e = lx_core::error::LxError::BadUsage(
            "no CSV data provided; use --file <PATH> or pipe CSV to stdin".to_string(),
        );
        print_error(&e, cli.json);
        process::exit(exit::BAD_USAGE);
    }

    // --dry-run: show what would be sent to the LLM, then exit.
    if cli.dry_run {
        if !cli.quiet {
            eprintln!(
                "[dry-run] CSV ({} bytes) + question: {}",
                csv_content.len(),
                question.trim()
            );
            eprintln!(
                "[dry-run] redaction: {}",
                if cli.no_redact { "disabled" } else { "enabled" }
            );
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

    let result = if cli.no_redact {
        run::run_no_redact(&csv_content, &question, &config, client.as_ref())
    } else {
        run::run(&csv_content, &question, &config, client.as_ref())
    };

    match result {
        Ok(output) => {
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&output).unwrap());
            } else {
                // stdout: answer only — pipe safe.
                println!("{}", output.to_plain());
                if lx_core::output::show_narration(cli.quiet, cli.verbose) {
                    eprintln!("# rows used: {}", output.used_rows);
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
