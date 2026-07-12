#![forbid(unsafe_code)]

use clap::Parser;
use lx_config::Config;
use lx_core::{error::print_error, exit};
use std::path::PathBuf;
use std::process;

mod run;

fn version_string() -> String {
    lx_core::version::build_version_string("lxjwt", env!("CARGO_PKG_VERSION"))
}

#[derive(Parser)]
#[command(
    name    = "lxjwt",
    about   = "Decode and explain a JWT token's claims",
    // Disable clap's built-in --version; we emit the canonical suite format manually.
    disable_version_flag = true
)]
struct Cli {
    /// JWT token to decode (reads from stdin if omitted)
    input: Option<String>,

    /// Output as JSON
    #[arg(long)]
    json: bool,

    /// Disable ANSI colours and formatting
    #[arg(long)]
    plain: bool,

    /// Show the decoded claims that would be sent to the LLM, then exit without sending
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

    /// Read input from file instead of stdin
    #[arg(long, value_name = "PATH")]
    file: Option<PathBuf>,

    /// Disable secret redaction of decoded claims (NOT recommended)
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

    // Collect input: positional arg > --file > stdin.
    let input = if let Some(s) = cli.input {
        s
    } else {
        let max = cli.max_input_bytes.unwrap_or(config.limits.max_input_bytes);
        lx_core::io::resolve_input(cli.file.as_deref(), max).unwrap_or_else(|e| {
            print_error(&e, cli.json);
            process::exit(e.exit_code());
        })
    };

    if input.trim().is_empty() {
        let e = lx_core::error::LxError::BadUsage(
            "no JWT provided; pipe or pass the JWT token as input".to_string(),
        );
        print_error(&e, cli.json);
        process::exit(exit::BAD_USAGE);
    }

    // --dry-run: decode locally and show what would be sent, then exit.
    if cli.dry_run {
        if !cli.quiet {
            match run::split_and_decode_jwt(input.trim()) {
                Ok((header, payload)) => {
                    eprintln!("[dry-run] decoded header:  {header}");
                    eprintln!("[dry-run] decoded payload: {payload}");
                    eprintln!("[dry-run] signature discarded — not sent to LLM");
                }
                Err(e) => {
                    eprintln!("[dry-run] {e}");
                }
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

    let result = if cli.no_redact {
        run::run_no_redact(&input, &config, client.as_ref())
    } else {
        run::run(&input, &config, client.as_ref())
    };

    match result {
        Ok(output) => {
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&output).unwrap());
            } else {
                // Plain mode: result (header+payload+notes) → stdout.
                // Explanation is the result for this tool (it IS the purpose).
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
