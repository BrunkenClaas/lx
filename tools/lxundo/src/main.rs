#![forbid(unsafe_code)]

use clap::Parser;
use lx_config::Config;
use lx_core::{error::print_error, exit};
use std::path::PathBuf;
use std::process;

mod run;

fn version_string() -> String {
    lx_core::version::build_version_string("lxundo", env!("CARGO_PKG_VERSION"))
}

#[derive(Parser)]
#[command(
    name    = "lxundo",
    about   = "Suggest how to undo a command",
    // Disable clap's built-in --version; we emit the canonical suite format manually.
    disable_version_flag = true
)]
struct Cli {
    /// The command to undo (reads from stdin if omitted)
    input: Option<String>,

    /// Output as JSON (includes undo_command and caution fields)
    #[arg(long)]
    json: bool,

    /// Disable ANSI colours and formatting
    #[arg(long)]
    plain: bool,

    /// Show the input that would be sent to the LLM, then exit without sending
    #[arg(long)]
    dry_run: bool,

    /// Suppress diagnostic messages on stderr (danger warnings are never suppressed)
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

    let input = if let Some(s) = cli.input {
        s
    } else {
        let max = cli.max_input_bytes.unwrap_or(config.limits.max_input_bytes);
        lx_core::io::resolve_input(cli.file.as_deref(), max).unwrap_or_else(|e| {
            print_error(&e, cli.json);
            process::exit(e.exit_code());
        })
    };

    if input.trim().is_empty() {
        let e = lx_core::error::LxError::BadUsage("no command provided".to_string());
        print_error(&e, cli.json);
        process::exit(exit::BAD_USAGE);
    }

    // --dry-run: show the input that would be sent to the LLM, then exit.
    if cli.dry_run {
        if !cli.quiet {
            eprintln!(
                "[dry-run] input that would be sent to LLM ({} bytes):",
                input.trim().len()
            );
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
            // Run local danger detection on the undo_command (nocmd rule).
            // Danger warnings are printed to stderr and NEVER suppressed by --quiet.
            let findings = run::check(&output.undo_command);
            run::warn_findings(&findings);
            let is_dangerous = !findings.is_empty();

            if cli.json {
                println!("{}", serde_json::to_string_pretty(&output).unwrap());
            } else {
                // Plain mode: only the undo_command on stdout.
                println!("{}", output.undo_command);
                // Caution goes to stderr — suppressed by --quiet unless it's a danger warning.
                if !output.caution.is_empty() && !is_dangerous {
                    if lx_core::output::show_narration(cli.quiet, cli.verbose) {
                        eprintln!("# {}", output.caution);
                    }
                } else if !output.caution.is_empty() && is_dangerous {
                    // Danger was already printed via check_and_warn; don't double-print caution.
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
