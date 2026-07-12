#![forbid(unsafe_code)]

use clap::Parser;
use lx_config::Config;
use lx_core::{error::print_error, exit};
use std::path::PathBuf;
use std::process;

mod danger;
mod run;

fn version_string() -> String {
    lx_core::version::build_version_string("lxcode", env!("CARGO_PKG_VERSION"))
}

#[derive(Parser)]
#[command(
    name  = "lxcode",
    about = "Generate code from a natural-language description",
    // Disable clap's built-in --version; we emit the canonical suite format manually.
    disable_version_flag = true
)]
struct Cli {
    /// Description of the code to generate (reads from stdin if omitted)
    description: Option<String>,

    /// Target programming language (rust|python|go|js|ts|bash|...; auto-detected if omitted)
    #[arg(long, default_value = "auto")]
    code_lang: String,

    /// Output as JSON {"code": "...", "language": "..."}
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

    /// Output language for comments and explanations (BCP-47, e.g. 'en', 'de', 'fr')
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

    if cli.verbose {
        eprintln!(
            "[verbose] config: model={} provider={} lang={}",
            config.effective_model(),
            config.llm.provider,
            config.output.lang
        );
        if cli.code_lang != "auto" {
            eprintln!("[verbose] code-lang: {}", cli.code_lang);
        }
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

    // --dry-run: show what would be sent to the LLM, then exit.
    if cli.dry_run {
        if !cli.quiet {
            eprintln!(
                "[dry-run] description that would be sent to LLM ({} bytes):",
                description.trim().len()
            );
            eprintln!("{}", description.trim());
            if cli.code_lang != "auto" {
                eprintln!("[dry-run] target language: {}", cli.code_lang);
            }
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

    let lang_hint = if cli.code_lang == "auto" {
        None
    } else {
        Some(cli.code_lang.as_str())
    };

    match run::run(&description, lang_hint, &config, client.as_ref()) {
        Ok(output) => {
            // §8.3 nocmd: danger warnings already printed to stderr inside run().
            // The code itself always goes to stdout — never executed.
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&output).unwrap());
            } else {
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
