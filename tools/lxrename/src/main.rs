#![forbid(unsafe_code)]

use clap::Parser;
use lx_config::Config;
use lx_core::{error::print_error, exit};
use std::path::PathBuf;
use std::process;

mod danger;
mod run;

fn version_string() -> String {
    lx_core::version::build_version_string("lxrename", env!("CARGO_PKG_VERSION"))
}

#[derive(Parser)]
#[command(
    name    = "lxrename",
    about   = "Generate a safe rename script from natural-language intent",
    // Disable clap's built-in --version; we emit the canonical suite format manually.
    disable_version_flag = true
)]
struct Cli {
    /// The rename intent (e.g. "rename test files to use snake_case")
    intent: Option<String>,

    /// Directory to list files from (instead of stdin)
    #[arg(long = "in", value_name = "PATH")]
    in_path: Option<PathBuf>,

    /// Output as JSON
    #[arg(long)]
    json: bool,

    /// Disable ANSI colours and formatting
    #[arg(long)]
    plain: bool,

    /// Show what would be sent to the LLM without actually sending it
    #[arg(long)]
    dry_run: bool,

    /// Suppress diagnostic messages on stderr (safety warnings are never suppressed)
    #[arg(short, long)]
    quiet: bool,

    /// Output language (BCP-47, e.g. 'en', 'de', 'fr')
    #[arg(long, default_value = "auto")]
    lang: String,

    /// Show verbose diagnostics on stderr
    #[arg(long)]
    verbose: bool,

    /// Maximum bytes to read from stdin
    #[arg(long)]
    max_input_bytes: Option<usize>,

    /// Read file list from file instead of stdin
    #[arg(long, value_name = "PATH")]
    file: Option<PathBuf>,

    /// Walk subdirectories recursively (requires --in); files listed as relative paths
    #[arg(long, short = 'r')]
    recursive: bool,

    /// Accept dangerous output and exit 0 instead of 3 (warning still printed to stderr)
    #[arg(long, short = 'D')]
    allow_dangerous: bool,

    /// Print version information
    #[arg(short = 'V', long = "version")]
    version: bool,
}

/// Collect files under `dir`, returning `(relative_path, annotation)` pairs.
/// `root` is the top-level directory for computing relative paths.
fn collect_files(
    root: &std::path::Path,
    dir: &std::path::Path,
    recursive: bool,
) -> Result<Vec<(String, String)>, std::io::Error> {
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            if recursive {
                let sub = collect_files(root, &entry.path(), recursive)?;
                entries.extend(sub);
            }
        } else if file_type.is_file() {
            let abs = entry.path();
            let rel = abs
                .strip_prefix(root)
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                .unwrap_or_else(|_| entry.file_name().to_string_lossy().to_string());
            let meta = entry.metadata()?;
            let size = meta.len();
            let created = meta
                .created()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| fmt_unix_timestamp(d.as_secs()))
                .unwrap_or_else(|| "unknown".to_string());
            let modified = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| fmt_unix_timestamp(d.as_secs()))
                .unwrap_or_else(|| "unknown".to_string());
            let annotation = format!("created={}  modified={}  size={}", created, modified, size);
            entries.push((rel, annotation));
        }
    }
    Ok(entries)
}

/// Format a Unix timestamp (seconds since epoch) as `YYYY-MM-DDTHH:MM:SS` (UTC).
fn fmt_unix_timestamp(secs: u64) -> String {
    // Hand-rolled to avoid pulling in chrono. Good enough for display purposes.
    const SECS_PER_MIN: u64 = 60;
    const SECS_PER_HOUR: u64 = 3600;
    const SECS_PER_DAY: u64 = 86400;

    let ss = secs % SECS_PER_MIN;
    let mm = (secs / SECS_PER_MIN) % 60;
    let hh = (secs / SECS_PER_HOUR) % 24;

    let mut days = secs / SECS_PER_DAY;
    // Compute year/month/day from days since 1970-01-01 using the Gregorian calendar.
    let mut year = 1970u64;
    loop {
        let days_in_year = if is_leap(year) { 366 } else { 365 };
        if days < days_in_year {
            break;
        }
        days -= days_in_year;
        year += 1;
    }
    let leap = is_leap(year);
    let month_days: [u64; 12] = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut month = 0usize;
    for &md in &month_days {
        if days < md {
            break;
        }
        days -= md;
        month += 1;
    }
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
        year,
        month + 1,
        days + 1,
        hh,
        mm,
        ss
    )
}

fn is_leap(year: u64) -> bool {
    (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400)
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

    // Resolve intent: must be the positional arg.
    let intent = match cli.intent {
        Some(s) => s,
        None => {
            let e = lx_core::error::LxError::BadUsage(
                "no rename intent provided; pass intent as positional argument".to_string(),
            );
            print_error(&e, cli.json);
            process::exit(exit::BAD_USAGE);
        }
    };

    if intent.trim().is_empty() {
        let e = lx_core::error::LxError::BadUsage("no rename intent provided".to_string());
        print_error(&e, cli.json);
        process::exit(exit::BAD_USAGE);
    }

    if cli.recursive && cli.in_path.is_none() {
        let e = lx_core::error::LxError::BadUsage("--recursive requires --in <path>".to_string());
        print_error(&e, cli.json);
        process::exit(exit::BAD_USAGE);
    }

    // Collect file list: --in <path> lists directory files with metadata, else read stdin/--file.
    let file_list = if let Some(ref in_path) = cli.in_path {
        let mut entries: Vec<(String, String)> = collect_files(in_path, in_path, cli.recursive)
            .unwrap_or_else(|err| {
                let e = lx_core::error::LxError::LogicalError(format!(
                    "cannot read directory '{}': {}",
                    in_path.display(),
                    err
                ));
                print_error(&e, cli.json);
                process::exit(exit::LOGICAL_ERROR);
            });
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        entries
            .into_iter()
            .map(|(name, ann)| format!("{}  {}", name, ann))
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        // Read from --file or stdin.
        if cli.file.is_none() && lx_core::platform::is_tty(lx_core::platform::Fd::Stdin) {
            let e = lx_core::error::LxError::BadUsage(
                "no input: provide files via stdin or --in <path>".to_string(),
            );
            print_error(&e, cli.json);
            process::exit(exit::BAD_USAGE);
        }
        let max = cli.max_input_bytes.unwrap_or(config.limits.max_input_bytes);
        lx_core::io::resolve_input(cli.file.as_deref(), max).unwrap_or_else(|e| {
            print_error(&e, cli.json);
            process::exit(e.exit_code());
        })
    };

    if file_list.trim().is_empty() {
        let e = lx_core::error::LxError::BadUsage(
            "no file list provided; pipe a file list or use --in <path>".to_string(),
        );
        print_error(&e, cli.json);
        process::exit(exit::BAD_USAGE);
    }

    // --dry-run: show what would be sent, then exit.
    if cli.dry_run {
        let combined = format!("Intent: {}\n\nFiles:\n{}", intent.trim(), file_list.trim());
        if !cli.quiet {
            eprintln!("[dry-run] input ({} bytes):", combined.len());
            eprintln!("{}", combined);
        }
        if !cli.quiet {
            eprintln!("[dry-run] system prompt:");
            eprintln!(
                "{}",
                lx_llm::inject_lang(run::SYSTEM_TEMPLATE, &config.output.lang)
                    .replace("{today}", &run::today_utc())
            );
        }
        process::exit(exit::SUCCESS);
    }

    let client = lx_llm::client_from_config(&config, cli.verbose).unwrap_or_else(|e| {
        print_error(&e, cli.json);
        process::exit(e.exit_code());
    });

    let dir_name: Option<String> = cli.in_path.as_ref().and_then(|p| {
        p.canonicalize()
            .unwrap_or_else(|_| p.clone())
            .file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_string())
    });

    match run::run(
        &file_list,
        &intent,
        dir_name.as_deref(),
        &config,
        client.as_ref(),
    ) {
        Ok(mut output) => {
            // fsbound check: determine canonical root.
            let root = if let Some(ref in_path) = cli.in_path {
                std::fs::canonicalize(in_path).unwrap_or_else(|_| in_path.clone())
            } else {
                std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
            };

            // Filter out any rename pairs where from or to escape the root.
            let filtered: Vec<run::Rename> = output
                .renames
                .into_iter()
                .filter(|r| {
                    let from_path = root.join(&r.from);
                    let to_path = root.join(&r.to);
                    // Check that both from and to stay within root.
                    let from_ok = std::fs::canonicalize(&from_path)
                        .map(|p| p.starts_with(&root))
                        .unwrap_or_else(|_| {
                            // File doesn't exist yet — check via path normalization.
                            from_path.starts_with(&root)
                        });
                    let to_ok = to_path.starts_with(&root);
                    if !from_ok || !to_ok {
                        eprintln!("warning: '{}' escapes allowed root, skipping", r.from);
                        return false;
                    }
                    true
                })
                .collect();

            // Overwrite check: warn if target already exists (not dangerous, exit 0).
            for r in &filtered {
                let to_path = root.join(&r.to);
                if to_path.exists() {
                    eprintln!("warning: '{}' would be overwritten", r.to);
                }
            }

            output.renames = filtered;
            // Rebuild script from filtered renames.
            output.script = run::build_script(&output.renames);
            // Re-run danger check on updated script.
            if danger::check_and_warn(&output.script) {
                output.dangerous = true;
            }

            if cli.json {
                println!("{}", serde_json::to_string_pretty(&output).unwrap());
            } else {
                print!("{}", output.to_plain());
            }

            let code = if output.dangerous && !cli.allow_dangerous {
                exit::DANGEROUS
            } else {
                exit::SUCCESS
            };
            process::exit(code);
        }
        Err(e) => {
            print_error(&e, cli.json);
            process::exit(e.exit_code());
        }
    }
}
