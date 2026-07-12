#![forbid(unsafe_code)]

use clap::Parser;
use lx_config::Config;
use lx_core::{error::print_error, exit};
use std::path::PathBuf;
use std::process;

mod danger;
mod run;

fn version_string() -> String {
    lx_core::version::build_version_string("lxfixcmd", env!("CARGO_PKG_VERSION"))
}

#[derive(Parser)]
#[command(
    name = "lxfixcmd",
    about = "Fix the last failed shell command",
    disable_version_flag = true
)]
struct Cli {
    /// The failed command to fix
    failed_cmd: Option<String>,

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

    /// Read error context from file instead of stdin
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

    // Require the positional argument.
    let failed_cmd = match cli.failed_cmd {
        Some(s) => s,
        None => {
            let e = lx_core::error::LxError::BadUsage("no failed command provided".to_string());
            print_error(&e, cli.json);
            process::exit(exit::BAD_USAGE);
        }
    };

    // Optional error context from --file or piped stdin (not required).
    let error_context =
        if cli.file.is_some() || !lx_core::platform::is_tty(lx_core::platform::Fd::Stdin) {
            let max = cli.max_input_bytes.unwrap_or(config.limits.max_input_bytes);
            lx_core::io::resolve_input(cli.file.as_deref(), max).unwrap_or_default()
        } else {
            String::new()
        };

    // --dry-run: show what would be sent to the LLM, then exit.
    if cli.dry_run {
        if !cli.quiet {
            eprintln!("[dry-run] input ({} bytes):", failed_cmd.len());
            eprintln!("{}", failed_cmd.trim());
            if !error_context.is_empty() {
                eprintln!("[dry-run] error context ({} bytes):", error_context.len());
                eprintln!("{}", error_context.trim());
            }
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

    match run::run(&failed_cmd, &error_context, &config, client.as_ref()) {
        Ok(output) => {
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&output).unwrap());
            } else {
                // Plain mode: command only on stdout; reason and danger warnings on stderr.
                println!("{}", output.command);
                if lx_core::output::show_narration(cli.quiet, cli.verbose) {
                    eprintln!("# {}", output.reason);
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
