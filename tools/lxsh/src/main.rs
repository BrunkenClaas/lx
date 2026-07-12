#![forbid(unsafe_code)]

use clap::Parser;
use lx_config::Config;
use lx_core::{error::print_error, exit};
use std::path::PathBuf;
use std::process;

mod danger;
mod run;

fn version_string() -> String {
    lx_core::version::build_version_string("lxsh", env!("CARGO_PKG_VERSION"))
}

#[derive(Parser)]
#[command(
    name    = "lxsh",
    about   = "Generate a shell command from a plain-English description",
    // Disable clap's built-in --version; we emit the canonical suite format manually.
    disable_version_flag = true
)]
struct Cli {
    /// Description of the command to generate (reads from stdin if omitted)
    description: Option<String>,

    /// Output as JSON (includes dangerous:bool field)
    #[arg(long)]
    json: bool,

    /// Disable ANSI colours and formatting
    #[arg(long)]
    plain: bool,

    /// Show the description that would be sent to the LLM, then exit without sending
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

    /// Read input from file instead of stdin
    #[arg(long, value_name = "PATH")]
    file: Option<PathBuf>,

    /// Target shell (auto-detected if omitted: bash|zsh|sh|powershell|cmd)
    #[arg(long, value_name = "SHELL")]
    shell: Option<String>,

    /// Accept dangerous output and exit 0 instead of 3 (warning still printed to stderr)
    #[arg(long, short = 'D')]
    allow_dangerous: bool,

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
    if let Some(s) = cli.shell {
        config.output.shell = s;
    }

    if cli.verbose {
        eprintln!(
            "[verbose] config: model={} provider={} lang={} shell={}",
            config.effective_model(),
            config.llm.provider,
            config.output.lang,
            config.output.shell,
        );
    }

    let description = if let Some(s) = cli.description {
        s
    } else {
        let max = cli.max_input_bytes.unwrap_or(config.limits.max_input_bytes);
        lx_core::io::resolve_input(cli.file.as_deref(), max).unwrap_or_else(|e| {
            print_error(&e, cli.json);
            process::exit(e.exit_code());
        })
    };

    if description.trim().is_empty() {
        let e = lx_core::error::LxError::BadUsage("no description provided".to_string());
        print_error(&e, cli.json);
        process::exit(exit::BAD_USAGE);
    }

    // --dry-run: show the description that would be sent to the LLM, then exit.
    // Also run danger check on the description itself — catches cases where the user
    // pastes a command directly as input rather than a plain-English description.
    if cli.dry_run {
        danger::warn_findings(&danger::check(description.trim()));
        if !cli.quiet {
            eprintln!(
                "[dry-run] description that would be sent to LLM ({} bytes):",
                description.trim().len()
            );
            eprintln!("{}", description.trim());
        }
        if !cli.quiet {
            eprintln!("[dry-run] system prompt:");
            eprintln!(
                "{}",
                lx_llm::inject_lang(run::SYSTEM_TEMPLATE, &config.output.lang)
                    .replace("{shell}", &config.output.shell)
                    .replace("{examples}", run::examples_for(&config.output.shell))
            );
        }
        process::exit(exit::SUCCESS);
    }

    let client = lx_llm::client_from_config(&config, cli.verbose).unwrap_or_else(|e| {
        print_error(&e, cli.json);
        process::exit(e.exit_code());
    });

    match run::run(&description, &config, client.as_ref()) {
        Ok((output, findings)) => {
            // Tier-3 danger warnings go to stderr and are always shown (never
            // suppressed by --quiet). Emit before the result so a reader sees the
            // caveat first. The command itself always goes to stdout — never executed.
            danger::warn_findings(&findings);
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&output).unwrap());
            } else {
                // Plain mode: only the command on stdout; danger warnings already on stderr.
                println!("{}", output.to_plain());
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
