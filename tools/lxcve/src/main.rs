#![forbid(unsafe_code)]

use clap::Parser;
use lx_config::Config;
use lx_core::{error::print_error, exit};
use std::path::PathBuf;
use std::process;

mod run;

fn version_string() -> String {
    lx_core::version::build_version_string("lxcve", env!("CARGO_PKG_VERSION"))
}

#[derive(Parser)]
#[command(
    name = "lxcve",
    about = "Check a dependency lockfile for known CVE vulnerabilities",
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
            "[verbose] config: model={} provider={} lang={}",
            config.effective_model(),
            config.llm.provider,
            config.output.lang
        );
    }

    // SEC: fsbound — if --path is given, verify --file stays within that root.
    if let Some(ref root_arg) = cli.path {
        match std::fs::canonicalize(root_arg) {
            Ok(canonical_root) => {
                if let Some(ref file_arg) = cli.file {
                    match std::fs::canonicalize(file_arg) {
                        Ok(canonical_file) => {
                            if !canonical_file.starts_with(&canonical_root) {
                                eprintln!(
                                    "error[E5]: file '{}' is outside the allowed path '{}'",
                                    file_arg.display(),
                                    root_arg.display()
                                );
                                eprintln!(
                                    "  hint: use --path to set the allowed root that contains your --file"
                                );
                                process::exit(exit::SECURITY_ABORT);
                            }
                        }
                        Err(e) => {
                            eprintln!("error[E1]: cannot resolve --file path: {}", e);
                            process::exit(exit::LOGICAL_ERROR);
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("error[E1]: cannot resolve --path directory: {}", e);
                process::exit(exit::LOGICAL_ERROR);
            }
        }
    }

    let max = cli.max_input_bytes.unwrap_or(config.limits.max_input_bytes);
    let input = lx_core::io::resolve_input(cli.file.as_deref(), max).unwrap_or_else(|e| {
        print_error(&e, cli.json);
        process::exit(e.exit_code());
    });

    if input.trim().is_empty() {
        let e = lx_core::error::LxError::BadUsage(
            "no lockfile content provided; pipe a lockfile or use --file <path>".to_string(),
        );
        print_error(&e, cli.json);
        process::exit(exit::BAD_USAGE);
    }

    // --dry-run: show what would be sent to the LLM, then exit.
    if cli.dry_run {
        if !cli.quiet {
            eprintln!("[dry-run] lockfile input ({} bytes):", input.len());
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
                println!("{}", serde_json::to_string_pretty(&output).unwrap());
            } else {
                let plain = output.to_plain();
                // Plain mode: result only on stdout.
                println!("{}", plain);
                if lx_core::output::show_narration(cli.quiet, cli.verbose)
                    && !output.vulns.is_empty()
                {
                    eprintln!(
                        "# {} vulnerability/vulnerabilities found",
                        output.vulns.len()
                    );
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
