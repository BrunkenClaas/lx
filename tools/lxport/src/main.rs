#![forbid(unsafe_code)]

use clap::Parser;
use lx_config::Config;
use lx_core::{
    error::{print_error, LxError},
    exit,
};
use std::path::PathBuf;
use std::process;

mod run;

fn version_string() -> String {
    lx_core::version::build_version_string("lxport", env!("CARGO_PKG_VERSION"))
}

#[derive(Parser)]
#[command(
    name = "lxport",
    about = "Explain what service runs on a port and flag any risk",
    disable_version_flag = true
)]
struct Cli {
    /// Port number to look up (1-65535)
    port: Option<String>,

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

    /// Read network context (ss/netstat output) from file instead of stdin
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

    // Validate port argument.
    let port_str = cli.port.unwrap_or_else(|| {
        let e = LxError::BadUsage("port number required".to_string());
        print_error(&e, cli.json);
        process::exit(exit::BAD_USAGE);
    });

    let port: u16 = port_str.parse().unwrap_or_else(|_| {
        let e = LxError::BadUsage(format!(
            "'{}' is not a valid port number (1-65535)",
            port_str
        ));
        print_error(&e, cli.json);
        process::exit(exit::BAD_USAGE);
    });

    if port == 0 {
        let e = LxError::BadUsage("port 0 is not a valid service port".to_string());
        print_error(&e, cli.json);
        process::exit(exit::BAD_USAGE);
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

    // Collect optional network context: --file or piped stdin.
    let context = if cli.file.is_some() || !lx_core::platform::is_tty(lx_core::platform::Fd::Stdin)
    {
        let max = cli.max_input_bytes.unwrap_or(config.limits.max_input_bytes);
        lx_core::io::resolve_input(cli.file.as_deref(), max).unwrap_or_default()
    } else {
        String::new()
    };

    // --dry-run: show what would be sent, then exit.
    if cli.dry_run {
        if !cli.quiet {
            let user_msg = if context.trim().is_empty() {
                format!("Port: {port}")
            } else {
                format!(
                    "Port: {port}\n\nNetwork context (from ss/netstat):\n{}",
                    context.trim()
                )
            };
            eprintln!("[dry-run] input ({} bytes):", user_msg.len());
            eprintln!("{}", user_msg);
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

    if context.is_empty() && !cli.quiet {
        eprintln!("# tip: pipe ss/netstat output for machine-specific results (e.g. ss -tlnp | lxport {port})");
    }

    match run::run(port, &context, &config, client.as_ref()) {
        Ok(output) => {
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&output).unwrap());
            } else {
                print!("{}", output.to_plain());
                if lx_core::output::show_narration(cli.quiet, cli.verbose) {
                    eprintln!(
                        "# service: {} | risk: {}",
                        output.likely_service, output.risk
                    );
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
