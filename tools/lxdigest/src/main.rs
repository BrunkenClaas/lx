#![forbid(unsafe_code)]

use clap::Parser;
use lx_config::Config;
use lx_core::{error::print_error, exit};
use std::path::PathBuf;
use std::process;

mod run;

fn version_string() -> String {
    lx_core::version::build_version_string("lxdigest", env!("CARGO_PKG_VERSION"))
}

#[derive(Parser)]
#[command(
    name = "lxdigest",
    about = "Summarise a whole directory with LLM assistance",
    disable_version_flag = true
)]
struct Cli {
    /// Directory to summarise (defaults to current directory)
    path: Option<PathBuf>,

    /// Output as JSON (full object including files list)
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

    /// Print config summary, token counts, and retry diagnostics to stderr
    #[arg(long)]
    verbose: bool,

    /// Maximum bytes to read from stdin (unused for directory input, kept for flag consistency)
    #[arg(long)]
    max_input_bytes: Option<usize>,

    /// Read path from file instead of --path argument (unused for directory input, kept for flag consistency)
    #[arg(long, value_name = "PATH")]
    file: Option<PathBuf>,

    /// Disable secret redaction (NOT recommended — file paths may contain sensitive data)
    #[arg(long)]
    no_redact: bool,

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

    // --no-redact: prominent warning.
    if cli.no_redact && !cli.quiet {
        eprintln!(
            "warning: --no-redact is set. File paths and names may contain sensitive \
             data that will be sent to the LLM provider unmasked."
        );
    }

    let root_path = cli.path.unwrap_or_else(|| PathBuf::from("."));

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
            "[verbose] config: model={} provider={} lang={} redact={}",
            config.effective_model(),
            config.llm.provider,
            config.output.lang,
            if cli.no_redact { "off" } else { "on" }
        );
        eprintln!("[verbose] path: {}", root_path.display());
    }

    // --dry-run: show the path and exit.
    if cli.dry_run {
        if !cli.quiet {
            eprintln!(
                "[dry-run] would summarise directory: {}",
                root_path.display()
            );
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

    match run::run(&root_path, cli.no_redact, &config, client.as_ref()) {
        Ok(output) => {
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&output).unwrap());
            } else {
                // Pipe-safe: summary only to stdout.
                println!("{}", output.to_plain());
                if lx_core::output::show_narration(cli.quiet, cli.verbose)
                    && !output.files.is_empty()
                {
                    eprintln!("# Notable files:");
                    for f in &output.files {
                        eprintln!("#   {f}");
                    }
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
