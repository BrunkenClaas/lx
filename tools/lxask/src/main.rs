#![forbid(unsafe_code)]

use clap::Parser;
use lx_config::Config;
use lx_core::{error::print_error, exit};
use std::path::PathBuf;
use std::process;

mod run;

fn version_string() -> String {
    lx_core::version::build_version_string("lxask", env!("CARGO_PKG_VERSION"))
}

/// Check whether a resolved path is a forbidden system path.
fn is_system_path(path: &std::path::Path) -> bool {
    let s = path.to_string_lossy().to_lowercase();
    s.contains("/etc/")
        || s.contains("/.ssh")
        || s.contains("\\.ssh")
        || s.contains("/.aws")
        || s.contains("\\.aws")
        || s.contains("/proc/")
        || s.contains("\\windows\\system32")
}

#[derive(Parser)]
#[command(
    name = "lxask",
    about = "Answer a question from local context or general knowledge",
    disable_version_flag = true
)]
struct Cli {
    /// Question to answer (reads from --file or stdin if omitted)
    question: Option<String>,

    /// Context file to read and include with the question
    #[arg(long, value_name = "FILE")]
    context: Option<PathBuf>,

    /// Output as JSON
    #[arg(long)]
    json: bool,

    /// Disable ANSI colours and formatting
    #[arg(long)]
    plain: bool,

    /// Show the redacted input that would be sent to the LLM, then exit without sending
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

    /// Maximum bytes to read from stdin
    #[arg(long)]
    max_input_bytes: Option<usize>,

    /// Read question from file instead of stdin
    #[arg(long, value_name = "PATH")]
    file: Option<PathBuf>,

    /// Disable secret redaction (NOT recommended — sensitive data may reach the LLM provider)
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

    // --no-redact: prominent warning. Power-user escape hatch only.
    if cli.no_redact {
        eprintln!("warning: --no-redact is active; sensitive data may be sent to the LLM");
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
            "[verbose] config: model={} provider={} lang={} redact={}",
            config.effective_model(),
            config.llm.provider,
            config.output.lang,
            if cli.no_redact { "off" } else { "on" }
        );
    }

    let max = cli.max_input_bytes.unwrap_or(config.limits.max_input_bytes);

    // Resolve the question: positional arg > --file > stdin.
    // When the question comes from the positional arg and no --context was given,
    // treat piped stdin as the context document so that
    //   lxask "what is the main argument?" < article.md
    // works as expected.
    let positional_question = cli.question.is_some();
    let question_raw = if let Some(q) = cli.question {
        q
    } else {
        lx_core::io::resolve_input(cli.file.as_deref(), max).unwrap_or_else(|e| {
            print_error(&e, cli.json);
            process::exit(e.exit_code());
        })
    };

    if question_raw.trim().is_empty() {
        let e = lx_core::error::LxError::BadUsage(
            "no question provided; pass a question as a positional argument or via stdin"
                .to_string(),
        );
        print_error(&e, cli.json);
        process::exit(exit::BAD_USAGE);
    }

    // Resolve optional context file (fsbound check).
    // If the question came from a positional arg (not stdin) and no --context was
    // given, read stdin as the context document.
    let stdin_as_context: Option<String> = if positional_question && cli.context.is_none() {
        let raw = lx_core::io::read_stdin(max).unwrap_or_default();
        if raw.trim().is_empty() {
            None
        } else {
            Some(raw)
        }
    } else {
        None
    };

    let context_raw: Option<String> = if stdin_as_context.is_some() {
        stdin_as_context
    } else {
        match cli.context {
            None => None,
            Some(ref ctx_path) => {
                // Canonicalize to resolve symlinks and check boundaries.
                let canonical = match ctx_path.canonicalize() {
                    Ok(p) => p,
                    Err(e) => {
                        let lx_err = lx_core::error::LxError::BadUsage(format!(
                            "cannot resolve context file '{}': {e}",
                            ctx_path.display()
                        ));
                        print_error(&lx_err, cli.json);
                        process::exit(exit::BAD_USAGE);
                    }
                };

                if is_system_path(&canonical) {
                    let lx_err = lx_core::error::LxError::SecurityAbort(format!(
                        "context file '{}' is in a forbidden system path",
                        canonical.display()
                    ));
                    print_error(&lx_err, cli.json);
                    process::exit(exit::SECURITY_ABORT);
                }

                match std::fs::read_to_string(&canonical) {
                    Ok(contents) => {
                        let truncated = if contents.len() > max {
                            eprintln!("warning: context file truncated to {} bytes", max);
                            // Find a valid UTF-8 boundary at or before `max`.
                            let boundary = contents
                                .char_indices()
                                .map(|(i, _)| i)
                                .take_while(|&i| i < max)
                                .last()
                                .unwrap_or(0);
                            contents[..boundary].to_string()
                        } else {
                            contents
                        };
                        Some(truncated)
                    }
                    Err(e) => {
                        let lx_err = lx_core::error::LxError::BadUsage(format!(
                            "cannot read context file '{}': {e}",
                            canonical.display()
                        ));
                        print_error(&lx_err, cli.json);
                        process::exit(exit::BAD_USAGE);
                    }
                }
            }
        } // end match cli.context
    }; // end context_raw

    // Apply redaction to both question and context.
    let level = lx_redact::RedactLevel::parse(&config.redact.level);

    let question = if cli.no_redact {
        question_raw.clone()
    } else {
        match lx_redact::redact(&question_raw, level) {
            Ok(r) => r,
            Err(e) => {
                let lx_err =
                    lx_core::error::LxError::SecurityAbort(format!("redaction failed: {e}"));
                print_error(&lx_err, cli.json);
                process::exit(exit::SECURITY_ABORT);
            }
        }
    };

    let context: Option<String> = match context_raw {
        None => None,
        Some(raw) => {
            if cli.no_redact {
                Some(raw)
            } else {
                match lx_redact::redact(&raw, level) {
                    Ok(r) => Some(r),
                    Err(e) => {
                        let lx_err = lx_core::error::LxError::SecurityAbort(format!(
                            "redaction of context failed: {e}"
                        ));
                        print_error(&lx_err, cli.json);
                        process::exit(exit::SECURITY_ABORT);
                    }
                }
            }
        }
    };

    // --dry-run: show what would be sent, then exit without calling LLM.
    if cli.dry_run {
        if !cli.quiet {
            eprintln!(
                "[dry-run] question ({} bytes) that would be sent to LLM:",
                question.len()
            );
            eprintln!("{}", question.trim());
            if let Some(ref ctx) = context {
                eprintln!("[dry-run] context ({} bytes):", ctx.len());
                eprintln!("{}", ctx.trim());
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

    match run::run(&question, context.as_deref(), &config, client.as_ref()) {
        Ok(output) => {
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&output).unwrap());
            } else {
                println!("{}", output.answer);
            }
            process::exit(exit::SUCCESS);
        }
        Err(e) => {
            print_error(&e, cli.json);
            process::exit(e.exit_code());
        }
    }
}
