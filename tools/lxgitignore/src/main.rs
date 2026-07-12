#![forbid(unsafe_code)]

use clap::Parser;
use lx_config::Config;
use lx_core::{error::print_error, exit};
use std::path::PathBuf;
use std::process;

mod run;

/// Build the canonical version string.
/// Format: "lxgitignore 1.0.0 (lx-coreutils 2026-07, <target>)"
fn version_string() -> String {
    lx_core::version::build_version_string("lxgitignore", env!("CARGO_PKG_VERSION"))
}

#[derive(Parser)]
#[command(
    name    = "lxgitignore",
    about   = "Generate a .gitignore file from a project's directory structure",
    // Disable clap's built-in --version; we handle it manually for canonical format.
    disable_version_flag = true
)]
struct Cli {
    /// Project description (e.g. "rust project with vscode"); overrides directory scanning
    description: Option<String>,

    /// Directory to scan for project structure (default: current directory)
    #[arg(long, value_name = "PATH")]
    path: Option<PathBuf>,

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

    /// Maximum bytes to read from stdin (when using --file or stdin input)
    #[arg(long)]
    max_input_bytes: Option<usize>,

    /// Read project structure listing from file instead of scanning a directory
    #[arg(long, value_name = "PATH")]
    file: Option<PathBuf>,

    /// Disable network access (not applicable, included for flag consistency)
    #[arg(long)]
    no_net: bool,

    /// Print version information
    #[arg(short = 'V', long = "version")]
    version: bool,
}

/// Walk a directory up to `max_depth` levels deep and collect file names,
/// extensions, and directory names as a plain-text listing.
///
/// Security: resolves symlinks; skips any that escape `canonical_root`.
/// Never reads file contents — only names and structure.
fn collect_project_structure(root: &std::path::Path, canonical_root: &std::path::Path) -> String {
    let mut lines: Vec<String> = Vec::new();
    walk_for_structure(root, canonical_root, 0, 3, &mut lines);
    lines.join("\n")
}

fn walk_for_structure(
    dir: &std::path::Path,
    canonical_root: &std::path::Path,
    depth: usize,
    max_depth: usize,
    out: &mut Vec<String>,
) {
    if depth > max_depth {
        return;
    }

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    // Collect and sort entries for deterministic output.
    let mut entry_list: Vec<_> = entries.flatten().collect();
    entry_list.sort_by_key(|e| e.file_name());

    for entry in entry_list {
        let path = entry.path();

        // fsbound: resolve symlinks; reject escapes.
        let canonical = match std::fs::canonicalize(&path) {
            Ok(c) => c,
            Err(_) => continue, // dangling symlink — skip
        };
        if !canonical.starts_with(canonical_root) {
            eprintln!(
                "warning: skipping {} (symlink escapes allowed root)",
                path.display()
            );
            continue;
        }

        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        // Skip hidden directories except .github (useful for gitignore).
        let file_type = match entry.file_type() {
            Ok(ft) => ft,
            Err(_) => continue,
        };

        let indent = "  ".repeat(depth);

        if file_type.is_dir() {
            out.push(format!("{}{}/", indent, name_str));
            // Don't recurse into .git — it's internal and always ignored.
            if name_str != ".git" {
                walk_for_structure(&canonical, canonical_root, depth + 1, max_depth, out);
            }
        } else if file_type.is_file() {
            out.push(format!("{}{}", indent, name_str));
        }
        // Symlinks after canonicalize already handled above.
    }
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

    // Collect input.
    // Edit mode: positional description given + stdin/--file has content → edit existing .gitignore.
    // Create mode: everything else (scan dir, use description, or read structure from stdin/--file).
    let max_bytes = cli.max_input_bytes.unwrap_or(config.limits.max_input_bytes);

    // Peek at stdin (non-TTY) early so we can decide mode.
    let stdin_content: Option<String> = if cli.path.is_none()
        && cli.file.is_none()
        && !lx_core::platform::is_tty(lx_core::platform::Fd::Stdin)
    {
        Some(lx_core::io::read_stdin(max_bytes).unwrap_or_default())
    } else {
        None
    };

    // Determine (input_for_llm, existing_for_edit_mode).
    let (input, existing): (String, Option<String>) = if let Some(ref desc) = cli.description {
        // Positional description given.
        if let Some(ref file_path) = cli.file {
            // --file = existing .gitignore to edit.
            let parent = file_path.parent().unwrap_or(std::path::Path::new("."));
            let content = lx_core::io::read_file(file_path, max_bytes, Some(parent))
                .unwrap_or_else(|e| {
                    print_error(&e, cli.json);
                    process::exit(e.exit_code());
                });
            (desc.clone(), Some(content))
        } else if let Some(ref s) = stdin_content {
            if s.trim().is_empty() {
                // Description + empty stdin → create mode
                (desc.clone(), None)
            } else {
                // Description + stdin content → edit mode
                (desc.clone(), Some(s.clone()))
            }
        } else {
            (desc.clone(), None)
        }
    } else if let Some(ref dir_path) = cli.path {
        // fsbound: resolve and validate the directory.
        let canonical = match std::fs::canonicalize(dir_path) {
            Ok(c) => c,
            Err(e) => {
                let err = lx_core::error::LxError::BadUsage(format!(
                    "cannot resolve path {}: {e}",
                    dir_path.display()
                ));
                print_error(&err, cli.json);
                process::exit(exit::BAD_USAGE);
            }
        };
        if !canonical.is_dir() {
            let err = lx_core::error::LxError::BadUsage(format!(
                "{} is not a directory",
                dir_path.display()
            ));
            print_error(&err, cli.json);
            process::exit(exit::BAD_USAGE);
        }

        if cli.verbose {
            eprintln!("[verbose] scanning directory: {}", canonical.display());
        }

        (collect_project_structure(&canonical, &canonical), None)
    } else if let Some(ref file_path) = cli.file {
        // Read structure listing from a file (create mode — no positional description).
        let parent = file_path.parent().unwrap_or(std::path::Path::new("."));
        let content =
            lx_core::io::read_file(file_path, max_bytes, Some(parent)).unwrap_or_else(|e| {
                print_error(&e, cli.json);
                process::exit(e.exit_code());
            });
        (content, None)
    } else {
        // No --path or --file: try stdin; fall back to scanning current dir.
        let s = stdin_content.unwrap_or_default();

        if s.trim().is_empty() {
            // Scan current directory.
            let cwd = match std::env::current_dir() {
                Ok(d) => d,
                Err(e) => {
                    let err = lx_core::error::LxError::BadUsage(format!(
                        "cannot determine current directory: {e}"
                    ));
                    print_error(&err, cli.json);
                    process::exit(exit::BAD_USAGE);
                }
            };
            let canonical = match std::fs::canonicalize(&cwd) {
                Ok(c) => c,
                Err(e) => {
                    let err = lx_core::error::LxError::BadUsage(format!(
                        "cannot resolve current directory: {e}"
                    ));
                    print_error(&err, cli.json);
                    process::exit(exit::BAD_USAGE);
                }
            };
            if cli.verbose {
                eprintln!(
                    "[verbose] scanning current directory: {}",
                    canonical.display()
                );
            }
            (collect_project_structure(&canonical, &canonical), None)
        } else {
            (s, None)
        }
    };

    if input.trim().is_empty() {
        let e =
            lx_core::error::LxError::BadUsage("no project structure found to analyse".to_string());
        print_error(&e, cli.json);
        process::exit(exit::BAD_USAGE);
    }

    // --dry-run: show what would be sent, then exit.
    if cli.dry_run {
        if !cli.quiet {
            let label = if existing.is_some() {
                "edit mode — intent"
            } else {
                "project structure"
            };
            eprintln!("[dry-run] {} ({} bytes):", label, input.len());
            eprintln!("{}", input.trim());
            if let Some(ref e) = existing {
                eprintln!("[dry-run] existing .gitignore ({} bytes):", e.len());
                eprintln!("{}", e.trim());
            }
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

    match run::run(&input, existing.as_deref(), &config, client.as_ref()) {
        Ok(output) => {
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&output).unwrap());
            } else {
                // stdout: the .gitignore content only — pipe safe.
                print!("{}", output.to_plain());
                if lx_core::output::show_narration(cli.quiet, cli.verbose) {
                    if existing.is_some() {
                        eprintln!("# .gitignore updated");
                    } else {
                        eprintln!("# .gitignore generated from project structure");
                    }
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
