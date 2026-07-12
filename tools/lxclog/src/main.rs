#![forbid(unsafe_code)]

use clap::Parser;
use lx_config::Config;
use lx_core::{error::print_error, exit};
use std::path::PathBuf;
use std::process;

mod run;

fn version_string() -> String {
    lx_core::version::build_version_string("lxclog", env!("CARGO_PKG_VERSION"))
}

#[derive(Parser)]
#[command(
    name    = "lxclog",
    about   = "Generate a Keep-a-Changelog changelog from git log output",
    // Disable clap's built-in --version; we emit the canonical suite format manually.
    disable_version_flag = true
)]
struct Cli {
    /// Output as JSON
    #[arg(long)]
    json: bool,

    /// Disable ANSI colours and formatting
    #[arg(long)]
    plain: bool,

    /// Show the redacted log that would be sent to the LLM, then exit without sending
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

    /// Disable secret redaction (NOT recommended — secrets in git logs will reach the LLM provider)
    #[arg(long)]
    no_redact: bool,

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

    // --no-redact: prominent warning.
    if cli.no_redact && !cli.quiet {
        eprintln!(
            "warning: --no-redact is set. Secrets in your git log will be sent to \
             the LLM provider unmasked. Proceed only if you have audited the log."
        );
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
    let log = lx_core::io::resolve_input(cli.file.as_deref(), max).unwrap_or_else(|e| {
        print_error(&e, cli.json);
        process::exit(e.exit_code());
    });

    if log.trim().is_empty() {
        let e = lx_core::error::LxError::BadUsage(
            "no git log provided; pipe `git log --oneline` or `git log --format=...` into lxclog"
                .to_string(),
        );
        print_error(&e, cli.json);
        process::exit(exit::BAD_USAGE);
    }

    // --dry-run: redact first, then show what *would* be sent, then exit without calling LLM.
    if cli.dry_run {
        let level = lx_redact::RedactLevel::parse(&config.redact.level);
        let redacted = if cli.no_redact {
            log.clone()
        } else {
            match lx_redact::redact(&log, level) {
                Ok(r) => r,
                Err(e) => {
                    let lx_err =
                        lx_core::error::LxError::SecurityAbort(format!("redaction failed: {e}"));
                    print_error(&lx_err, cli.json);
                    process::exit(exit::SECURITY_ABORT);
                }
            }
        };
        if !cli.quiet {
            eprintln!(
                "[dry-run] redacted log ({} bytes) that would be sent to LLM:",
                redacted.len()
            );
            eprintln!("{}", redacted.trim());
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

    let result = if cli.no_redact {
        run::run_no_redact(&log, &config, client.as_ref())
    } else {
        run::run(&log, &config, client.as_ref())
    };

    match result {
        Ok((output, warnings)) => {
            // Tier-2 warnings (e.g. log truncation): shown unless --quiet.
            for w in &warnings {
                lx_core::output::warn(w);
            }
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&output).unwrap());
            } else {
                print!("{}", output.to_plain());
            }
            process::exit(exit::SUCCESS);
        }
        Err(e) => {
            print_error(&e, cli.json);
            process::exit(e.exit_code());
        }
    }
}
