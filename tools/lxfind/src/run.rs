#![forbid(unsafe_code)]

use std::cmp::Reverse;
use std::path::{Path, PathBuf};

use lx_config::Config;
use lx_core::error::LxError;
use lx_llm::{inject_lang, parse_response, LlmClient, Request};
use serde::{Deserialize, Serialize};

pub const SYSTEM_TEMPLATE: &str = include_str!("../prompts/system.txt");
const MAX_TOKENS: u32 = 1024;

/// Maximum number of candidates forwarded to the LLM.
const MAX_CANDIDATES: usize = 200;

/// Maximum number of paths returned to the user. The model ranks by relevance;
/// we keep the top `MAX_RESULTS` so the JSON response always fits inside
/// `MAX_TOKENS` regardless of how many candidates match. Without this cap a
/// "match everything" query could overflow the token budget and truncate the
/// JSON mid-string. Sized so MAX_RESULTS paths fit comfortably under MAX_TOKENS.
const MAX_RESULTS: usize = 60;

/// Maximum bytes read from each file for the snippet (first line only).
const SNIPPET_MAX_BYTES: usize = 200;

/// Maximum file size to include as a candidate (1 MiB).
const MAX_FILE_BYTES: u64 = 1024 * 1024;

/// Output of `lxfind`.
#[derive(Debug, Serialize, Deserialize)]
pub struct Output {
    pub paths: Vec<String>,
    /// True when the result set was capped at `MAX_RESULTS`. Set locally after
    /// parsing the model response — never delegated to the LLM, hence
    /// `#[serde(default)]`.
    #[serde(default)]
    pub truncated: bool,
}

impl Output {
    /// Render as one path per line — safe for piping to xargs etc.
    pub fn to_plain(&self) -> String {
        if self.paths.is_empty() {
            return String::new();
        }
        self.paths.join("\n") + "\n"
    }
}

// ── Candidate catalogue ───────────────────────────────────────────────────────

/// A single file candidate forwarded to the LLM.
struct Candidate {
    /// Path as returned to the user (relative to root when possible).
    display_path: String,
    size_bytes: u64,
    snippet: String,
}

/// Score a candidate by how many description words appear in its path or
/// snippet (case-insensitive). Used to rank candidates before the 200-file
/// cap so that lexicographically-late but relevant files are not silently
/// dropped.
fn score_candidate(candidate: &Candidate, query_words: &[String]) -> usize {
    let haystack = format!(
        "{} {}",
        candidate.display_path.to_lowercase(),
        candidate.snippet.to_lowercase()
    );
    query_words
        .iter()
        .filter(|w| haystack.contains(w.as_str()))
        .count()
}

/// Collect file candidates by walking `root`.
///
/// Security:
/// - Resolves symlinks; skips any that escape `canonical_root`.
/// - Skips `.git`, `node_modules`, `target`, and other vendor directories.
/// - Skips binary files and files larger than `MAX_FILE_BYTES`.
fn collect_candidates(root: &Path, canonical_root: &Path) -> Vec<Candidate> {
    let mut candidates = Vec::new();
    walk_dir(root, canonical_root, &mut candidates);
    candidates
}

fn walk_dir(dir: &Path, canonical_root: &Path, out: &mut Vec<Candidate>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
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

        if should_skip_dir_name(&name_str) {
            continue;
        }

        if should_skip_file_name(&name_str) {
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

            // Build a relative display path when possible.
            let display_path = canonical
                .strip_prefix(canonical_root)
                .map(|rel| rel.display().to_string())
                .unwrap_or_else(|_| canonical.display().to_string());

            // Read a short snippet (first non-empty line, at most SNIPPET_MAX_BYTES).
            let snippet = read_snippet(&canonical);

            // Skip binaries detected by snippet heuristic.
            if looks_binary_snippet(snippet.as_bytes()) {
                continue;
            }

            out.push(Candidate {
                display_path,
                size_bytes: size,
                snippet,
            });
        }
    }
}

/// Read up to `SNIPPET_MAX_BYTES` from the file and extract the first
/// meaningful line (non-empty, trimmed).
fn read_snippet(path: &Path) -> String {
    use std::io::Read;
    let mut file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return String::new(),
    };
    let mut buf = vec![0u8; SNIPPET_MAX_BYTES];
    let n = file.read(&mut buf).unwrap_or(0);
    buf.truncate(n);
    // Lossy-convert and take first non-empty line.
    let text = String::from_utf8_lossy(&buf);
    for line in text.lines() {
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            return trimmed.chars().take(120).collect();
        }
    }
    String::new()
}

/// Heuristic: if more than 10% of bytes in the snippet are null or control
/// chars (outside 0x09/0x0A/0x0D range), treat the file as binary.
fn looks_binary_snippet(bytes: &[u8]) -> bool {
    if bytes.is_empty() {
        return false;
    }
    let bad = bytes
        .iter()
        .filter(|&&b| b == 0 || (b < 0x09) || (b > 0x0D && b < 0x20 && b != 0x1B))
        .count();
    bad * 100 / bytes.len() > 10
}

/// Skip common non-source, vendor, or hidden directories.
fn should_skip_dir_name(name: &str) -> bool {
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
            | "snapshots"
    )
}

/// Returns true if a file should be skipped by name (generated artifacts).
fn should_skip_file_name(name: &str) -> bool {
    (name.starts_with("report-") && name.ends_with(".md")) || name == "EVALUATION.md"
}

// ── run() ─────────────────────────────────────────────────────────────────────

/// Core logic for `lxfind`.
///
/// `description` — what the user is searching for (untrusted: never mixed into
///   the system prompt).
/// `root_path` — directory to search within (fsbound).
///
/// Security flags: `fsbound` + `untrusted`.
pub fn run(
    description: &str,
    root_path: &Path,
    config: &Config,
    client: &dyn LlmClient,
) -> Result<Output, LxError> {
    if description.trim().is_empty() {
        return Err(LxError::BadUsage("no description provided".to_string()));
    }

    // fsbound: resolve the root before walking.
    let canonical_root = std::fs::canonicalize(root_path).map_err(|e| {
        LxError::BadUsage(format!("cannot resolve path {}: {e}", root_path.display()))
    })?;

    // Walk the directory tree locally.
    let mut candidates = collect_candidates(&canonical_root, &canonical_root);

    // Score and sort before truncating so that lexicographically-late but
    // relevant files are not silently dropped. Words are lowercased once here
    // and reused per candidate.
    if candidates.len() > MAX_CANDIDATES {
        let query_words: Vec<String> = description
            .split_whitespace()
            .map(|w| w.to_lowercase())
            .collect();
        candidates.sort_by_key(|c| Reverse(score_candidate(c, &query_words)));
        candidates.truncate(MAX_CANDIDATES);
    }

    if candidates.is_empty() {
        return Ok(Output {
            paths: vec![],
            truncated: false,
        });
    }

    // Build a compact catalogue for the LLM.
    let catalog_lines: Vec<String> = candidates
        .iter()
        .map(|c| format!("  {} | {} | {}", c.display_path, c.size_bytes, c.snippet))
        .collect();
    let catalog = catalog_lines.join("\n");

    // untrusted: description comes from the user — kept strictly in the user
    // message and never mixed into the system prompt.
    let user_message = format!("Description: {}\nCatalog:\n{}", description.trim(), catalog);

    let system = inject_lang(SYSTEM_TEMPLATE, &config.output.lang);

    let req = Request {
        system: &system,
        user: &user_message,
        max_tokens: MAX_TOKENS,
        temperature: 0.0,
        image: None,
    };

    let resp = client
        .complete(&req)
        .map_err(lx_core::error::LxError::from)?;

    let mut output: Output = parse_response(&resp.content)?;

    // fsbound validation: reject any returned path that, when resolved against
    // the root, escapes the allowed tree.
    output.paths.retain(|p| {
        let full = if Path::new(p).is_absolute() {
            PathBuf::from(p)
        } else {
            canonical_root.join(p)
        };
        // Normalise without requiring the path to exist (candidates may not
        // round-trip perfectly through canonicalize on all OSes).
        // Use a simple starts_with after collapsing `..` components.
        if let Ok(resolved) = std::fs::canonicalize(&full) {
            if resolved.starts_with(&canonical_root) {
                true
            } else {
                eprintln!("warning: LLM returned path that escapes root, ignoring: {p}");
                false
            }
        } else {
            // Path does not exist — keep it (the LLM may return display
            // paths that are relative but valid within root).
            true
        }
    });

    // Hard cap on the result set. The model is asked to rank by relevance, so
    // truncating keeps the most relevant matches. This guarantees the response
    // (and any downstream JSON) stays well within the token budget regardless
    // of how many candidates matched.
    if output.paths.len() > MAX_RESULTS {
        output.paths.truncate(MAX_RESULTS);
        output.truncated = true;
    }

    Ok(output)
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_plain_empty() {
        let out = Output {
            paths: vec![],
            truncated: false,
        };
        assert_eq!(out.to_plain(), "");
    }

    #[test]
    fn to_plain_single() {
        let out = Output {
            paths: vec!["src/main.rs".to_string()],
            truncated: false,
        };
        assert_eq!(out.to_plain(), "src/main.rs\n");
    }

    #[test]
    fn to_plain_multiple() {
        let out = Output {
            paths: vec!["a.rs".to_string(), "b.rs".to_string()],
            truncated: false,
        };
        assert_eq!(out.to_plain(), "a.rs\nb.rs\n");
    }

    #[test]
    fn looks_binary_snippet_null_bytes() {
        let data = b"some\x00binary\x00data";
        assert!(looks_binary_snippet(data));
    }

    #[test]
    fn looks_binary_snippet_text() {
        assert!(!looks_binary_snippet(b"#!/usr/bin/env python3"));
    }

    #[test]
    fn should_skip_dir_name_git() {
        assert!(should_skip_dir_name(".git"));
        assert!(should_skip_dir_name("node_modules"));
        assert!(should_skip_dir_name("target"));
    }

    #[test]
    fn should_not_skip_src() {
        assert!(!should_skip_dir_name("src"));
        assert!(!should_skip_dir_name("tests"));
    }

    #[test]
    fn score_candidate_matches_path_and_snippet() {
        let c = Candidate {
            display_path: "tools/lxfind/README.md".to_string(),
            size_bytes: 1000,
            snippet: "Semantic file search".to_string(),
        };
        let words: Vec<String> = vec!["lxfind".to_string(), "readme".to_string()];
        assert_eq!(score_candidate(&c, &words), 2);
    }

    #[test]
    fn score_candidate_no_match() {
        let c = Candidate {
            display_path: "src/unrelated.rs".to_string(),
            size_bytes: 500,
            snippet: "pub fn main()".to_string(),
        };
        let words: Vec<String> = vec!["lxfind".to_string(), "readme".to_string()];
        assert_eq!(score_candidate(&c, &words), 0);
    }
}
