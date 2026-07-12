#![forbid(unsafe_code)]

use clap::Parser;
use lx_config::Config;
use lx_core::{error::print_error, exit};
use std::path::PathBuf;
use std::process;

mod run;

fn version_string() -> String {
    lx_core::version::build_version_string("lxfirewall", env!("CARGO_PKG_VERSION"))
}

#[derive(Parser)]
#[command(
    name = "lxfirewall",
    about = "Generate or explain firewall rules (iptables/nftables/ufw)",
    disable_version_flag = true
)]
struct Cli {
    /// Firewall intent to implement, e.g. "allow SSH only from 10.0.0.0/8" (generate mode).
    /// Omit to explain rules piped on stdin (explain mode).
    intent: Option<String>,

    /// Output as JSON (full object to stdout)
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

    /// Maximum bytes to read from stdin
    #[arg(long)]
    max_input_bytes: Option<usize>,

    /// Read firewall ruleset from file instead of stdin
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

    // Resolve target OS: --target overrides, else host OS.
    let target_os = if cli.target == "auto" {
        lx_core::platform::os().to_string()
    } else {
        cli.target.to_lowercase()
    };

    let intent = cli.intent.clone().unwrap_or_default();

    // Read stdin (optional — ruleset context for generate mode, or the ruleset for explain mode).
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

    // Validate: need at least one of intent or state.
    if intent.trim().is_empty() && state.trim().is_empty() {
        let e = lx_core::error::LxError::BadUsage(
            "provide an intent as argument (generate mode) or pipe existing rules (explain mode)"
                .to_string(),
        );
        print_error(&e, cli.json);
        process::exit(exit::BAD_USAGE);
    }

    // OS mismatch warning — warn if piped state looks like a different OS.
    if !state.trim().is_empty() {
        if let Some(warn) = run::detect_os_mismatch(&state, &target_os) {
            eprintln!("# WARNING: {warn}");
        }
    }

    // --dry-run: show what would be sent, then exit.
    if cli.dry_run {
        if !cli.quiet {
            if !intent.trim().is_empty() {
                eprintln!(
                    "[dry-run] intent ({} bytes):\n{}",
                    intent.trim().len(),
                    intent.trim()
                );
            }
            if !state.trim().is_empty() {
                eprintln!(
                    "[dry-run] ruleset ({} bytes):\n{}",
                    state.trim().len(),
                    state.trim()
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

    match run::run(&intent, &state, &target_os, &config, client.as_ref()) {
        Ok((output, em)) => {
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&output).unwrap());
            } else {
                println!("{}", output.to_plain(em));
                // Warnings → stderr, suppressed by --quiet EXCEPT danger warnings.
                for w in &output.warnings {
                    if lx_core::output::show_narration(cli.quiet, cli.verbose) {
                        eprintln!("# WARNING: {}", w);
                    }
                }
            }

            // DANGER warning is always printed — never suppressed by --quiet.
            if output.dangerous {
                eprintln!(
                    "DANGER: This firewall command may lock out SSH access or flush all rules."
                );
                eprintln!("   Review carefully before running. This command was NOT executed.");
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
