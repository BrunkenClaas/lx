#![forbid(unsafe_code)]

use clap::Parser;
use lx_config::Config;
use lx_core::{error::print_error, exit};
use std::path::PathBuf;
use std::process;

mod run;

// Build the canonical version string once at startup.
// Format: "lxman 1.0.0 (lx-coreutils 2026-07, <target>)"
fn version_string() -> String {
    lx_core::version::build_version_string("lxman", env!("CARGO_PKG_VERSION"))
}

#[derive(Parser)]
#[command(
    name    = "lxman",
    about   = "Show a plain-language man page for a command",
    // Disable clap's built-in --version; we handle it manually below so we can
    // produce the canonical "lxman X.Y.Z (lx-coreutils YYYY-MM, <target>)" format.
    disable_version_flag = true
)]
struct Cli {
    /// The CLI tool to explain (positional argument)
    tool: Option<String>,

    /// The CLI tool to explain (alternative flag form)
    #[arg(long = "for", value_name = "TOOL")]
    for_tool: Option<String>,

    /// Output as JSON
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

    /// Show token usage and estimated cost on stderr
    #[arg(long)]
    verbose: bool,

    /// Maximum bytes to read from stdin
    #[arg(long)]
    max_input_bytes: Option<usize>,

    /// Read input from file instead of stdin (not used for lxman; included for consistency)
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

    // CLI flags override config.
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

    // Resolve tool name: --for takes precedence, then positional, then error.
    let tool_name = match cli.for_tool.or(cli.tool) {
        Some(name) => name,
        None => {
            let e = lx_core::error::LxError::BadUsage(
                "no tool name provided; use `lxman <tool>` or `lxman --for <tool>`".to_string(),
            );
            print_error(&e, cli.json);
            process::exit(exit::BAD_USAGE);
        }
    };

    if tool_name.trim().is_empty() {
        let e = lx_core::error::LxError::BadUsage("tool name must not be empty".to_string());
        print_error(&e, cli.json);
        process::exit(exit::BAD_USAGE);
    }

    // --dry-run: show what would be sent, then exit.
    if cli.dry_run {
        if !cli.quiet {
            eprintln!("[dry-run] tool: {}", tool_name.trim());
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

    match run::run(&tool_name, &config, client.as_ref()) {
        Ok(output) => {
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&output).unwrap());
            } else {
                print!("{}", output.to_plain());
            }
            process::exit(exit::SUCCESS);
        }
        Err(e) => {
            print_error(&e, cli.json);
            process::exit(e.exit_code());
        }
    }
}
