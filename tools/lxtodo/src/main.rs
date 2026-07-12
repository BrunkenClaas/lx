#![forbid(unsafe_code)]

use clap::Parser;
use lx_config::Config;
use lx_core::{error::print_error, exit};
use std::path::PathBuf;
use std::process;

mod run;

/// Build the canonical version string.
/// Format: "lxtodo 1.0.0 (lx-coreutils 2026-07, <target>)"
fn version_string() -> String {
    lx_core::version::build_version_string("lxtodo", env!("CARGO_PKG_VERSION"))
}

#[derive(Parser)]
#[command(
    name    = "lxtodo",
    about   = "Extract TODO/FIXME/HACK comments and action items from code or text",
    // Disable clap's built-in --version; we handle it manually for canonical format.
    disable_version_flag = true
)]
struct Cli {
    /// Input text (reads from stdin if omitted)
    input: Option<String>,

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

    /// Read input from file instead of stdin
    #[arg(long, value_name = "PATH")]
    file: Option<PathBuf>,

    /// Disable network access (no LLM call; local scan only)
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

    let max_bytes = cli.max_input_bytes.unwrap_or(config.limits.max_input_bytes);

    // Collect input: positional arg > --file (with fsbound) > stdin.
    let (input, source_name) = if let Some(s) = cli.input {
        (s, "stdin".to_string())
    } else if let Some(ref path) = cli.file {
        // fsbound: resolve the path against itself as root (the file's parent
        // directory is the allowed root). Use lx_core::io::read_file with the
        // file's parent as the allowed_root to prevent symlink escapes.
        let parent = path.parent().unwrap_or(std::path::Path::new("."));
        let content = lx_core::io::read_file(path, max_bytes, Some(parent)).unwrap_or_else(|e| {
            print_error(&e, cli.json);
            process::exit(e.exit_code());
        });
        let name = path.display().to_string();
        (content, name)
    } else {
        let content = lx_core::io::read_stdin(max_bytes).unwrap_or_else(|e| {
            print_error(&e, cli.json);
            process::exit(e.exit_code());
        });
        (content, "stdin".to_string())
    };

    // --dry-run: show what would be sent, then exit.
    if cli.dry_run {
        if !cli.quiet {
            eprintln!("[dry-run] source: {source_name}");
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

    // --no-net: run local scan only, no LLM.
    if cli.no_net {
        let local_hits = run::local_scan(&input);
        let output = run::Output {
            todos: local_hits
                .into_iter()
                .map(|(ln, text)| run::TodoItem {
                    file: if source_name == "stdin" {
                        None
                    } else {
                        Some(source_name.clone())
                    },
                    line: Some(ln),
                    text,
                })
                .collect(),
        };
        if cli.json {
            println!("{}", serde_json::to_string_pretty(&output).unwrap());
        } else {
            print!("{}", output.to_plain());
            if lx_core::output::show_narration(cli.quiet, cli.verbose) && output.todos.is_empty() {
                eprintln!("# No TODO items found.");
            }
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
                print!("{}", output.to_plain());
                if lx_core::output::show_narration(cli.quiet, cli.verbose)
                    && output.todos.is_empty()
                {
                    eprintln!("# No TODO items found.");
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
