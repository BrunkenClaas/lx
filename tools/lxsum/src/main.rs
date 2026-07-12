#![forbid(unsafe_code)]

use clap::Parser;
use lx_config::Config;
use lx_core::{error::print_error, exit};
use std::path::PathBuf;
use std::process;

mod run;

fn version_string() -> String {
    lx_core::version::build_version_string("lxsum", env!("CARGO_PKG_VERSION"))
}

#[derive(Parser)]
#[command(
    name = "lxsum",
    about = "Summarise a file or command output",
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

    /// Disable secret redaction (NOT recommended — secrets in input will reach the LLM provider)
    #[arg(long)]
    no_redact: bool,

    /// Read input from file instead of stdin
    #[arg(long, value_name = "PATH")]
    file: Option<PathBuf>,

    /// Print version information
    #[arg(short = 'V', long = "version")]
    version: bool,

    // ── Hub flags ────────────────────────────────────────────────────────────
    /// Produce only a one-sentence summary (no bullets or body)
    #[arg(long)]
    short: bool,

    /// Produce a short title/subject line (5–10 words) instead of a summary
    #[arg(long)]
    headline: bool,

    /// Approximate maximum number of words in the summary body
    #[arg(long, value_name = "N")]
    max_words: Option<u32>,

    /// Approximate maximum number of bullet/outline items or prose lines
    #[arg(long, value_name = "N")]
    max_lines: Option<u32>,

    /// Output format: prose | bullets (default) | outline
    #[arg(long, value_name = "FMT", default_value = "bullets")]
    format: String,
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

    // Validate --format early.
    let sum_format = run::SumFormat::parse(&cli.format).unwrap_or_else(|| {
        eprintln!(
            "error[E2]: invalid --format value '{}'; valid values: prose, bullets, outline",
            cli.format
        );
        process::exit(exit::BAD_USAGE);
    });

    // --no-redact: prominent warning.
    if cli.no_redact && !cli.quiet {
        eprintln!(
            "warning: --no-redact is set. Secrets in your input will be sent to \
             the LLM provider unmasked. Proceed only if you have audited the input."
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
        eprintln!("[verbose] format: {}", cli.format);
        if cli.short {
            eprintln!("[verbose] short mode enabled");
        }
    }

    let max = cli.max_input_bytes.unwrap_or(config.limits.max_input_bytes);
    let input = lx_core::io::resolve_input(cli.file.as_deref(), max).unwrap_or_else(|e| {
        print_error(&e, cli.json);
        process::exit(e.exit_code());
    });

    if input.trim().is_empty() {
        let e = lx_core::error::LxError::BadUsage(
            "no input provided; pipe text or a file into lxsum".to_string(),
        );
        print_error(&e, cli.json);
        process::exit(exit::BAD_USAGE);
    }

    // --dry-run: redact first, show what would be sent, exit without calling LLM.
    if cli.dry_run {
        let level = lx_redact::RedactLevel::parse(&config.redact.level);
        let redacted = if cli.no_redact {
            input.clone()
        } else {
            match lx_redact::redact(&input, level) {
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
                "[dry-run] redacted input ({} bytes) that would be sent to LLM:",
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

    let opts = run::SumOptions {
        short: cli.short,
        headline: cli.headline,
        max_words: cli.max_words,
        max_lines: cli.max_lines,
        format: sum_format,
    };

    let result = if cli.no_redact {
        run::run_no_redact_with_opts(&input, &config, client.as_ref(), &opts)
    } else {
        run::run_with_opts(&input, &config, client.as_ref(), &opts)
    };

    match result {
        Ok((output, warnings)) => {
            // Tier-2 warnings (e.g. input truncation): shown unless --quiet.
            for w in &warnings {
                lx_core::output::warn(w);
            }
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&output).unwrap());
            } else if cli.headline {
                // Headline mode: emit the bare subject line (no "Summary:" prefix)
                // so it can be piped into a commit/email subject or title.
                println!("{}", output.to_headline());
            } else {
                println!("{}", output.to_plain());
            }
            process::exit(exit::SUCCESS);
        }
        Err(e) => {
            print_error(&e, cli.json);
            process::exit(e.exit_code());
        }
    }
}
