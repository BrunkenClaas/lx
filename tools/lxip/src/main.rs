#![forbid(unsafe_code)]

use clap::Parser;
use lx_config::Config;
use lx_core::{error::print_error, exit};
use std::path::PathBuf;
use std::process;

mod run;

fn version_string() -> String {
    lx_core::version::build_version_string("lxip", env!("CARGO_PKG_VERSION"))
}

#[derive(Parser)]
#[command(
    name = "lxip",
    about = "Generate or explain ip commands — state-aware (pipe ip route/addr for context)",
    disable_version_flag = true
)]
struct Cli {
    /// Intent (e.g. "add a static route to 10.0.0.0/24 via 192.168.1.254")
    intent: Option<String>,

    #[arg(long)]
    json: bool,
    #[arg(long)]
    plain: bool,
    #[arg(long)]
    dry_run: bool,
    #[arg(short, long)]
    quiet: bool,
    #[arg(long, default_value = "auto")]
    lang: String,
    #[arg(long)]
    verbose: bool,
    #[arg(long)]
    max_input_bytes: Option<usize>,
    #[arg(long, value_name = "PATH")]
    file: Option<PathBuf>,
    #[arg(long, short = 'D')]
    allow_dangerous: bool,
    /// Target OS for generated commands (linux, windows, macos). Defaults to host OS.
    #[arg(long, value_name = "OS", default_value = "auto")]
    target: String,
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

    let target_os = if cli.target == "auto" {
        lx_core::platform::os().to_string()
    } else {
        cli.target.to_lowercase()
    };

    let intent = cli.intent.clone().unwrap_or_default();

    // Read stdin if available (ip state context)
    let max_bytes = cli.max_input_bytes.unwrap_or(config.limits.max_input_bytes);
    let state = if !lx_core::platform::is_tty(lx_core::platform::Fd::Stdin) {
        lx_core::io::resolve_input(cli.file.as_deref(), max_bytes).unwrap_or_default()
    } else if let Some(ref path) = cli.file {
        lx_core::io::resolve_input(Some(path.as_path()), max_bytes).unwrap_or_else(|e| {
            print_error(&e, cli.json);
            process::exit(e.exit_code());
        })
    } else {
        String::new()
    };

    // Check we have at least one input
    if intent.trim().is_empty() && state.trim().is_empty() {
        let e = lx_core::error::LxError::BadUsage(
            "provide an intent as argument or pipe ip state via stdin".to_string(),
        );
        print_error(&e, cli.json);
        process::exit(exit::BAD_USAGE);
    }

    // OS mismatch warning
    if !state.trim().is_empty() {
        if let Some(warn) = run::detect_os_mismatch(&state, &target_os) {
            eprintln!("# WARNING: {warn}");
        }
    }

    if cli.dry_run {
        if !cli.quiet {
            if !intent.is_empty() {
                eprintln!("[dry-run] intent: {}", intent.trim());
            }
            if !state.is_empty() {
                eprintln!("[dry-run] state ({} bytes):\n{}", state.len(), state.trim());
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

    match run::run(&intent, &state, &target_os, &config, client.as_ref()) {
        Ok((output, explain_mode)) => {
            // DANGER warning is never suppressed
            if output.dangerous {
                eprintln!("DANGER: This command may disrupt network connectivity.");
                eprintln!("   Review carefully before running. This command was NOT executed.");
            }

            if cli.json {
                println!("{}", serde_json::to_string_pretty(&output).unwrap());
            } else {
                println!("{}", output.to_plain(explain_mode));
                if lx_core::output::show_narration(cli.quiet, cli.verbose) {
                    for w in &output.warnings {
                        eprintln!("# WARNING: {w}");
                    }
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
