#![forbid(unsafe_code)]

use clap::Parser;
use lx_config::Config;
use lx_core::{error::print_error, exit};
use lxmount::run;
use std::path::PathBuf;
use std::process;

fn version_string() -> String {
    lx_core::version::build_version_string("lxmount", env!("CARGO_PKG_VERSION"))
}

#[derive(Parser)]
#[command(
    name = "lxmount",
    about = "Generate a mount command and fstab entry from a plain-English description",
    disable_version_flag = true
)]
struct Cli {
    /// Description of the mount task (e.g. "mount my NTFS USB drive read-write at /media/usb")
    description: Option<String>,

    /// Output as JSON (includes fstab_line, notes, and dangerous fields)
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

    /// Read context (fstab/lsblk output) from file instead of stdin
    #[arg(long, value_name = "PATH")]
    file: Option<PathBuf>,

    /// Accept dangerous output and exit 0 instead of 3 (warning still printed to stderr)
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
            config.output.lang
        );
    }

    let target_os = if cli.target == "auto" {
        lx_core::platform::os().to_string()
    } else {
        cli.target.to_lowercase()
    };

    // Description is optional — if absent and context has content, we enter explain mode.
    let description = cli.description.unwrap_or_default();

    // Optional context from stdin (fstab/lsblk output). If stdin is a TTY, context is empty.
    let max = cli.max_input_bytes.unwrap_or(config.limits.max_input_bytes);
    let context = if cli.file.is_some() {
        lx_core::io::resolve_input(cli.file.as_deref(), max).unwrap_or_else(|e| {
            print_error(&e, cli.json);
            process::exit(e.exit_code());
        })
    } else if lx_core::platform::is_tty(lx_core::platform::Fd::Stdin) {
        String::new()
    } else {
        lx_core::io::read_stdin(max).unwrap_or_else(|e| {
            print_error(&e, cli.json);
            process::exit(e.exit_code());
        })
    };

    // OS mismatch warning
    if !context.trim().is_empty() {
        if let Some(warn) = run::detect_os_mismatch(&context, &target_os) {
            eprintln!("# WARNING: {warn}");
        }
    }

    // --dry-run: show what would be sent, then exit.
    if cli.dry_run {
        if !cli.quiet {
            let explain_mode = description.trim().is_empty() && !context.trim().is_empty();
            let user_msg = if explain_mode {
                format!("Explain this mount configuration:\n{}", context.trim())
            } else if context.trim().is_empty() {
                description.trim().to_string()
            } else {
                format!(
                    "Request: {}\n\nCurrent system state:\n{}",
                    description.trim(),
                    context.trim()
                )
            };
            eprintln!("[dry-run] input ({} bytes):", user_msg.len());
            eprintln!("{}", user_msg);
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

    match run::run(&description, &context, &target_os, &config, client.as_ref()) {
        Ok((output, explain_mode)) => {
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&output).unwrap());
            } else if explain_mode {
                // Explain mode: explanation → stdout
                println!("{}", output.explanation);
                if lx_core::output::show_narration(cli.quiet, cli.verbose)
                    && !output.notes.is_empty()
                {
                    eprintln!("# note: {}", output.notes);
                }
            } else {
                // Generate mode: command → stdout; fstab line and notes → stderr.
                println!("{}", output.to_plain(false));
                if lx_core::output::show_narration(cli.quiet, cli.verbose) {
                    if let Some(ref fstab) = output.fstab_line {
                        if !fstab.is_empty() {
                            eprintln!("# fstab: {fstab}");
                        }
                    }
                    if !output.notes.is_empty() {
                        eprintln!("# note: {}", output.notes);
                    }
                }
            }
            let code = if output.dangerous && !cli.allow_dangerous {
                exit::DANGEROUS
            } else {
                exit::SUCCESS
            };
            process::exit(code);
        }
        Err(e) => {
            print_error(&e, cli.json);
            process::exit(e.exit_code());
        }
    }
}
