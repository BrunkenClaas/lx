#![forbid(unsafe_code)]

use clap::Parser;
use lx_config::Config;
use lx_core::{error::print_error, exit};
use std::process;

mod run;

fn version_string() -> String {
    lx_core::version::build_version_string("lxtl", env!("CARGO_PKG_VERSION"))
}

#[derive(Parser)]
#[command(
    name = "lxtl",
    about = "Translate text to a target language (e.g. lxtl French < text.txt)",
    disable_version_flag = true
)]
struct Cli {
    /// Target language (BCP-47 or full name, e.g. 'fr', 'French', 'de')
    to: Option<String>,

    /// Read input from file instead of stdin
    #[arg(long, value_name = "PATH")]
    file: Option<std::path::PathBuf>,

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

    /// Output language for metadata (BCP-47, e.g. 'en', 'de')
    #[arg(long, default_value = "auto")]
    lang: String,

    /// Show token usage and estimated cost on stderr
    #[arg(long)]
    verbose: bool,

    /// Maximum bytes to read from stdin
    #[arg(long)]
    max_input_bytes: Option<usize>,

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

    let target_lang = match cli.to {
        Some(l) => l,
        None => {
            let e = lx_core::error::LxError::BadUsage(
                "missing required argument: <TO> (e.g. lxtl French)".to_string(),
            );
            print_error(&e, cli.json);
            process::exit(exit::BAD_USAGE);
        }
    };

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
        eprintln!("[verbose] target_lang: {}", target_lang);
    }

    // Collect input: --file > stdin.
    let max = cli.max_input_bytes.unwrap_or(config.limits.max_input_bytes);
    let input = lx_core::io::resolve_input(cli.file.as_deref(), max).unwrap_or_else(|e| {
        print_error(&e, cli.json);
        process::exit(e.exit_code());
    });

    if input.trim().is_empty() {
        let e = lx_core::error::LxError::BadUsage("no input provided".to_string());
        print_error(&e, cli.json);
        process::exit(exit::BAD_USAGE);
    }

    // --dry-run: show what would be sent, then exit.
    if cli.dry_run {
        if !cli.quiet {
            eprintln!("[dry-run] target language: {}", target_lang);
            eprintln!("[dry-run] input ({} bytes):", input.len());
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

    match run::run(&input, &target_lang, &config, client.as_ref()) {
        Ok(output) => {
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&output).unwrap());
            } else {
                println!("{}", output.text);
            }
            process::exit(exit::SUCCESS);
        }
        Err(e) => {
            print_error(&e, cli.json);
            process::exit(e.exit_code());
        }
    }
}
