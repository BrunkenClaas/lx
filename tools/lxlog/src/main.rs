#![forbid(unsafe_code)]

use clap::Parser;
use lx_config::Config;
use lx_core::{error::print_error, exit};
use std::path::PathBuf;
use std::process;

mod run;

fn version_string() -> String {
    lx_core::version::build_version_string("lxlog", env!("CARGO_PKG_VERSION"))
}

#[derive(Parser)]
#[command(
    name = "lxlog",
    about = "Analyse logs and detect anomalies (stdin or --file)",
    disable_version_flag = true
)]
struct Cli {
    /// Output as JSON
    #[arg(long)]
    json: bool,

    /// Disable ANSI colours and formatting
    #[arg(long)]
    plain: bool,

    /// Show what would be sent to the LLM, then exit
    #[arg(long)]
    dry_run: bool,

    /// Suppress diagnostic messages on stderr
    #[arg(short, long)]
    quiet: bool,

    /// Output language (BCP-47, e.g. 'en', 'de', 'fr')
    #[arg(long, default_value = "auto")]
    lang: String,

    /// Print config summary, token counts, and retry diagnostics to stderr
    #[arg(long)]
    verbose: bool,

    /// Maximum bytes to read from stdin
    #[arg(long)]
    max_input_bytes: Option<usize>,

    /// Disable secret redaction (NOT recommended — logs may contain credentials or PII)
    #[arg(long)]
    no_redact: bool,

    /// Read input from this file instead of stdin
    #[arg(long, value_name = "PATH")]
    file: Option<PathBuf>,

    /// Restrict file access to this directory (fsbound)
    #[arg(long, value_name = "DIR")]
    path: Option<PathBuf>,

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

    // --no-redact: prominent warning.
    if cli.no_redact && !cli.quiet {
        eprintln!(
            "warning: --no-redact is set. Log data containing credentials, IP addresses, \
             or PII will be sent to the LLM provider unmasked. Proceed only if you have \
             audited the log content."
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

    // fsbound: if --path is given, verify --file is within that root.
    if let Some(ref root_arg) = cli.path {
        match std::fs::canonicalize(root_arg) {
            Ok(canonical_root) => {
                if let Some(ref file_arg) = cli.file {
                    match std::fs::canonicalize(file_arg) {
                        Ok(canonical_file) => {
                            if !canonical_file.starts_with(&canonical_root) {
                                let err = lx_core::error::LxError::SecurityAbort(format!(
                                    "file '{}' is outside the allowed path '{}'",
                                    file_arg.display(),
                                    root_arg.display()
                                ));
                                print_error(&err, cli.json);
                                process::exit(exit::SECURITY_ABORT);
                            }
                        }
                        Err(e) => {
                            let err = lx_core::error::LxError::LogicalError(format!(
                                "cannot resolve --file path: {e}"
                            ));
                            print_error(&err, cli.json);
                            process::exit(exit::LOGICAL_ERROR);
                        }
                    }
                }
            }
            Err(e) => {
                let err = lx_core::error::LxError::LogicalError(format!(
                    "cannot resolve --path directory: {e}"
                ));
                print_error(&err, cli.json);
                process::exit(exit::LOGICAL_ERROR);
            }
        }
    }

    let max = cli.max_input_bytes.unwrap_or(config.limits.max_input_bytes);
    let log_content = lx_core::io::resolve_input(cli.file.as_deref(), max).unwrap_or_else(|e| {
        print_error(&e, cli.json);
        process::exit(e.exit_code());
    });

    if log_content.trim().is_empty() {
        let e = lx_core::error::LxError::BadUsage(
            "no log content provided; pipe a log file or use --file <path>".to_string(),
        );
        print_error(&e, cli.json);
        process::exit(exit::BAD_USAGE);
    }

    // --dry-run: show the aggregated+redacted content that would go to the LLM, then exit.
    if cli.dry_run {
        let level = lx_redact::RedactLevel::parse(&config.redact.level);
        let display = if cli.no_redact {
            log_content.clone()
        } else {
            match lx_redact::redact(&log_content, level) {
                Ok(r) => r,
                Err(e) => {
                    let lx_err =
                        lx_core::error::LxError::SecurityAbort(format!("redaction failed: {e}"));
                    print_error(&lx_err, cli.json);
                    process::exit(exit::SECURITY_ABORT);
                }
            }
        };
        let (aggregated, used_lines, capped) = run::aggregate_logs(&display);
        if !cli.quiet {
            if capped {
                eprintln!(
                    "warning: log too large — only {} lines sent to the LLM; results may be incomplete",
                    used_lines
                );
            }
            eprintln!(
                "[dry-run] aggregated log ({} bytes, {}) that would be sent to LLM:",
                aggregated.len(),
                used_lines
            );
            eprintln!("{}", aggregated.trim());
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
        run::run_no_redact(&log_content, &config, client.as_ref())
    } else {
        run::run(&log_content, &config, client.as_ref())
    };

    match result {
        Ok(output) => {
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&output).unwrap());
            } else {
                println!("{}", output.to_plain());
                if lx_core::output::show_narration(cli.quiet, cli.verbose) {
                    if output.capped {
                        eprintln!(
                            "warning: log too large — only {} lines sent to the LLM; results may be incomplete",
                            output.used_lines
                        );
                    }
                    if output.anomalies.is_empty() {
                        eprintln!("# no anomalies detected");
                    } else {
                        eprintln!("# {} anomaly/anomalies detected", output.anomalies.len());
                    }
                    if !output.used_lines.is_empty() {
                        eprintln!("# lines analysed: {}", output.used_lines);
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
