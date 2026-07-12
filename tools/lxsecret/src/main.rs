#![forbid(unsafe_code)]

use clap::Parser;
use lx_config::Config;
use lx_core::{error::print_error, exit};
use std::path::PathBuf;
use std::process;

mod run;

fn version_string() -> String {
    lx_core::version::build_version_string("lxsecret", env!("CARGO_PKG_VERSION"))
}

#[derive(Parser)]
#[command(
    name  = "lxsecret",
    about = "Scan for accidentally committed secrets and API keys",
    // Disable clap's built-in --version; we emit the canonical suite format manually.
    disable_version_flag = true
)]
struct Cli {
    /// Path to a file or directory to scan (reads from stdin if omitted)
    #[arg(value_name = "PATH")]
    path: Option<PathBuf>,

    /// Output as JSON
    #[arg(long)]
    json: bool,

    /// Disable ANSI colours and formatting
    #[arg(long)]
    plain: bool,

    /// Show what would be scanned without calling the LLM
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

    /// Maximum bytes to read from stdin or per file
    #[arg(long)]
    max_input_bytes: Option<usize>,

    /// Read input from file instead of stdin (alias for PATH argument)
    #[arg(long, value_name = "PATH")]
    file: Option<PathBuf>,

    /// Skip LLM classification — perform local detection only (faster, no API key needed)
    #[arg(long)]
    no_llm: bool,

    /// Acknowledged: lxsecret always masks output; this flag is a no-op kept for suite consistency
    #[arg(long)]
    no_redact: bool,

    /// Enable strict scanning: also sweeps for high-entropy tokens without a keyword nearby,
    /// Bearer tokens, bare connection-string passwords, and additional niche formats
    #[arg(long)]
    strict: bool,

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

    // --no-redact: prominent informational warning.
    // lxsecret always masks secrets in output and never sends values to the LLM.
    if cli.no_redact && !cli.quiet {
        eprintln!(
            "warning: --no-redact has no effect on lxsecret. \
             Secret values are always masked and never sent to the LLM."
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
            "[verbose] config: model={} provider={} lang={}",
            config.effective_model(),
            config.llm.provider,
            config.output.lang
        );
        eprintln!("[verbose] llm-classify: {}", !cli.no_llm);
        eprintln!("[verbose] strict: {}", cli.strict);
    }

    let max = cli.max_input_bytes.unwrap_or(config.limits.max_input_bytes);

    // Determine scan mode: directory, file, or stdin.
    // Priority: positional PATH > --file > stdin.
    let scan_path = cli.path.as_deref().or(cli.file.as_deref());

    // --dry-run: show what would be scanned, then exit.
    if cli.dry_run {
        if !cli.quiet {
            match scan_path {
                Some(p) => eprintln!("[dry-run] would scan path: {}", p.display()),
                None => eprintln!("[dry-run] would scan stdin"),
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

    // Build LLM client only when classification is desired.
    let maybe_client: Option<Box<dyn lx_llm::LlmClient>> = if cli.no_llm {
        None
    } else {
        match lx_llm::client_from_config(&config, cli.verbose) {
            Ok(c) => Some(c),
            Err(e) => {
                print_error(&e, cli.json);
                process::exit(e.exit_code());
            }
        }
    };
    let client_ref: Option<&dyn lx_llm::LlmClient> = maybe_client.as_deref();

    let output = if let Some(path) = scan_path {
        let meta = std::fs::metadata(path).unwrap_or_else(|e| {
            let err =
                lx_core::error::LxError::BadUsage(format!("cannot access {}: {e}", path.display()));
            print_error(&err, cli.json);
            process::exit(exit::BAD_USAGE);
        });

        if meta.is_dir() {
            // fsbound: scan within this directory only.
            run::scan_directory(path, &config, client_ref, max, cli.strict).unwrap_or_else(|e| {
                print_error(&e, cli.json);
                process::exit(e.exit_code());
            })
        } else {
            // Single file — read content then scan.
            let parent = path.parent().unwrap_or(path);
            let content = lx_core::io::read_file(path, max, Some(parent)).unwrap_or_else(|e| {
                print_error(&e, cli.json);
                process::exit(e.exit_code());
            });
            scan_text_with_client(
                &content,
                path.display().to_string(),
                &config,
                client_ref,
                cli.strict,
            )
        }
    } else {
        // Stdin scan.
        let input = lx_core::io::resolve_input(None, max).unwrap_or_else(|e| {
            print_error(&e, cli.json);
            process::exit(e.exit_code());
        });
        scan_text_with_client(&input, "stdin".to_string(), &config, client_ref, cli.strict)
    };

    // Emit result.
    let count = output.findings.len();
    if cli.json {
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
    } else {
        let plain = output.to_plain();
        if !plain.is_empty() {
            print!("{plain}");
        }
    }

    if lx_core::output::show_narration(cli.quiet, cli.verbose) {
        if count == 0 {
            eprintln!("No secrets found.");
        } else {
            eprintln!("{count} potential secret(s) found. Review the findings above.");
        }
    }

    process::exit(exit::SUCCESS);
}

/// Scan text content and classify findings via LLM (or locally if client is None).
fn scan_text_with_client(
    input: &str,
    _source_name: String,
    config: &Config,
    client: Option<&dyn lx_llm::LlmClient>,
    strict: bool,
) -> run::Output {
    match client {
        Some(cl) => run::run(input, config, cl, strict).unwrap_or(run::Output { findings: vec![] }),
        None => run::Output {
            findings: run::run_local(input, strict),
        },
    }
}
