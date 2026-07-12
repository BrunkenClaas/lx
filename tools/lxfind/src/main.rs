#![forbid(unsafe_code)]

use clap::Parser;
use lx_config::Config;
use lx_core::{error::print_error, exit};
use std::path::PathBuf;
use std::process;

mod run;

fn version_string() -> String {
    lx_core::version::build_version_string("lxfind", env!("CARGO_PKG_VERSION"))
}

#[derive(Parser)]
#[command(
    name = "lxfind",
    about = "Semantic file search: find files by description",
    disable_version_flag = true
)]
struct Cli {
    /// Description of the file to find (e.g. "the script that runs backups")
    description: Option<String>,

    /// Directory to search within (default: current directory)
    #[arg(default_value = ".")]
    path: PathBuf,

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

    /// Read description from file instead of positional argument
    #[arg(long, value_name = "PATH")]
    file: Option<PathBuf>,

    /// Disable network access (not applicable for lxfind, included for flag
    /// consistency)
    #[arg(long)]
    no_net: bool,

    /// Print version information
    #[arg(short = 'V', long = "version")]
    version: bool,
}

fn main() {
    let cli = Cli::parse();

    lx_core::platform::enable_ansi();
    lx_core::output::set_quiet(cli.quiet);

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
        eprintln!("[verbose] search root: {}", cli.path.display());
    }

    // Resolve the description: positional arg > --file > stdin.
    let description = if let Some(d) = cli.description {
        d
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

    if cli.dry_run {
        if !cli.quiet {
            eprintln!("[dry-run] description ({} bytes):", description.len());
            eprintln!("{}", description.trim());
            eprintln!("[dry-run] search root: {}", cli.path.display());
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

    match run::run(&description, &cli.path, &config, client.as_ref()) {
        Ok(output) => {
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&output).unwrap());
            } else {
                // stdout: one path per line — pipe safe.
                print!("{}", output.to_plain());
                if lx_core::output::show_narration(cli.quiet, cli.verbose)
                    && output.paths.is_empty()
                {
                    eprintln!("# no matching files found");
                }
                if lx_core::output::show_narration(cli.quiet, cli.verbose) && output.truncated {
                    eprintln!(
                        "# results capped at {} most relevant matches; narrow your description for more",
                        output.paths.len()
                    );
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
