#![forbid(unsafe_code)]

use clap::Parser;
use lx_config::Config;
use lx_core::{error::print_error, exit};
use std::path::PathBuf;
use std::process;

mod run;

/// Build the canonical version string once at startup.
/// Format: "lxperm 1.0.0 (lx-coreutils 2026-07, <target>)"
fn version_string() -> String {
    lx_core::version::build_version_string("lxperm", env!("CARGO_PKG_VERSION"))
}

#[derive(Parser)]
#[command(
    name = "lxperm",
    about = "Explain file permissions and their security risks",
    // Disable clap's built-in --version; we handle it manually to produce the
    // canonical "lxperm X.Y.Z (lx-coreutils YYYY-MM, <target>)" format.
    disable_version_flag = true
)]
struct Cli {
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

    /// Read input from a file or scan a directory path (fsbound)
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

    let max = cli.max_input_bytes.unwrap_or(config.limits.max_input_bytes);

    // Input: --file path (fsbound: if directory, scan it) OR stdin (ls -l output).
    let input = if let Some(ref path) = cli.file {
        if path.is_dir() {
            // fsbound: scan directory, produce synthetic ls -l style output.
            scan_directory(path, max, cli.json)
        } else {
            // fsbound: read file, enforcing the file's parent as root.
            let root = path.parent().unwrap_or(path.as_path());
            lx_core::io::read_file(path, max, Some(root)).unwrap_or_else(|e| {
                print_error(&e, cli.json);
                process::exit(e.exit_code());
            })
        }
    } else {
        // Read ls -l output from stdin.
        lx_core::io::resolve_input(None, max).unwrap_or_else(|e| {
            print_error(&e, cli.json);
            process::exit(e.exit_code());
        })
    };

    if input.trim().is_empty() {
        let e = lx_core::error::LxError::BadUsage("no input provided".to_string());
        print_error(&e, cli.json);
        process::exit(exit::BAD_USAGE);
    }

    // --dry-run: show what would be sent, then exit.
    if cli.dry_run {
        if !cli.quiet {
            eprintln!("[dry-run] input ({} bytes):", input.len());
            eprintln!("{}", input.trim());
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

    match run::run(&input, &config, client.as_ref()) {
        Ok(output) => {
            if cli.json {
                // Full JSON object → stdout (consumer parses fields explicitly).
                println!("{}", serde_json::to_string_pretty(&output).unwrap());
            } else {
                // Explanation IS the result for lxperm — goes to stdout.
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

/// Scan a directory and produce ls -l style output for its direct entries.
///
/// fsbound: only lists entries within the given directory.
/// Symlinks that would escape are noted but not followed.
fn scan_directory(dir: &std::path::Path, _max_bytes: usize, json: bool) -> String {
    let canonical_dir = match std::fs::canonicalize(dir) {
        Ok(p) => p,
        Err(e) => {
            let err = lx_core::error::LxError::BadUsage(format!(
                "cannot resolve directory {}: {e}",
                dir.display()
            ));
            print_error(&err, json);
            std::process::exit(exit::BAD_USAGE);
        }
    };

    let entries = match std::fs::read_dir(&canonical_dir) {
        Ok(e) => e,
        Err(e) => {
            let err = lx_core::error::LxError::BadUsage(format!(
                "cannot read directory {}: {e}",
                canonical_dir.display()
            ));
            print_error(&err, json);
            std::process::exit(exit::BAD_USAGE);
        }
    };

    let mut lines = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();

        // fsbound: verify the resolved entry stays within the scanned dir.
        if let Ok(canon) = std::fs::canonicalize(&path) {
            if !canon.starts_with(&canonical_dir) {
                eprintln!(
                    "warning: skipping {} — resolves outside scan root",
                    path.display()
                );
                continue;
            }
        }

        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };

        let perm_str = format_permissions(&meta);
        let size = meta.len();
        let name = entry.file_name().to_string_lossy().into_owned();

        // Produce a minimal ls -l style line.
        // Format: <perm> 1 owner group <size> Jan  1 00:00 <name>
        lines.push(format!(
            "{} 1 owner group {:>10} Jan  1 00:00 {}",
            perm_str, size, name
        ));
    }

    lines.join("\n")
}

/// Format file metadata as a Unix-style 10-character permission string.
///
/// On Windows this is approximate:
/// - type char: 'd' (dir), 'l' (symlink), '-' (file)
/// - read-only metadata maps to no owner write bit
/// - group/other permissions are approximated (Windows has no direct equivalent)
fn format_permissions(meta: &std::fs::Metadata) -> String {
    let ft = meta.file_type();
    let is_dir = ft.is_dir();
    let is_link = ft.is_symlink();
    let writable = !meta.permissions().readonly();

    let type_char = if is_dir {
        'd'
    } else if is_link {
        'l'
    } else {
        '-'
    };
    let owner_w = if writable { 'w' } else { '-' };
    let owner_x = if is_dir { 'x' } else { '-' };
    // Group/other: dirs allow traversal; files are read-only for group/other.
    let group_w = if writable && is_dir { 'w' } else { '-' };
    let group_x = if is_dir { 'x' } else { '-' };
    let other_x = if is_dir { 'x' } else { '-' };

    format!("{type_char}r{owner_w}{owner_x}r{group_w}{group_x}r-{other_x}")
}
