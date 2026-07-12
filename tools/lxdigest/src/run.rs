#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};

use lx_config::Config;
use lx_core::error::LxError;
use lx_llm::{inject_lang, parse_response, LlmClient, Request};
use lx_redact::RedactLevel;
use serde::{Deserialize, Serialize};

pub const SYSTEM_TEMPLATE: &str = include_str!("../prompts/system.txt");

/// Tight token budget — summary + notable file list.
const MAX_TOKENS: u32 = 1024;

/// Maximum number of files forwarded to the LLM.
const MAX_FILES: usize = 200;

/// Maximum file size to include in the listing (10 MiB).
const MAX_FILE_BYTES: u64 = 10 * 1024 * 1024;

/// Maximum bytes for the assembled listing sent to the LLM.
const MAX_LISTING_BYTES: usize = 32_000;

/// Output of `lxdigest`.
#[derive(Debug, Serialize, Deserialize)]
pub struct Output {
    pub summary: String,
    pub files: Vec<String>,
}

impl Output {
    /// Pipe-safe plain output: just the summary text.
    pub fn to_plain(&self) -> String {
        self.summary.clone()
    }
}

// ── Directory walker ──────────────────────────────────────────────────────────

struct FileEntry {
    /// Display path relative to root when possible.
    display_path: String,
    size_bytes: u64,
}

/// Walk `dir` collecting file entries, enforcing fsbound via `canonical_root`.
fn collect_entries(dir: &Path, canonical_root: &Path) -> Vec<FileEntry> {
    let mut entries = Vec::new();
    walk_dir(dir, canonical_root, &mut entries);
    entries
}

fn walk_dir(dir: &Path, canonical_root: &Path, out: &mut Vec<FileEntry>) {
    let read_dir = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(_) => return,
    };

    for entry in read_dir.flatten() {
        let path = entry.path();

        // fsbound: resolve symlinks; skip any that escape root.
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

        if should_skip_name(&name_str) {
            continue;
        }

        let file_type = match entry.file_type() {
            Ok(ft) => ft,
            Err(_) => continue,
        };

        if file_type.is_dir() {
            walk_dir(&canonical, canonical_root, out);
        } else if file_type.is_file() {
            let metadata = match std::fs::metadata(&canonical) {
                Ok(m) => m,
                Err(_) => continue,
            };
            let size = metadata.len();
            if size > MAX_FILE_BYTES {
                continue;
            }

            let display_path = canonical
                .strip_prefix(canonical_root)
                .map(|rel| rel.display().to_string())
                .unwrap_or_else(|_| canonical.display().to_string());

            out.push(FileEntry {
                display_path,
                size_bytes: size,
            });
        }
    }
}

/// Skip common vendor, VCS, and generated directories.
fn should_skip_name(name: &str) -> bool {
    matches!(
        name,
        ".git"
            | ".hg"
            | ".svn"
            | "node_modules"
            | "target"
            | ".cargo"
            | "vendor"
            | ".tox"
            | "__pycache__"
            | ".venv"
            | "venv"
            | "dist"
            | "build"
            | ".idea"
            | ".vscode"
    )
}

// ── run() ─────────────────────────────────────────────────────────────────────

/// Core logic for `lxdigest`.
///
/// `input` is the pre-redacted directory listing (assembled and redacted by
/// the caller). `root_path` is the directory to walk (fsbound enforced here).
///
/// Security flags: `fsbound` + `redact` + `untrusted`.
pub fn run(
    root_path: &Path,
    no_redact: bool,
    config: &Config,
    client: &dyn LlmClient,
) -> Result<Output, LxError> {
    // fsbound: resolve the root before walking.
    let canonical_root = std::fs::canonicalize(root_path).map_err(|e| {
        LxError::BadUsage(format!("cannot resolve path {}: {e}", root_path.display()))
    })?;

    // Walk the directory tree locally.
    let mut entries = collect_entries(&canonical_root, &canonical_root);

    // Sort for determinism and limit to MAX_FILES.
    entries.sort_by(|a, b| a.display_path.cmp(&b.display_path));
    if entries.len() > MAX_FILES {
        entries.truncate(MAX_FILES);
    }

    // Build a compact listing for the LLM.
    let listing_lines: Vec<String> = entries
        .iter()
        .map(|e| format!("  {} | {}", e.display_path, e.size_bytes))
        .collect();
    let raw_listing = listing_lines.join("\n");

    // Truncate if needed.
    let listing = if raw_listing.len() > MAX_LISTING_BYTES {
        eprintln!(
            "warning: directory listing truncated to {} bytes",
            MAX_LISTING_BYTES
        );
        &raw_listing[..MAX_LISTING_BYTES]
    } else {
        &raw_listing
    };

    // redact: apply secret masking before sending to the LLM.
    let safe_listing = if no_redact {
        listing.to_string()
    } else {
        let level = RedactLevel::parse(&config.redact.level);
        lx_redact::redact(listing, level)
            .map_err(|e| LxError::SecurityAbort(format!("redaction failed: {e}")))?
    };

    let user_message = format!(
        "Directory: {}\nDirectory listing:\n{}",
        canonical_root.display(),
        safe_listing
    );

    let system = inject_lang(SYSTEM_TEMPLATE, &config.output.lang);

    let req = Request {
        system: &system,
        user: &user_message,
        max_tokens: MAX_TOKENS,
        temperature: 0.0,
        image: None,
    };

    let resp = client.complete(&req).map_err(LxError::from)?;

    let out: Output = parse_response(&resp.content)?;

    if out.summary.is_empty() {
        return Err(LxError::LogicalError(
            "model returned empty summary".to_string(),
        ));
    }

    // fsbound: reject any file paths returned by the LLM that escape root.
    let files = out
        .files
        .into_iter()
        .filter(|p| {
            let full: PathBuf = if Path::new(p).is_absolute() {
                PathBuf::from(p)
            } else {
                canonical_root.join(p)
            };
            if let Ok(resolved) = std::fs::canonicalize(&full) {
                if resolved.starts_with(&canonical_root) {
                    true
                } else {
                    eprintln!("warning: LLM returned path that escapes root, ignoring: {p}");
                    false
                }
            } else {
                // Path does not exist — keep it (display paths may be relative).
                true
            }
        })
        .collect();

    Ok(Output {
        summary: out.summary,
        files,
    })
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_plain_returns_summary() {
        let out = Output {
            summary: "A Rust project.".to_string(),
            files: vec!["src/main.rs".to_string()],
        };
        assert_eq!(out.to_plain(), "A Rust project.");
    }

    #[test]
    fn to_plain_empty_files() {
        let out = Output {
            summary: "Empty directory.".to_string(),
            files: vec![],
        };
        assert_eq!(out.to_plain(), "Empty directory.");
    }

    #[test]
    fn should_skip_name_git() {
        assert!(should_skip_name(".git"));
        assert!(should_skip_name("node_modules"));
        assert!(should_skip_name("target"));
    }

    #[test]
    fn should_not_skip_src() {
        assert!(!should_skip_name("src"));
        assert!(!should_skip_name("tests"));
    }
}
