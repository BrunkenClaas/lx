#![forbid(unsafe_code)]

use clap::Parser;
use lx_config::Config;
use lx_core::{error::print_error, exit};
use std::path::PathBuf;
use std::process;

mod fetch;
mod run;

fn version_string() -> String {
    lx_core::version::build_version_string("lxurl", env!("CARGO_PKG_VERSION"))
}

#[derive(Parser)]
#[command(
    name = "lxurl",
    about = "Fetch a URL and answer questions about its content",
    disable_version_flag = true
)]
struct Cli {
    /// URL to fetch
    url: Option<String>,

    /// Question to answer about the page (default: summarise the page)
    #[arg(long, value_name = "QUESTION")]
    question: Option<String>,

    /// Output as JSON
    #[arg(long)]
    json: bool,

    /// Disable ANSI colours and formatting
    #[arg(long)]
    plain: bool,

    /// Show the fetched text that would be sent to the LLM, then exit without sending
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

    /// Maximum bytes to fetch from the URL
    #[arg(long)]
    max_input_bytes: Option<usize>,

    /// Unused — accepted for pipeline compatibility
    #[arg(long, value_name = "PATH", hide = true)]
    file: Option<PathBuf>,

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

    let url = match cli.url {
        Some(u) => u,
        None => {
            let e =
                lx_core::error::LxError::BadUsage("missing required argument: <URL>".to_string());
            print_error(&e, cli.json);
            process::exit(exit::BAD_USAGE);
        }
    };

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
        eprintln!("[verbose] url: {}", url);
    }

    // Validate URL early (SSRF check) before any network activity.
    if let Err(e) = fetch::validate_url(&url) {
        print_error(&e, cli.json);
        process::exit(exit::BAD_USAGE);
    }

    let question = cli.question.unwrap_or_default();

    if cli.dry_run {
        // Fetch and show extracted text, then exit without calling LLM.
        let max_bytes = cli.max_input_bytes.unwrap_or(fetch::DEFAULT_MAX_URL_BYTES);
        match fetch::fetch_and_extract(&url, max_bytes, config.llm.timeout_secs) {
            Ok((text, truncated)) => {
                if !cli.quiet {
                    eprintln!(
                        "[dry-run] fetched {} bytes (truncated={truncated}); first 500 chars:",
                        text.len()
                    );
                    let preview = &text[..text.len().min(500)];
                    eprintln!("{preview}");
                }
            }
            Err(e) => {
                print_error(&e, cli.json);
                process::exit(e.exit_code());
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

    match run::run(&url, &question, &config, client.as_ref()) {
        Ok(output) => {
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
