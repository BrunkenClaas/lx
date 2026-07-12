#![forbid(unsafe_code)]

use clap::Parser;
use lx_config::Config;
use lx_core::{error::print_error, exit};
use std::process;

mod run;

fn version_string() -> String {
    lx_core::version::build_version_string("lxpatch", env!("CARGO_PKG_VERSION"))
}

#[derive(Parser)]
#[command(
    name = "lxpatch",
    about = "Turn a described change into an applyable unified diff",
    disable_version_flag = true
)]
struct Cli {
    /// Description of the change to make
    description: Option<String>,
    #[arg(long)]
    json: bool,
    #[arg(long)]
    plain: bool,
    #[arg(long)]
    dry_run: bool,
    #[arg(short, long)]
    quiet: bool,
    #[arg(long, default_value = "auto")]
    lang: String,
    #[arg(long)]
    verbose: bool,
    #[arg(long)]
    max_input_bytes: Option<usize>,
    #[arg(long, value_name = "PATH")]
    file: Option<std::path::PathBuf>,
    #[arg(short = 'V', long = "version")]
    version: bool,
    #[arg(short = 'D', long)]
    allow_dangerous: bool,
}

fn main() {
    let cli = Cli::parse();
    lx_core::platform::enable_ansi();
    lx_core::output::set_quiet(cli.quiet);

    if cli.version {
        println!("{}", version_string());
        process::exit(exit::SUCCESS);
    }

    let description = match cli.description {
        Some(d) => d,
        None => {
            let e = lx_core::error::LxError::BadUsage(
                "no change description provided; pass the change as an argument".to_string(),
            );
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
            "[verbose] model={} provider={}",
            config.effective_model(),
            config.llm.provider
        );
    }

    let max = cli.max_input_bytes.unwrap_or(config.limits.max_input_bytes);
    let file_content = lx_core::io::resolve_input(cli.file.as_deref(), max).unwrap_or_else(|e| {
        print_error(&e, cli.json);
        process::exit(e.exit_code());
    });

    if cli.dry_run {
        if !cli.quiet {
            eprintln!(
                "[dry-run] input ({} bytes):\n{}",
                file_content.len(),
                file_content.trim()
            );
            eprintln!(
                "[dry-run] system prompt:\n{}",
                lx_llm::inject_lang(run::SYSTEM_TEMPLATE, &config.output.lang)
            );
        }
        process::exit(exit::SUCCESS);
    }

    let client = lx_llm::client_from_config(&config, cli.verbose).unwrap_or_else(|e| {
        print_error(&e, cli.json);
        process::exit(e.exit_code());
    });

    match run::run(&file_content, &description, &config, client.as_ref()) {
        Ok(output) => {
            if output.dangerous {
                eprintln!(
                    "DANGER: The generated diff contains a potentially destructive pattern. \
                     Review carefully before applying."
                );
            }
            if output.dangerous && !cli.allow_dangerous {
                process::exit(exit::DANGEROUS);
            }
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&output).unwrap());
            } else {
                println!("{}", output.to_plain());
                if lx_core::output::show_narration(cli.quiet, cli.verbose) {
                    eprintln!("# {}", output.summary);
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
