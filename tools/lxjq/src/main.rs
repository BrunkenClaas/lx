#![forbid(unsafe_code)]

use clap::Parser;
use lx_config::Config;
use lx_core::{
    error::print_error,
    exit,
    platform::{self, Fd},
};
use std::path::PathBuf;
use std::process;

mod run;

fn version_string() -> String {
    lx_core::version::build_version_string("lxjq", env!("CARGO_PKG_VERSION"))
}

#[derive(Parser)]
#[command(
    name = "lxjq",
    about = "Generate a jq expression from a plain-English description",
    // Disable clap's built-in --version; we emit the canonical suite format manually.
    disable_version_flag = true
)]
struct Cli {
    /// Plain-English description of the desired jq transformation
    description: Option<String>,

    /// Optional JSON input to use as structural context for the expression
    #[arg(long, value_name = "JSON")]
    input: Option<String>,

    /// Output as JSON
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

    // Determine description and optional existing jq expression.
    let (description, existing) = if let Some(desc) = cli.description {
        let existing = if cli.file.is_some() {
            let s = lx_core::io::resolve_input(cli.file.as_deref(), max).unwrap_or_else(|e| {
                print_error(&e, cli.json);
                process::exit(e.exit_code());
            });
            if s.trim().is_empty() {
                None
            } else {
                Some(s)
            }
        } else if !platform::is_tty(Fd::Stdin) {
            let s = lx_core::io::read_stdin(max).unwrap_or_default();
            if s.trim().is_empty() {
                None
            } else {
                Some(s)
            }
        } else {
            None
        };
        (desc, existing)
    } else {
        let s = lx_core::io::resolve_input(cli.file.as_deref(), max).unwrap_or_else(|e| {
            print_error(&e, cli.json);
            process::exit(e.exit_code());
        });
        (s, None)
    };

    if description.trim().is_empty() {
        let e = lx_core::error::LxError::BadUsage("no description provided".to_string());
        print_error(&e, cli.json);
        process::exit(exit::BAD_USAGE);
    }

    // --dry-run: show what would be sent to the LLM, then exit.
    if cli.dry_run {
        if !cli.quiet {
            eprintln!("[dry-run] input ({} bytes):", description.trim().len());
            eprintln!("{}", description.trim());
            if let Some(ref ex) = existing {
                eprintln!("[dry-run] existing expression ({} bytes):", ex.trim().len());
                eprintln!("{}", ex.trim());
            }
            if let Some(ctx) = &cli.input {
                eprintln!("[dry-run] json context ({} bytes): {}", ctx.len(), ctx);
            }
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

    let json_context = cli.input.as_deref();

    match run::run(
        &description,
        json_context,
        existing.as_deref(),
        &config,
        client.as_ref(),
    ) {
        Ok((output, findings)) => {
            // Tier-3 danger warnings → stderr, always shown (never suppressed by --quiet).
            run::warn_findings(&findings);
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&output).unwrap());
            } else {
                println!("{}", output.to_plain());
                if lx_core::output::show_narration(cli.quiet, cli.verbose) {
                    eprintln!("# {}", output.explanation);
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
