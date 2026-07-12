#![forbid(unsafe_code)]

use clap::Parser;
use lx_config::Config;
use lx_core::{error::print_error, exit};
use std::path::PathBuf;
use std::process;

mod run;

fn version_string() -> String {
    lx_core::version::build_version_string("lxgraph", env!("CARGO_PKG_VERSION"))
}

#[derive(Parser)]
#[command(
    name = "lxgraph",
    about = "Generate an ASCII chart from numbers",
    disable_version_flag = true
)]
struct Cli {
    /// Output as JSON
    #[arg(long)]
    json: bool,

    /// Disable ANSI colours and formatting
    #[arg(long)]
    plain: bool,

    /// Show what would be sent to the LLM, then exit without calling it
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

    let max = cli.max_input_bytes.unwrap_or(config.limits.max_input_bytes);
    let input = lx_core::io::resolve_input(cli.file.as_deref(), max).unwrap_or_else(|e| {
        print_error(&e, cli.json);
        process::exit(e.exit_code());
    });

    if input.trim().is_empty() {
        let e = lx_core::error::LxError::BadUsage(
            "no input provided; pipe numbers or label,value pairs into lxgraph".to_string(),
        );
        print_error(&e, cli.json);
        process::exit(exit::BAD_USAGE);
    }

    // --dry-run: show what would be sent to the LLM, exit without calling it.
    if cli.dry_run {
        if !cli.quiet {
            eprintln!(
                "[dry-run] {} bytes of input that would be sent to LLM:",
                input.len()
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
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&output).unwrap());
            } else {
                // Plain mode: only the chart to stdout (pipe-safe).
                println!("{}", output.to_plain());
                if lx_core::output::show_narration(cli.quiet, cli.verbose)
                    && !output.series.is_empty()
                {
                    eprintln!("# Series: {}", output.series.join(", "));
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
