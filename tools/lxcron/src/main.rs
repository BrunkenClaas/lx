#![forbid(unsafe_code)]

use clap::Parser;
use lx_config::Config;
use lx_core::{error::print_error, exit};
use lxcron::run::{self, Mode};
use std::path::PathBuf;
use std::process;

fn version_string() -> String {
    lx_core::version::build_version_string("lxcron", env!("CARGO_PKG_VERSION"))
}

#[derive(Parser)]
#[command(
    name = "lxcron",
    about = "Generate or explain a crontab line",
    disable_version_flag = true
)]
struct Cli {
    /// Description to generate a crontab line for (omit to read from stdin for explain mode)
    description: Option<String>,

    /// Output as JSON (includes dangerous:bool field)
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

    let max = cli.max_input_bytes.unwrap_or(config.limits.max_input_bytes);

    // Mode detection:
    // 1. Try to read stdin/--file (for explain mode). If stdin is a TTY and no --file: stdin_content = "".
    // 2. If stdin_content is non-empty and detect_mode says Explain: mode = Explain, input = stdin_content.
    // 3. Else if description.is_some(): mode = Generate, input = description.
    // 4. Else: BadUsage error.
    // Mode detection:
    // - description + stdin/--file with crontab → Edit mode (change existing crontab)
    // - description only (no stdin) → Generate mode
    // - stdin/--file with crontab, no description → Explain mode
    // - stdin/--file without crontab pattern, no description → Generate (treat as description)
    let (input, mode) = if let Some(desc) = cli.description {
        // Positional arg given. Check for existing crontab in stdin/--file → Edit mode.
        let existing = if cli.file.is_some() {
            Some(
                lx_core::io::resolve_input(cli.file.as_deref(), max).unwrap_or_else(|e| {
                    print_error(&e, cli.json);
                    process::exit(e.exit_code());
                }),
            )
        } else if !lx_core::platform::is_tty(lx_core::platform::Fd::Stdin) {
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
        match existing {
            Some(ref cron_line) if run::detect_mode(cron_line) == Mode::Explain => {
                // stdin looks like an existing crontab line → Edit mode.
                // Build a combined input: "change_desc\n\n<existing crontab line>"
                let combined = format!("{}\n\n{}", desc.trim(), cron_line.trim());
                (combined, Mode::Edit)
            }
            _ => (desc, Mode::Generate),
        }
    } else if cli.file.is_some() {
        let content = lx_core::io::resolve_input(cli.file.as_deref(), max).unwrap_or_else(|e| {
            print_error(&e, cli.json);
            process::exit(e.exit_code());
        });
        let detected = run::detect_mode(&content);
        (content, detected)
    } else if lx_core::platform::is_tty(lx_core::platform::Fd::Stdin) {
        let e = lx_core::error::LxError::BadUsage(
            "no input provided — pass a description as an argument, pipe a crontab line, or use --file".to_string(),
        );
        print_error(&e, cli.json);
        process::exit(exit::BAD_USAGE);
    } else {
        let content = lx_core::io::read_stdin(max).unwrap_or_else(|e| {
            print_error(&e, cli.json);
            process::exit(e.exit_code());
        });
        let detected = run::detect_mode(&content);
        (content, detected)
    };

    if input.trim().is_empty() {
        let e = lx_core::error::LxError::BadUsage("no input provided".to_string());
        print_error(&e, cli.json);
        process::exit(exit::BAD_USAGE);
    }

    // --dry-run: show what would be sent, then exit.
    if cli.dry_run {
        if !cli.quiet {
            eprintln!("[dry-run] input ({} bytes):", input.len());
            eprintln!("{}", input.trim());
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

    match run::run(&input, mode, &config, client.as_ref()) {
        Ok(output) => {
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&output).unwrap());
            } else {
                match mode {
                    Mode::Generate | Mode::Edit => {
                        // crontab line → stdout; explanation → stderr
                        println!("{}", output.crontab);
                        if lx_core::output::show_narration(cli.quiet, cli.verbose) {
                            eprintln!("# {}", output.explanation);
                        }
                    }
                    Mode::Explain => {
                        // explanation IS the result → stdout
                        println!("{}", output.explanation);
                    }
                }
            }
            if output.dangerous && !cli.allow_dangerous {
                process::exit(exit::DANGEROUS);
            }
            process::exit(exit::SUCCESS);
        }
        Err(e) => {
            print_error(&e, cli.json);
            process::exit(e.exit_code());
        }
    }
}
