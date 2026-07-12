#![forbid(unsafe_code)]

use clap::{Parser, Subcommand};
use lx::{catalog::TOOLS, config_cmd, model, render};
use lx_config::Config;
use lx_core::error::print_error;
use lx_core::{exit, platform};
use std::process;

fn version_string() -> String {
    lx_core::version::build_version_string("lx", env!("CARGO_PKG_VERSION"))
}

#[derive(Parser)]
#[command(
    name = "lx",
    about = "LX Coreutils umbrella — discover and explore all 72 tools",
    disable_version_flag = true
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Print version information
    #[arg(short = 'V', long = "version")]
    version: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Browse and search the tool catalog
    Tools(ToolsArgs),

    /// Report the effective LLM model the suite will use
    Model(ModelArgs),

    /// Interactive wizard to create or update the user config file
    Config(ConfigArgs),
}

#[derive(Parser)]
struct ConfigArgs {
    /// Accept all defaults non-interactively (writes without prompting)
    #[arg(short = 'y', long)]
    yes: bool,

    /// Print the resulting TOML to stdout; do not write a file
    #[arg(long)]
    print: bool,

    /// Skip the overwrite confirmation when the file already exists
    #[arg(long)]
    force: bool,
}

#[derive(Parser)]
struct ModelArgs {
    /// Output as JSON: {"model","provider","reachable","error"}
    #[arg(long)]
    json: bool,

    /// Skip the live verification call; report resolved config only
    #[arg(long)]
    no_verify: bool,

    /// Show config/connection diagnostics on stderr
    #[arg(long)]
    verbose: bool,
}

#[derive(Parser)]
struct ToolsArgs {
    /// Keyword to search (substring match over name and purpose)
    keyword: Option<String>,

    /// Show only one category (short id or name substring, e.g. "code", "security")
    #[arg(long, value_name = "CAT")]
    cat: Option<String>,

    /// Output as JSON array
    #[arg(long)]
    json: bool,

    /// Disable ANSI color and formatting
    #[arg(long)]
    plain: bool,
}

fn main() {
    let cli = Cli::parse();

    platform::enable_ansi();

    if cli.version {
        println!("{}", version_string());
        process::exit(exit::SUCCESS);
    }

    // `lx model` — diagnostic: report the effective model (may call the LLM
    // to verify reachability). lx produces no LLM *content*; here the LLM is
    // contacted only to confirm the resolved model answers. See design §13.13.
    if let Some(Commands::Model(m)) = &cli.command {
        run_model(m);
        process::exit(exit::SUCCESS);
    }

    // `lx config` — interactive wizard to create or update the user config
    // file. Writes the user config path only; never touches project-local .lx.toml.
    if let Some(Commands::Config(c)) = &cli.command {
        let code = config_cmd::run(&config_cmd::ConfigArgs {
            yes: c.yes,
            print: c.print,
            force: c.force,
        });
        process::exit(code);
    }

    // No subcommand → print help and exit.
    let args = match cli.command {
        Some(Commands::Tools(a)) => a,
        Some(Commands::Model(_)) | Some(Commands::Config(_)) => unreachable!("handled above"),
        None => {
            let mut cmd = <Cli as clap::CommandFactory>::command();
            let _ = cmd.print_help();
            println!();
            process::exit(exit::SUCCESS);
        }
    };

    let color = !args.plain && platform::is_tty(platform::Fd::Stdout);

    // Resolve which tools to show.
    let tools: Vec<&lx::catalog::ToolEntry> = if let Some(ref kw) = args.keyword {
        render::filter_by_keyword(kw)
    } else {
        render::filter_by_cat(args.cat.as_deref())
    };

    if tools.is_empty() {
        eprintln!(
            "No tools found. Try `lx tools` for the full list or `lx tools --cat <category>`."
        );
        process::exit(exit::SUCCESS);
    }

    if args.json {
        println!("{}", render::render_json(&tools));
        process::exit(exit::SUCCESS);
    }

    let output = if args.keyword.is_some() {
        // Keyword search: flat hit list showing full purpose text.
        render::render_hits(&tools, color)
    } else {
        // Full or category view: compact multi-column grouped layout.
        let width = if color { render::term_columns() } else { 80 };
        render::render_grouped(&tools, color, width)
    };

    if !output.is_empty() {
        print!("{output}");
        // Print tool count footer.
        let total = TOOLS.len();
        if color {
            eprintln!(
                "\x1b[2m{} of {} tools shown. lx tools --help for options.\x1b[0m",
                tools.len(),
                total
            );
        }
    }

    process::exit(exit::SUCCESS);
}

/// Handle `lx model`. Loads config, resolves the effective model, and
/// (unless `--no-verify`) makes a minimal LLM call to confirm reachability.
///
/// Plain mode: the **model name** goes to stdout (one line, pipe-safe) so a
/// script can do `MODEL=$(lx model --no-verify)`. The provider and
/// reachability go to stderr as diagnostics. `--json` emits the full object to
/// stdout. A failed verification call exits non-zero so scripts can detect it.
fn run_model(args: &ModelArgs) {
    let config = Config::load().unwrap_or_else(|e| {
        print_error(&e, args.json);
        process::exit(exit::LOGICAL_ERROR);
    });

    let info = model::probe(&config, args.no_verify, args.verbose).unwrap_or_else(|e| {
        print_error(&e, args.json);
        process::exit(e.exit_code());
    });

    if args.json {
        println!("{}", serde_json::to_string(&info).unwrap());
    } else {
        // stdout: just the model name — pipe-safe, the one thing a script wants.
        println!("{}", info.model);
        // stderr: provider + reachability diagnostics.
        match info.reachable {
            Some(true) => eprintln!("# provider: {} | reachable: yes", info.provider),
            Some(false) => eprintln!(
                "# provider: {} | reachable: NO ({})",
                info.provider,
                info.error.as_deref().unwrap_or("unknown error")
            ),
            None => eprintln!("# provider: {} | reachable: (not checked)", info.provider),
        }
    }

    // A failed verification is a real error: exit non-zero so the harness knows
    // the configured model is not actually answering.
    if info.reachable == Some(false) {
        process::exit(exit::LOGICAL_ERROR);
    }
}
