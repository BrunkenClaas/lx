#![forbid(unsafe_code)]

use lx_config::Config;
use lx_core::error::LxError;
use lx_llm::{inject_lang, parse_response, LlmClient, Request};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const SYSTEM_TEMPLATE: &str = include_str!("../prompts/system.txt");

/// Max tokens: allow up to ~50 matches with file/line/snippet each ~60 chars.
const MAX_TOKENS: u32 = 2048;

/// Context lines to include around each candidate hit (before + after).
const CONTEXT_LINES: usize = 2;

/// Maximum number of candidate blocks to send to the LLM per call.
/// Keeps the prompt from growing unbounded on large directories.
/// This is a cost guardrail only — it never decides relevance, only volume.
const MAX_CANDIDATE_BLOCKS: usize = 40;

// ── Output types ──────────────────────────────────────────────────────────────

/// A single semantic match.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Match {
    pub file: String,
    pub line: u64,
    pub snippet: String,
}

/// Output of `lxgrep`.
#[derive(Debug, Serialize, Deserialize)]
pub struct Output {
    pub matches: Vec<Match>,
    /// True when the input exceeded the candidate-block budget and some lines
    /// were not sent to the LLM (set locally, never expected from the model).
    #[serde(default)]
    pub capped: bool,
}

impl Output {
    /// Render in grep-compatible plain text: `file:line: snippet`
    pub fn to_plain(&self) -> String {
        let mut out = String::new();
        for m in &self.matches {
            out.push_str(&format!("{}:{}: {}\n", m.file, m.line, m.snippet));
        }
        out
    }
}

// ── Candidate block ──────────────────────────────────────────────────────────

/// A block of lines grouped around a candidate hit.
struct CandidateBlock {
    /// Display path (relative to root if possible, otherwise the given path).
    file: String,
    /// 1-based line number of the first line in this block.
    start_line: u64,
    /// All lines in the block (with ±CONTEXT_LINES context).
    lines: Vec<String>,
}

impl CandidateBlock {
    /// Render the block in the format expected by the system prompt.
    fn render(&self) -> String {
        let mut s = format!("[file:{} line:{}]\n", self.file, self.start_line);
        for l in &self.lines {
            s.push_str(l);
            s.push('\n');
        }
        s
    }
}

/// Render a full set of blocks into the `QUERY: ... INPUT BLOCKS: ...` user
/// message expected by the system prompt. Exposed so `main.rs` can show the
/// exact request body in `--dry-run`.
pub fn render_user_message(query: &str, blocks: &[String]) -> String {
    let mut user_msg = format!("QUERY: {}\nINPUT BLOCKS:\n", query.trim());
    for block in blocks {
        user_msg.push_str(block);
    }
    user_msg
}

// ── Local pre-ranking (volume control only — never a relevance gate) ─────────

/// Extract keywords from the query for fast substring ranking.
///
/// Splits on whitespace and punctuation, lower-cases, drops stopwords,
/// keeps tokens >= 3 chars. Used only to *prioritise* which lines are kept
/// when input must be sampled down to fit the budget — never to decide
/// whether the LLM is called at all.
fn extract_keywords(query: &str) -> Vec<String> {
    const STOPWORDS: &[&str] = &[
        "the", "and", "for", "with", "that", "this", "are", "from", "how", "what", "where", "when",
        "does", "any", "all", "not", "use", "used", "using",
    ];
    query
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| t.len() >= 3)
        .map(|t| t.to_lowercase())
        .filter(|t| !STOPWORDS.contains(&t.as_str()))
        .collect()
}

/// Returns true if `line` contains at least one keyword (case-insensitive).
fn line_matches_keyword(line: &str, keywords: &[String]) -> bool {
    if keywords.is_empty() {
        return false;
    }
    let lower = line.to_lowercase();
    keywords.iter().any(|kw| lower.contains(kw.as_str()))
}

/// Build context blocks around a set of hit line-indices, merging overlapping
/// or adjacent windows so context doesn't fragment or duplicate.
fn blocks_from_hit_indices(
    lines: &[&str],
    display_path: &str,
    mut hit_indices: Vec<usize>,
) -> Vec<CandidateBlock> {
    if hit_indices.is_empty() {
        return vec![];
    }
    hit_indices.sort_unstable();
    hit_indices.dedup();
    let n = lines.len();

    let mut merged_ranges: Vec<(usize, usize)> = Vec::new(); // (start, end) inclusive
    {
        let first = hit_indices[0];
        let ctx_start = first.saturating_sub(CONTEXT_LINES);
        let ctx_end = (first + CONTEXT_LINES).min(n - 1);
        merged_ranges.push((ctx_start, ctx_end));
    }
    for &i in &hit_indices[1..] {
        let ctx_start = i.saturating_sub(CONTEXT_LINES);
        let ctx_end = (i + CONTEXT_LINES).min(n - 1);
        let last = merged_ranges.last_mut().unwrap();
        if ctx_start <= last.1 + 1 {
            last.1 = last.1.max(ctx_end);
        } else {
            merged_ranges.push((ctx_start, ctx_end));
        }
    }

    merged_ranges
        .into_iter()
        .map(|(start, end)| CandidateBlock {
            file: display_path.to_string(),
            start_line: (start + 1) as u64,
            lines: lines[start..=end].iter().map(|l| l.to_string()).collect(),
        })
        .collect()
}

/// Split `content` into non-overlapping windows of `window` lines each,
/// covering the whole file. Used to sample context evenly when there aren't
/// enough (or any) keyword hits to fill the budget — mirrors lxlog's
/// "fill remaining slots with evenly-sampled lines" strategy.
fn evenly_sampled_indices(line_count: usize, window: usize, max_samples: usize) -> Vec<usize> {
    if line_count == 0 || max_samples == 0 {
        return vec![];
    }
    let step = (line_count / max_samples.max(1)).max(window).max(1);
    let mut indices = Vec::new();
    let mut i = 0;
    while i < line_count && indices.len() < max_samples {
        indices.push(i);
        i += step;
    }
    indices
}

/// Produce up to `budget` candidate blocks from one file's content.
///
/// Strategy (volume control only, never a relevance decision):
/// 1. Find all keyword-hit lines and build context blocks around them first.
/// 2. If those blocks fit within `budget`, also add evenly-sampled blocks from
///    the rest of the file so the LLM still sees the whole document when it
///    fits — this is what lets semantically-relevant lines with no literal
///    keyword overlap still reach the model.
/// 3. If the file is small enough that everything fits in `budget` blocks on
///    its own merged ranges, that happens automatically via step 1+2 covering
///    every line.
///
/// Returns the blocks and whether this file's content had to be cut down.
fn candidate_blocks_for_file(
    content: &str,
    display_path: &str,
    keywords: &[String],
    budget: usize,
) -> (Vec<CandidateBlock>, bool) {
    if budget == 0 {
        return (vec![], !content.is_empty());
    }

    let lines: Vec<&str> = content.lines().collect();
    let n = lines.len();
    if n == 0 {
        return (vec![], false);
    }

    let hit_indices: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| line_matches_keyword(l, keywords))
        .map(|(i, _)| i)
        .collect();

    let mut all_indices = hit_indices.clone();

    // Fill remaining budget with evenly-sampled coverage of the whole file so
    // semantically relevant lines with no literal keyword overlap are still
    // visible to the model. Window size matches the context radius so sampled
    // windows don't degenerate into single lines.
    let approx_blocks_per_index = (2 * CONTEXT_LINES + 1).max(1);
    let target_indices = budget.saturating_mul(approx_blocks_per_index);
    if all_indices.len() < target_indices {
        let remaining = target_indices - all_indices.len();
        let sampled = evenly_sampled_indices(n, CONTEXT_LINES * 2 + 1, remaining.max(1));
        all_indices.extend(sampled);
    }

    let blocks = blocks_from_hit_indices(&lines, display_path, all_indices);
    let capped = blocks.len() > budget;
    let mut blocks = blocks;
    blocks.truncate(budget);
    (blocks, capped)
}

// ── fsbound helpers ───────────────────────────────────────────────────────────

/// Resolve `path` and verify it is within `root`.
///
/// Returns the display string (relative to root if possible) and the canonical
/// path, or an error if the path escapes the root.
fn resolve_and_check_fsbound(path: &Path, root: &Path) -> Result<(String, PathBuf), LxError> {
    let canonical = std::fs::canonicalize(path)
        .map_err(|e| LxError::BadUsage(format!("cannot resolve {}: {e}", path.display())))?;
    let root_canonical = std::fs::canonicalize(root)
        .map_err(|e| LxError::BadUsage(format!("cannot resolve root {}: {e}", root.display())))?;

    if !canonical.starts_with(&root_canonical) {
        return Err(LxError::SecurityAbort(format!(
            "path {} escapes allowed root {}",
            canonical.display(),
            root_canonical.display()
        )));
    }

    // Display path: relative to root when possible.
    let display = canonical
        .strip_prefix(&root_canonical)
        .map(|rel| rel.to_string_lossy().to_string())
        .unwrap_or_else(|_| canonical.to_string_lossy().to_string());

    Ok((display, canonical))
}

/// Returns true if the path component should be skipped (ignore rules).
fn should_skip_component(name: &str) -> bool {
    matches!(
        name,
        ".git"
            | "target"
            | "node_modules"
            | ".tox"
            | ".venv"
            | "venv"
            | "__pycache__"
            | ".mypy_cache"
            | ".pytest_cache"
            | "dist"
            | "build"
            | ".DS_Store"
    )
}

/// Collect all (display_path, content) pairs from `paths` (files or
/// directories), verifying every path stays within `root`.
///
/// Exposed (not just `run_on_files`-internal) so `main.rs` can build the
/// same file set for `--dry-run` previews without duplicating fs-walk logic.
pub fn collect_file_contents(
    paths: &[PathBuf],
    root: &Path,
    max_bytes: usize,
) -> Result<Vec<(String, String)>, LxError> {
    let mut files = Vec::new();
    for p in paths {
        collect_paths_from(p, root, max_bytes, &mut files)?;
    }
    Ok(files)
}

fn collect_paths_from(
    p: &Path,
    root: &Path,
    max_bytes: usize,
    files: &mut Vec<(String, String)>,
) -> Result<(), LxError> {
    let meta = std::fs::metadata(p)
        .map_err(|e| LxError::BadUsage(format!("cannot stat {}: {e}", p.display())))?;

    if meta.is_dir() {
        resolve_and_check_fsbound(p, root)?;
        let entries = std::fs::read_dir(p)
            .map_err(|e| LxError::BadUsage(format!("cannot read dir {}: {e}", p.display())))?;
        let mut children: Vec<PathBuf> = entries
            .filter_map(|e| e.ok())
            .filter(|e| {
                let name = e.file_name();
                let name_str = name.to_string_lossy();
                !should_skip_component(&name_str)
            })
            .map(|e| e.path())
            .collect();
        children.sort();
        for child in children {
            collect_paths_from(&child, root, max_bytes, files)?;
        }
    } else if meta.is_file() {
        let (display, _) = resolve_and_check_fsbound(p, root)?;
        let content = read_file_limited(p, max_bytes)?;
        files.push((display, content));
    }
    // Symlinks to non-file/non-dir: skip silently.
    Ok(())
}

/// Read a file up to `max_bytes`, returning lossy UTF-8.
fn read_file_limited(path: &Path, max_bytes: usize) -> Result<String, LxError> {
    use std::io::Read;
    let mut f = std::fs::File::open(path)
        .map_err(|e| LxError::BadUsage(format!("cannot open {}: {e}", path.display())))?;
    let mut buf = Vec::with_capacity(max_bytes.min(65_536));
    let mut chunk = [0u8; 8_192];
    let mut total = 0usize;
    loop {
        match f.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                let remaining = max_bytes.saturating_sub(total);
                if n >= remaining {
                    buf.extend_from_slice(&chunk[..remaining]);
                    break;
                } else {
                    buf.extend_from_slice(&chunk[..n]);
                    total += n;
                }
            }
            Err(e) => {
                return Err(LxError::BadUsage(format!(
                    "read error on {}: {e}",
                    path.display()
                )))
            }
        }
    }
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

// ── Shared block-building + completion ────────────────────────────────────────

/// Build candidate blocks across all files, allocating each file a
/// proportional share of `MAX_CANDIDATE_BLOCKS` (per-file fairness) so a
/// single large file cannot starve every other file out of the budget.
fn build_blocks(
    file_content_pairs: &[(&str, &str)],
    keywords: &[String],
) -> (Vec<CandidateBlock>, bool) {
    let n_files = file_content_pairs.len();
    if n_files == 0 {
        return (vec![], false);
    }
    let per_file_budget = (MAX_CANDIDATE_BLOCKS / n_files).max(1);

    let mut all_blocks = Vec::new();
    let mut any_capped = false;
    for (display, content) in file_content_pairs {
        let (blocks, capped) =
            candidate_blocks_for_file(content, display, keywords, per_file_budget);
        any_capped |= capped;
        all_blocks.extend(blocks);
    }

    // Cost guardrail: even with per-file fairness, clamp to the global cap.
    if all_blocks.len() > MAX_CANDIDATE_BLOCKS {
        any_capped = true;
        all_blocks.truncate(MAX_CANDIDATE_BLOCKS);
    }

    (all_blocks, any_capped)
}

/// Render blocks, call the LLM exactly once, and return the parsed `Output`
/// with `capped` filled in locally.
fn complete_with_blocks(
    query: &str,
    blocks: &[CandidateBlock],
    capped: bool,
    config: &Config,
    client: &dyn LlmClient,
) -> Result<Output, LxError> {
    if blocks.is_empty() {
        // No content at all to search (e.g. all files empty) — not a relevance
        // decision, just nothing exists to send.
        return Ok(Output {
            matches: vec![],
            capped,
        });
    }

    let rendered: Vec<String> = blocks.iter().map(|b| b.render()).collect();
    let user_msg = render_user_message(query, &rendered);
    let system = inject_lang(SYSTEM_TEMPLATE, &config.output.lang);

    let req = Request {
        system: &system,
        user: &user_msg,
        max_tokens: MAX_TOKENS,
        temperature: 0.0,
        image: None,
    };

    let resp = client.complete(&req).map_err(LxError::from)?;
    let mut out: Output = parse_response(&resp.content)?;
    out.capped = capped;
    Ok(out)
}

// ── Public run() ──────────────────────────────────────────────────────────────

/// Core logic for lxgrep.
///
/// Pure function: no I/O, no process::exit. Testable with MockLlmClient.
///
/// Local code only ever decides *how much* content to sample into the single
/// LLM call (cost control). Relevance is always the model's decision — an
/// empty result must come from the LLM, never from local keyword logic.
///
/// - `query`: the natural-language search query.
/// - `file_content_pairs`: list of `(display_name, content)` pairs already
///   read from disk (or `[("<stdin>", content)]` for stdin mode).
///   The caller is responsible for fsbound checks when reading files.
/// - `config` and `client` as usual.
pub fn run(
    query: &str,
    file_content_pairs: &[(&str, &str)],
    config: &Config,
    client: &dyn LlmClient,
) -> Result<Output, LxError> {
    if query.trim().is_empty() {
        return Err(LxError::BadUsage("no query provided".to_string()));
    }
    if file_content_pairs.is_empty() {
        return Err(LxError::BadUsage("no content to search".to_string()));
    }

    let keywords = extract_keywords(query);
    let (blocks, capped) = build_blocks(file_content_pairs, &keywords);
    complete_with_blocks(query, &blocks, capped, config, client)
}

// ── File-system entry point (called from main.rs) ─────────────────────────────

/// Walk `paths` (files or directories) under `root`, sample down to the
/// candidate-block budget with per-file fairness, then call the LLM once.
///
/// This is separated from `run()` so that `run()` remains purely testable with
/// in-memory content.
pub fn run_on_files(
    query: &str,
    paths: &[PathBuf],
    root: &Path,
    config: &Config,
    client: &dyn LlmClient,
) -> Result<Output, LxError> {
    if query.trim().is_empty() {
        return Err(LxError::BadUsage("no query provided".to_string()));
    }

    let max_bytes = config.limits.max_input_bytes;
    let files = collect_file_contents(paths, root, max_bytes)?;
    if files.is_empty() {
        return Err(LxError::BadUsage("no content to search".to_string()));
    }

    let keywords = extract_keywords(query);
    let pairs: Vec<(&str, &str)> = files
        .iter()
        .map(|(d, c)| (d.as_str(), c.as_str()))
        .collect();
    let (blocks, capped) = build_blocks(&pairs, &keywords);
    complete_with_blocks(query, &blocks, capped, config, client)
}

/// Build the rendered user message that `run()`/`run_on_files()` would send,
/// without calling the LLM. Used by `main.rs` for `--dry-run` so the user can
/// see exactly what content reaches the model.
pub fn preview_user_message(query: &str, file_content_pairs: &[(&str, &str)]) -> String {
    let keywords = extract_keywords(query);
    let (blocks, _capped) = build_blocks(file_content_pairs, &keywords);
    let rendered: Vec<String> = blocks.iter().map(|b| b.render()).collect();
    render_user_message(query, &rendered)
}

// ── Unit tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_keywords_filters_stopwords() {
        let kw = extract_keywords("how does the error handling work");
        assert!(kw.contains(&"error".to_string()));
        assert!(kw.contains(&"handling".to_string()));
        assert!(!kw.contains(&"the".to_string()));
    }

    #[test]
    fn extract_keywords_lowercases() {
        let kw = extract_keywords("Connection Timeout");
        assert!(kw.contains(&"connection".to_string()));
        assert!(kw.contains(&"timeout".to_string()));
    }

    #[test]
    fn candidate_blocks_basic() {
        let content = "fn main() {\n    println!(\"hello\");\n}\n\nfn error_handler() {\n    eprintln!(\"err\");\n}\n";
        let kw = extract_keywords("error handler");
        let (blocks, _capped) = candidate_blocks_for_file(content, "main.rs", &kw, 40);
        assert!(!blocks.is_empty());
        assert!(blocks.iter().any(|b| b.file == "main.rs"));
    }

    #[test]
    fn candidate_blocks_no_keyword_hit_still_covers_file() {
        // No literal keyword overlap, but the file is small — even sampling
        // must still surface content so the LLM can judge semantic relevance.
        let content = "fn add(a: i32, b: i32) -> i32 { a + b }\n";
        let kw = extract_keywords("database connection");
        let (blocks, _capped) = candidate_blocks_for_file(content, "math.rs", &kw, 40);
        assert!(
            !blocks.is_empty(),
            "small file with no keyword hit must still produce blocks for the LLM to judge"
        );
    }

    #[test]
    fn to_plain_format() {
        let out = Output {
            matches: vec![Match {
                file: "src/main.rs".to_string(),
                line: 42,
                snippet: "    Err(e) => eprintln!(\"error: {e}\"),".to_string(),
            }],
            capped: false,
        };
        let plain = out.to_plain();
        assert_eq!(
            plain.trim(),
            "src/main.rs:42:     Err(e) => eprintln!(\"error: {e}\"),"
        );
    }

    #[test]
    fn run_empty_query_returns_bad_usage() {
        use lx_testkit::MockLlmClient;
        let client = MockLlmClient::returning(r#"{"matches":[]}"#);
        let config = Config::default();
        let err = run("   ", &[("file.rs", "fn main() {}")], &config, &client).unwrap_err();
        assert_eq!(err.exit_code(), lx_core::exit::BAD_USAGE);
    }

    #[test]
    fn run_empty_content_returns_bad_usage() {
        use lx_testkit::MockLlmClient;
        let client = MockLlmClient::returning(r#"{"matches":[]}"#);
        let config = Config::default();
        let err = run("error handling", &[], &config, &client).unwrap_err();
        assert_eq!(err.exit_code(), lx_core::exit::BAD_USAGE);
    }

    #[test]
    fn run_calls_llm_even_when_query_has_no_literal_keyword_match() {
        // Regression test: previously, lxgrep returned an empty result WITHOUT
        // calling the LLM whenever the query's keywords had no literal
        // substring match in the content. That defeats semantic search (see
        // unattended-upgrades.log "what was updated" bug report). The LLM
        // must always be the one deciding relevance.
        use lx_testkit::MockLlmClient;
        let client = MockLlmClient::returning(
            r#"{"matches":[{"file":"math.rs","line":1,"snippet":"fn add(a: i32, b: i32) -> i32 { a + b }"}]}"#,
        );
        let config = Config::default();
        let out = run(
            "database connection pool",
            &[("math.rs", "fn add(a: i32, b: i32) -> i32 { a + b }")],
            &config,
            &client,
        )
        .unwrap();
        assert_eq!(
            client.call_count(),
            1,
            "LLM must be called regardless of literal keyword overlap"
        );
        assert_eq!(out.matches.len(), 1);
    }

    #[test]
    fn run_oversized_input_sets_capped_and_calls_llm_once() {
        use lx_testkit::MockLlmClient;
        let client = MockLlmClient::returning(r#"{"matches":[]}"#);
        let config = Config::default();
        // Space keyword hits far enough apart (well beyond 2*CONTEXT_LINES)
        // that their context windows cannot merge, forcing far more than
        // MAX_CANDIDATE_BLOCKS distinct blocks and triggering the cap.
        let mut content = String::new();
        for i in 0..500 {
            content.push_str(&format!("line {i}: filler text\n"));
            content.push_str("error something happened here\n");
            for j in 0..10 {
                content.push_str(&format!("line {i}-{j}: more filler\n"));
            }
        }
        let out = run("error", &[("big.log", &content)], &config, &client).unwrap();
        assert_eq!(client.call_count(), 1, "must still be exactly one LLM call");
        assert!(out.capped, "oversized input must set capped=true");
    }

    #[test]
    fn build_blocks_per_file_fairness() {
        // Two files, both with many keyword hits — neither should be starved
        // out entirely by the other when the combined budget is tight.
        let mut content_a = String::new();
        let mut content_b = String::new();
        for i in 0..200 {
            content_a.push_str(&format!("a-line {i}: error in module a\n"));
            content_b.push_str(&format!("b-line {i}: error in module b\n"));
        }
        let kw = extract_keywords("error");
        let pairs: Vec<(&str, &str)> = vec![("a.log", &content_a), ("b.log", &content_b)];
        let (blocks, _capped) = build_blocks(&pairs, &kw);
        assert!(blocks.iter().any(|b| b.file == "a.log"));
        assert!(blocks.iter().any(|b| b.file == "b.log"));
    }
}
