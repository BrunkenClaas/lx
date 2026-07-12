#![forbid(unsafe_code)]

use clap::Parser;
use lx_config::Config;
use lx_core::{error::print_error, exit};
use std::path::PathBuf;
use std::process;

mod run;

fn version_string() -> String {
    lx_core::version::build_version_string("lxfixscript", env!("CARGO_PKG_VERSION"))
}

#[derive(Parser)]
#[command(
    name = "lxfixscript",
    about = "Fix a broken shell script given an optional error message",
    disable_version_flag = true
)]
struct Cli {
    /// Optional error message from running the broken script
    error_msg: Option<String>,

    /// Output as JSON (includes dangerous:bool field)
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

    /// Show verbose diagnostics on stderr
    #[arg(long)]
    verbose: bool,

    /// Maximum bytes to read from stdin
    #[arg(long)]
    max_input_bytes: Option<usize>,

    /// Read script from file instead of stdin
    #[arg(long, value_name = "PATH")]
    file: Option<PathBuf>,

    /// Accept dangerous output and exit 0 instead of 3 (warning still printed to stderr)
    #[arg(long, short = 'D')]
    allow_dangerous: bool,

    /// Target OS for script conventions (linux, windows, macos). Defaults to host OS.
    #[arg(long, value_name = "OS", default_value = "auto")]
    target: String,

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
            config.output.lang,
        );
    }

    let target_os = if cli.target == "auto" {
        lx_core::platform::os().to_string()
    } else {
        cli.target.to_lowercase()
    };

    // Script is required — read from --file or piped stdin.
    let max = cli.max_input_bytes.unwrap_or(config.limits.max_input_bytes);
    let script = lx_core::io::resolve_input(cli.file.as_deref(), max).unwrap_or_else(|e| {
        print_error(&e, cli.json);
        process::exit(exit::BAD_USAGE);
    });

    let error_msg = cli.error_msg.as_deref().unwrap_or("");

    // --dry-run: show what would be sent to the LLM, then exit.
    if cli.dry_run {
        if !cli.quiet {
            eprintln!("[dry-run] input ({} bytes):", script.len());
            eprintln!("{}", script.trim());
            if !error_msg.is_empty() {
                eprintln!("[dry-run] error message: {}", error_msg.trim());
            }
            eprintln!("[dry-run] target os: {target_os}");
            eprintln!("[dry-run] system prompt:");
            eprintln!(
                "{}",
                lx_llm::inject_os(
                    &lx_llm::inject_lang(run::SYSTEM_TEMPLATE, &config.output.lang),
                    &target_os
                )
            );
        }
        process::exit(exit::SUCCESS);
    }

    let client = lx_llm::client_from_config(&config, cli.verbose).unwrap_or_else(|e| {
        print_error(&e, cli.json);
        process::exit(e.exit_code());
    });

    match run::run(&script, error_msg, &target_os, &config, client.as_ref()) {
        Ok(output) => {
            // DANGER warning always goes to stderr — never suppressed by --quiet.
            // (The warning is already printed inside run() when dangerous == true.)

            let code = if output.dangerous && !cli.allow_dangerous {
                exit::DANGEROUS
            } else {
                exit::SUCCESS
            };

            if cli.json {
                println!("{}", serde_json::to_string_pretty(&output).unwrap());
            } else {
                // Plain mode: corrected script only on stdout; changes on stderr.
                println!("{}", output.to_plain());
                if lx_core::output::show_narration(cli.quiet, cli.verbose) {
                    for change in &output.changes {
                        eprintln!("# fix: {}", change);
                    }
                }
            }

            process::exit(code);
        }
        Err(e) => {
            print_error(&e, cli.json);
            process::exit(e.exit_code());
        }
    }
}
