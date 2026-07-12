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
    lx_core::version::build_version_string("lxmakefile", env!("CARGO_PKG_VERSION"))
}

#[derive(Parser)]
#[command(
    name = "lxmakefile",
    about = "Generate a Makefile or justfile from a task description",
    disable_version_flag = true
)]
struct Cli {
    /// Description of the tasks to include (reads from stdin if omitted)
    description: Option<String>,

    /// Hint for output format: make (default) or just
    #[arg(long, value_name = "FORMAT", default_value = "make")]
    format: String,

    /// Output as JSON
    #[arg(long)]
    json: bool,

    /// Disable ANSI colours and formatting
    #[arg(long)]
    plain: bool,

    /// Show the description that would be sent to the LLM, then exit without sending
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

    /// Read input from file instead of stdin
    #[arg(long, value_name = "PATH")]
    file: Option<PathBuf>,

    /// Print version information
    #[arg(short = 'V', long = "version")]
    version: bool,
}

fn main() {
    let cli = Cli::parse();

    platform::enable_ansi();
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
        eprintln!("[verbose] format hint: {}", cli.format);
    }

    let max = cli.max_input_bytes.unwrap_or(config.limits.max_input_bytes);

    // Determine intent and existing content (for edit mode).
    let (intent, existing) = if let Some(desc) = cli.description {
        let existing = if cli.file.is_some() {
            Some(
                lx_core::io::resolve_input(cli.file.as_deref(), max).unwrap_or_else(|e| {
                    print_error(&e, cli.json);
                    process::exit(e.exit_code());
                }),
            )
        } else if !platform::is_tty(Fd::Stdin) {
            let s = lx_core::io::read_stdin(max).unwrap_or_else(|e| {
                print_error(&e, cli.json);
                process::exit(e.exit_code());
            });
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
        let content = lx_core::io::resolve_input(cli.file.as_deref(), max).unwrap_or_else(|e| {
            print_error(&e, cli.json);
            process::exit(e.exit_code());
        });
        (content, None)
    };

    if intent.trim().is_empty() {
        let e = lx_core::error::LxError::BadUsage("no description provided".to_string());
        print_error(&e, cli.json);
        process::exit(exit::BAD_USAGE);
    }

    // Incorporate format hint into the intent sent to the LLM (create mode only).
    let effective_intent = if existing.is_none() && cli.format == "just" {
        format!("[Generate a justfile]\n{}", intent.trim())
    } else {
        intent.trim().to_string()
    };

    // --dry-run: show what would be sent to the LLM, then exit.
    if cli.dry_run {
        if !cli.quiet {
            let preview = match &existing {
                Some(c) if !c.trim().is_empty() => format!(
                    "Edit mode — change: {}\n\n---\n{}",
                    effective_intent,
                    c.trim()
                ),
                _ => effective_intent.clone(),
            };
            eprintln!("[dry-run] input ({} bytes):", preview.len());
            eprintln!("{}", preview);
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

    match run::run(
        &effective_intent,
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
                // Plain mode: content goes to stdout; danger warnings on stderr.
                println!("{}", output.content);
            }
            process::exit(exit::SUCCESS);
        }
        Err(e) => {
            print_error(&e, cli.json);
            process::exit(e.exit_code());
        }
    }
}
