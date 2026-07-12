#![forbid(unsafe_code)]

use clap::Parser;
use lx_config::Config;
use lx_core::{error::print_error, exit};
use std::path::PathBuf;
use std::process;

mod run;

fn version_string() -> String {
    lx_core::version::build_version_string("lxgrep", env!("CARGO_PKG_VERSION"))
}

#[derive(Parser)]
#[command(
    name = "lxgrep",
    about = "Semantic grep: find lines matching a natural-language query",
    disable_version_flag = true
)]
struct Cli {
    /// Natural-language search query
    query: Option<String>,

    /// Files or directories to search (reads from stdin if omitted)
    #[arg(value_name = "PATH")]
    paths: Vec<PathBuf>,

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

    /// Maximum bytes to read from stdin or any single file
    #[arg(long)]
    max_input_bytes: Option<usize>,

    /// Read stdin content from file instead of stdin (search this file's content)
    #[arg(long, value_name = "PATH")]
    file: Option<PathBuf>,

    /// Disable network access (no-op for lxgrep; LLM call is always local config)
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
    }

    // Require the query positional argument.
    let query = match cli.query {
        Some(q) => q,
        None => {
            let e =
                lx_core::error::LxError::BadUsage("missing required argument: <query>".to_string());
            print_error(&e, cli.json);
            process::exit(exit::BAD_USAGE);
        }
    };

    let max_bytes = cli.max_input_bytes.unwrap_or(config.limits.max_input_bytes);

    // Determine the root for fsbound checks.
    // If explicit paths were given, the root is the current directory.
    // If reading from stdin / --file, there is no fsbound restriction (we own the data).
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    // Collect content to search.
    let output = if !cli.paths.is_empty() {
        // File/directory mode — fsbound relative to cwd.
        if cli.dry_run {
            if !cli.quiet {
                eprintln!("[dry-run] query: {query}");
                eprintln!("[dry-run] paths: {:?}", cli.paths);
            }
            let files = match run::collect_file_contents(&cli.paths, &cwd, max_bytes) {
                Ok(f) => f,
                Err(e) => {
                    print_error(&e, cli.json);
                    process::exit(e.exit_code());
                }
            };
            if !cli.quiet {
                let pairs: Vec<(&str, &str)> = files
                    .iter()
                    .map(|(d, c)| (d.as_str(), c.as_str()))
                    .collect();
                eprintln!("[dry-run] user message:");
                eprintln!("{}", run::preview_user_message(&query, &pairs));
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

        match run::run_on_files(&query, &cli.paths, &cwd, &config, client.as_ref()) {
            Ok(o) => o,
            Err(e) => {
                print_error(&e, cli.json);
                process::exit(e.exit_code());
            }
        }
    } else {
        // Stdin / --file mode.
        let content = if let Some(ref path) = cli.file {
            // --file given — read that file (no fsbound, user explicitly named it).
            lx_core::io::read_file_limited(path, max_bytes).unwrap_or_else(|e| {
                print_error(&e, cli.json);
                process::exit(e.exit_code());
            })
        } else {
            lx_core::io::resolve_input(None, max_bytes).unwrap_or_else(|e| {
                print_error(&e, cli.json);
                process::exit(e.exit_code());
            })
        };

        if content.trim().is_empty() {
            let e = lx_core::error::LxError::BadUsage("no content to search".to_string());
            print_error(&e, cli.json);
            process::exit(exit::BAD_USAGE);
        }

        if cli.dry_run {
            if !cli.quiet {
                eprintln!("[dry-run] query: {query}");
                eprintln!("[dry-run] stdin ({} bytes)", content.len());
                eprintln!("[dry-run] user message:");
                eprintln!(
                    "{}",
                    run::preview_user_message(&query, &[("<stdin>", &content)])
                );
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

        // Stdin is treated as anonymous "<stdin>" file.
        match run::run(&query, &[("<stdin>", &content)], &config, client.as_ref()) {
            Ok(o) => o,
            Err(e) => {
                print_error(&e, cli.json);
                process::exit(e.exit_code());
            }
        }
    };

    if output.capped {
        lx_core::output::warn(
            "input exceeded the search budget; a sampled subset was analysed — some lines were not searched",
        );
    }

    if cli.json {
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
    } else {
        let plain = output.to_plain();
        if plain.is_empty() {
            if lx_core::output::show_narration(cli.quiet, cli.verbose) {
                eprintln!("no matches found");
            }
        } else {
            print!("{}", plain);
        }
    }
    process::exit(exit::SUCCESS);
}
