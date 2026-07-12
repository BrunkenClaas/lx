#![forbid(unsafe_code)]

use clap::Parser;
use lx_config::Config;
use lx_core::{error::print_error, exit};
use std::path::PathBuf;
use std::process;

mod run;

fn version_string() -> String {
    lx_core::version::build_version_string("lxdraft", env!("CARGO_PKG_VERSION"))
}

#[derive(Parser)]
#[command(
    name = "lxdraft",
    about = "Draft an email, ticket, reply, or message from bullet points",
    // Disable clap's built-in --version; we emit the canonical suite format manually.
    disable_version_flag = true
)]
struct Cli {
    /// Input keywords or bullet points (alternatively pass via stdin or --file)
    input: Option<String>,

    /// Draft kind: email, ticket, reply, or message
    #[arg(long, default_value = "email", value_name = "KIND")]
    kind: String,

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

    // Validate --kind value.
    let kind = cli.kind.to_lowercase();
    if !matches!(kind.as_str(), "email" | "ticket" | "reply" | "message") {
        let e = lx_core::error::LxError::BadUsage(format!(
            "invalid --kind '{}'; expected one of: email, ticket, reply, message",
            kind
        ));
        print_error(&e, cli.json);
        process::exit(exit::BAD_USAGE);
    }

    // --no-redact: prominent warning.
    if cli.no_redact && !cli.quiet {
        eprintln!(
            "warning: --no-redact is set. Sensitive content in your input will be sent to \
             the LLM provider unmasked. Proceed only if you have reviewed the input."
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
        eprintln!("[verbose] kind: {}", kind);
    }

    let max = cli.max_input_bytes.unwrap_or(config.limits.max_input_bytes);

    // Resolve input: positional arg takes priority, then --file / stdin.
    let input = if let Some(s) = cli.input {
        s
    } else {
        lx_core::io::resolve_input(cli.file.as_deref(), max).unwrap_or_else(|e| {
            print_error(&e, cli.json);
            process::exit(e.exit_code());
        })
    };

    if input.trim().is_empty() {
        let e = lx_core::error::LxError::BadUsage(
            "no input provided; pass bullet points as argument, --file, or via stdin".to_string(),
        );
        print_error(&e, cli.json);
        process::exit(exit::BAD_USAGE);
    }

    // --dry-run: redact first, show what would be sent, then exit.
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

    let result = if cli.no_redact {
        run::run_no_redact(&input, &kind, &config, client.as_ref())
    } else {
        run::run(&input, &kind, &config, client.as_ref())
    };

    match result {
        Ok(output) => {
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&output).unwrap());
            } else {
                // subject → stderr (diagnostic, not part of the pipe result)
                if let Some(ref subj) = output.subject {
                    if lx_core::output::show_narration(cli.quiet, cli.verbose) {
                        eprintln!("Subject: {}", subj);
                    }
                }
                // body → stdout (the result)
                println!("{}", output.body);
                if lx_core::output::show_narration(cli.quiet, cli.verbose) {
                    eprintln!("# Generated {} draft", kind);
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
