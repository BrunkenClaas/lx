#![forbid(unsafe_code)]

use clap::Parser;
use lx_config::Config;
use lx_core::{error::print_error, exit};
use std::path::PathBuf;
use std::process;

mod run;
use run::ConfigMode;

fn version_string() -> String {
    lx_core::version::build_version_string("lxconf", env!("CARGO_PKG_VERSION"))
}

#[derive(Parser)]
#[command(
    name = "lxconf",
    about = "Check a config file for typical errors and misconfigurations",
    disable_version_flag = true
)]
struct Cli {
    /// Description for create mode (e.g. "nginx reverse proxy config") or change for edit mode
    description: Option<String>,

    /// Output as JSON
    #[arg(long)]
    json: bool,

    /// Disable ANSI colours and formatting
    #[arg(long)]
    plain: bool,

    /// Show the (redacted) input that would be sent to the LLM, then exit
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

    /// Maximum bytes to read from stdin or file
    #[arg(long)]
    max_input_bytes: Option<usize>,

    /// Read config from file instead of stdin
    #[arg(long, value_name = "PATH")]
    file: Option<PathBuf>,

    /// Restrict file access to this root directory (fsbound)
    #[arg(long, value_name = "DIR")]
    root: Option<PathBuf>,

    /// Disable secret redaction (NOT recommended — secrets will reach the LLM provider)
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

    // --version: canonical suite-aware format, then exit 0.
    if cli.version {
        println!("{}", version_string());
        process::exit(exit::SUCCESS);
    }

    // --no-redact: prominent warning.
    if cli.no_redact && !cli.quiet {
        eprintln!(
            "warning: --no-redact is set. Secrets in your config will be sent to \
             the LLM provider unmasked. Proceed only if you have audited the file."
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

    let max_bytes = cli.max_input_bytes.unwrap_or(config.limits.max_input_bytes);

    // Helper: read existing config from --file with fsbound, or from stdin.
    let read_existing_config = |json: bool| -> Option<String> {
        if let Some(ref file_path) = cli.file {
            let root = cli
                .root
                .clone()
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
            let canonical_file = match std::fs::canonicalize(file_path) {
                Ok(p) => p,
                Err(e) => {
                    let err = lx_core::error::LxError::BadUsage(format!(
                        "cannot resolve {}: {e}",
                        file_path.display()
                    ));
                    print_error(&err, json);
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
                    print_error(&err, json);
                    process::exit(exit::BAD_USAGE);
                }
            };
            if !canonical_file.starts_with(&canonical_root) {
                let err = lx_core::error::LxError::SecurityAbort(format!(
                    "path {} escapes allowed root {}",
                    canonical_file.display(),
                    canonical_root.display()
                ));
                print_error(&err, json);
                process::exit(exit::SECURITY_ABORT);
            }
            let content =
                lx_core::io::read_file_limited(file_path, max_bytes).unwrap_or_else(|e| {
                    print_error(&e, json);
                    process::exit(e.exit_code());
                });
            if content.trim().is_empty() {
                None
            } else {
                Some(content)
            }
        } else if !lx_core::platform::is_tty(lx_core::platform::Fd::Stdin) {
            let s = lx_core::io::read_stdin(max_bytes).unwrap_or_default();
            if s.trim().is_empty() {
                None
            } else {
                Some(s)
            }
        } else {
            None
        }
    };

    // Determine mode and inputs.
    let (input, existing, mode) = if let Some(ref desc) = cli.description {
        // Positional description given.
        let existing = read_existing_config(cli.json);
        if existing.is_some() {
            (desc.clone(), existing, ConfigMode::Edit)
        } else {
            (desc.clone(), None, ConfigMode::Create)
        }
    } else {
        // No description: audit existing config (stdin/--file required).
        let content = if let Some(file_path) = cli.file.as_ref() {
            let root = cli
                .root
                .clone()
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
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
        if content.trim().is_empty() {
            let e = lx_core::error::LxError::BadUsage(
                "no config content provided; use --file <PATH> or pipe config to stdin".to_string(),
            );
            print_error(&e, cli.json);
            process::exit(exit::BAD_USAGE);
        }
        (content, None, ConfigMode::Audit)
    };

    // --dry-run: show what would be sent to the LLM, then exit.
    if cli.dry_run {
        if !cli.quiet {
            let preview_input = match mode {
                ConfigMode::Audit => {
                    let level = lx_redact::RedactLevel::parse(&config.redact.level);
                    if cli.no_redact {
                        input.clone()
                    } else {
                        match lx_redact::redact(&input, level) {
                            Ok(r) => r,
                            Err(e) => {
                                let lx_err = lx_core::error::LxError::SecurityAbort(format!(
                                    "redaction failed: {e}"
                                ));
                                print_error(&lx_err, cli.json);
                                process::exit(exit::SECURITY_ABORT);
                            }
                        }
                    }
                }
                _ => input.clone(),
            };
            eprintln!("[dry-run] input ({} bytes):", preview_input.len());
            eprintln!("{}", preview_input.trim());
            if let Some(ref ex) = existing {
                eprintln!("[dry-run] existing config ({} bytes):", ex.len());
                eprintln!("{}", ex.trim());
            }
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
        run::run_no_redact(&input, existing.as_deref(), mode, &config, client.as_ref())
    } else {
        run::run(&input, existing.as_deref(), mode, &config, client.as_ref())
    };

    match result {
        Ok((output, warnings)) => {
            // Tier-2 warnings (e.g. input truncation): shown unless --quiet.
            for w in &warnings {
                lx_core::output::warn(w);
            }
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&output).unwrap());
            } else {
                println!("{}", output.to_plain(mode));
            }
            process::exit(exit::SUCCESS);
        }
        Err(e) => {
            print_error(&e, cli.json);
            process::exit(e.exit_code());
        }
    }
}
