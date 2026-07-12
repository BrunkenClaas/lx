#![forbid(unsafe_code)]

use clap::Parser;
use lx_config::Config;
use lx_core::{error::print_error, exit};
use std::path::PathBuf;
use std::process;

mod run;

fn version_string() -> String {
    lx_core::version::build_version_string("lxkill", env!("CARGO_PKG_VERSION"))
}

#[derive(Parser)]
#[command(
    name    = "lxkill",
    about   = "Generate the command to find and kill a described process",
    // Disable clap's built-in --version; we emit the canonical suite format manually.
    disable_version_flag = true
)]
struct Cli {
    /// Description of the process to kill (e.g. "process listening on port 3000")
    description: Option<String>,

    /// Output as JSON (includes dangerous:bool field)
    #[arg(long)]
    json: bool,

    /// Disable ANSI colours and formatting
    #[arg(long)]
    plain: bool,

    /// Show the input and system prompt that would be sent, then exit without sending
    #[arg(long)]
    dry_run: bool,

    /// Suppress diagnostic messages on stderr (DANGER warnings are never suppressed)
    #[arg(short, long)]
    quiet: bool,

    /// Output language (BCP-47, e.g. 'en', 'de', 'fr')
    #[arg(long, default_value = "auto")]
    lang: String,

    /// Show verbose diagnostics on stderr
    #[arg(long)]
    verbose: bool,

    /// Maximum bytes to read from stdin (context only)
    #[arg(long)]
    max_input_bytes: Option<usize>,

    /// Read process list context from file instead of stdin
    #[arg(long, value_name = "PATH")]
    file: Option<PathBuf>,

    /// Accept dangerous output and exit 0 instead of 3 (DANGER warning still printed)
    #[arg(long, short = 'D')]
    allow_dangerous: bool,

    /// Target OS for generated commands (linux, windows, macos). Defaults to host OS.
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

    // description is the required positional argument
    let description = match cli.description {
        Some(d) => d,
        None => {
            let e = lx_core::error::LxError::BadUsage(
                "no description provided; pass the process description as an argument".to_string(),
            );
            print_error(&e, cli.json);
            process::exit(exit::BAD_USAGE);
        }
    };

    if description.trim().is_empty() {
        let e = lx_core::error::LxError::BadUsage("description must not be empty".to_string());
        print_error(&e, cli.json);
        process::exit(exit::BAD_USAGE);
    }

    // stdin is optional context (ps/ss output) — read only if not a TTY
    let max_bytes = cli.max_input_bytes.unwrap_or(config.limits.max_input_bytes);
    let context = if !lx_core::platform::is_tty(lx_core::platform::Fd::Stdin) {
        lx_core::io::resolve_input(cli.file.as_deref(), max_bytes).unwrap_or_default()
    } else if let Some(ref path) = cli.file {
        // --file explicitly provided even on TTY
        lx_core::io::resolve_input(Some(path.as_path()), max_bytes).unwrap_or_else(|e| {
            print_error(&e, cli.json);
            process::exit(e.exit_code());
        })
    } else {
        String::new()
    };

    // --dry-run: show what would be sent, then exit.
    if cli.dry_run {
        if !cli.quiet {
            eprintln!(
                "[dry-run] input ({} bytes):\n{}",
                description.trim().len(),
                description.trim()
            );
            if !context.trim().is_empty() {
                eprintln!(
                    "[dry-run] context ({} bytes):\n{}",
                    context.trim().len(),
                    context.trim()
                );
            }
            eprintln!("[dry-run] target os: {target_os}");
            eprintln!(
                "[dry-run] system prompt:\n{}",
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

    match run::run(&description, &context, &target_os, &config, client.as_ref()) {
        Ok((output, dangerous)) => {
            // Tier-3 danger warning → stderr, always shown (never suppressed by --quiet).
            run::warn_danger(dangerous);
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&output).unwrap());
            } else {
                println!("{}", output.to_plain());
                if lx_core::output::show_narration(cli.quiet, cli.verbose) {
                    eprintln!("# target: {} — {}", output.target, output.reason);
                }
            }
            if output.dangerous && !cli.allow_dangerous {
                process::exit(exit::DANGEROUS);
            }
            process::exit(exit::SUCCESS);
        }
        Err(e) => {
            print_error(&e, cli.json);
            process::exit(e.exit_code());
        }
    }
}
