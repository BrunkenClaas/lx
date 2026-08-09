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

/// Budget for line-oriented input (see [`looks_line_oriented`]). Each candidate
/// is a single line with no context, so the same token spend covers far more
/// candidates than the block budget does — 400 short lines is comparable in size
/// to 40 five-line blocks.
const MAX_CANDIDATE_LINES: usize = 400;

/// A line-oriented input's lines must be no longer than this (on average) to
/// qualify. Path lists, `ls` output and ID lists sit far below it; prose and
/// source code sit above.
const LINE_ORIENTED_MAX_AVG_LEN: usize = 120;

/// Minimum number of lines before line-oriented handling kicks in. Below this
/// everything fits in the block budget anyway, so the distinction is moot.
const LINE_ORIENTED_MIN_LINES: usize = 50;

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
    /// True when the byte limit cut the input short *before* sampling — a
    /// different failure from the sampling flag above, and a different remedy
    /// (raise `--max-input-bytes` rather than narrow the query). Set locally
    /// from the reader, hence `#[serde(default)]`.
    #[serde(default)]
    pub input_truncated: bool,
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
///
/// Keywords are already lower-cased by [`extract_keywords`]. For ASCII keywords
/// the comparison folds case in place; allocating a lower-cased copy of every
/// line costs one allocation per line per file, which on a large input is the
/// single hottest cost in the sampler. Non-ASCII keywords fall back to the
/// allocating path so Unicode case folding keeps its exact semantics.
fn line_matches_keyword(line: &str, keywords: &[String]) -> bool {
    if keywords.is_empty() {
        return false;
    }
    if keywords.iter().all(|kw| kw.is_ascii()) && line.is_ascii() {
        let hay = line.as_bytes();
        return keywords.iter().any(|kw| {
            let needle = kw.as_bytes();
            !needle.is_empty()
                && hay.len() >= needle.len()
                && hay
                    .windows(needle.len())
                    .any(|w| w.eq_ignore_ascii_case(needle))
        });
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

/// Reduce `items` to at most `k` entries spread evenly across the WHOLE slice.
///
/// Unlike `truncate(k)`, this keeps the first and last entries and equalises the
/// gaps between the rest. That difference is what makes hit-downsampling a
/// *volume* decision rather than a positional one: truncating a sorted hit list
/// keeps only the lowest line numbers, so the model never sees past the head of
/// a large file — a relevance gate by another name (design_document.md §7.5).
///
/// Input order is preserved, so an ascending index list stays ascending.
fn downsample_evenly<T>(items: Vec<T>, k: usize) -> Vec<T> {
    let len = items.len();
    if k == 0 {
        return vec![];
    }
    if len <= k {
        return items;
    }
    // Positions i*(len-1)/(k-1) for i in 0..k: hits both ends, equal spacing.
    let keep: std::collections::BTreeSet<usize> = if k == 1 {
        std::iter::once(0).collect()
    } else {
        (0..k).map(|i| i * (len - 1) / (k - 1)).collect()
    };
    items
        .into_iter()
        .enumerate()
        .filter_map(|(i, v)| keep.contains(&i).then_some(v))
        .collect()
}

/// Share of a sampling budget reserved for even coverage of the whole input,
/// even when keyword hits alone could fill it.
///
/// Without this reserve, a query whose keywords match a large fraction of the
/// input lets the hit set decide what the model is allowed to consider at all —
/// which is a relevance decision made by local code, forbidden by §7.5. One
/// quarter keeps three quarters of the budget for prioritised hits while
/// guaranteeing the tail stays reachable.
const COVERAGE_SHARE_DENOM: usize = 4;

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

/// Returns true when `content` looks like a list of self-contained records
/// (a path list from `find`/`ls`, an ID list, a CSV-ish column) rather than
/// prose or source code.
///
/// This matters because the block sampler assumes *content*: it spends
/// `2 * CONTEXT_LINES + 1` lines of budget per candidate so the model can see
/// what surrounds a hit. On a record list that context is meaningless — the
/// neighbours of a path are unrelated paths — so four fifths of the budget is
/// wasted and the block cap ends up deciding which records the model may
/// consider at all. That would make it a relevance gate, which this file's
/// contract forbids.
///
/// Deliberately conservative: it must not fire on real source or prose, where
/// context genuinely carries meaning. Requires *all* of:
/// - enough lines that the block budget would actually bind,
/// - short lines on average (records, not sentences or statements),
/// - no blank lines (paragraphs and code have them; generated lists do not),
/// - no leading indentation (structure implies context matters).
fn looks_line_oriented(content: &str) -> bool {
    let lines: Vec<&str> = content.lines().collect();
    if lines.len() < LINE_ORIENTED_MIN_LINES {
        return false;
    }

    let mut total_len = 0usize;
    for l in &lines {
        // A blank line separates paragraphs/blocks — a sign context matters.
        if l.trim().is_empty() {
            return false;
        }
        // Leading whitespace implies nesting/structure (code, YAML, prose).
        if l.starts_with(' ') || l.starts_with('\t') {
            return false;
        }
        total_len += l.chars().count();
    }

    total_len / lines.len() <= LINE_ORIENTED_MAX_AVG_LEN
}

/// Produce up to `budget` single-line candidates from line-oriented content.
///
/// Keyword hits come first (volume control, exactly as in the block path), then
/// evenly-spaced lines fill any remaining budget so records with no literal
/// keyword overlap still reach the model. Every candidate is one line with no
/// context, which is what makes the far larger budget affordable.
fn candidate_lines_for_file(
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

    let hits: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| line_matches_keyword(l, keywords))
        .map(|(i, _)| i)
        .collect();

    // Reserve part of the budget for even coverage of the whole list, so a
    // relevant record that shares no keyword with the query is still visible
    // even when hits alone would fill the budget (§7.5).
    let coverage_budget = (budget / COVERAGE_SHARE_DENOM).max(1).min(budget);
    let hit_budget = budget - coverage_budget;

    // Hits are prioritised, but when there are more of them than their share
    // allows they are thinned ACROSS THE WHOLE FILE — never cut to a prefix.
    let mut indices = downsample_evenly(hits, hit_budget);
    indices.extend(evenly_sampled_indices(n, 1, budget - indices.len()));
    indices.sort_unstable();
    indices.dedup();

    // Deduplication can free slots when a sampled index coincided with a hit.
    // Reclaim them with a denser pass, but thin the result back down *evenly*:
    // a denser sample is front-loaded relative to the budget, so sorting and
    // truncating it would pull every candidate towards the head — the exact
    // bias this function exists to avoid.
    if indices.len() < budget && indices.len() < n {
        indices.extend(evenly_sampled_indices(n, 1, (budget - indices.len()) * 2));
        indices.sort_unstable();
        indices.dedup();
    }

    indices = downsample_evenly(indices, budget);
    // Honest meaning: some line of the input never reached the model.
    let capped = n > indices.len();

    let blocks = indices
        .into_iter()
        .map(|i| CandidateBlock {
            file: display_path.to_string(),
            start_line: (i + 1) as u64,
            lines: vec![lines[i].to_string()],
        })
        .collect();

    (blocks, capped)
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

    // Reserve part of the budget for coverage of the whole file, so relevant
    // lines with no literal keyword overlap stay reachable even when hits alone
    // would fill it (§7.5).
    let coverage_blocks = (budget / COVERAGE_SHARE_DENOM).max(1).min(budget);
    let hit_blocks = budget - coverage_blocks;

    // Build the hit blocks first, then thin them ACROSS THE WHOLE FILE rather
    // than truncating to a prefix. Downsampling happens in *block* units, not
    // hit-index units, because context merging means k hits rarely yield k
    // blocks — thinning the indices would not bound the block count.
    let hit_only = blocks_from_hit_indices(&lines, display_path, hit_indices);
    let mut blocks = downsample_evenly(hit_only, hit_blocks);

    // Fill the rest with evenly-sampled coverage of the whole file. Window size
    // matches the context radius so sampled windows don't degenerate into
    // single lines.
    let target_indices = budget.saturating_mul((2 * CONTEXT_LINES + 1).max(1));
    let sampled = evenly_sampled_indices(n, CONTEXT_LINES * 2 + 1, target_indices.max(1));
    let coverage = blocks_from_hit_indices(&lines, display_path, sampled);
    let room = budget.saturating_sub(blocks.len());
    blocks.extend(downsample_evenly(coverage, room));

    blocks.sort_by_key(|b| b.start_line);
    blocks.dedup_by_key(|b| b.start_line);
    blocks.truncate(budget);

    // Honest meaning: some line of the file never reached the model. A block
    // covers 2*CONTEXT_LINES+1 lines, so compare against the lines covered.
    let covered: usize = blocks.iter().map(|b| b.lines.len()).sum();
    let capped = n > covered;
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
    // `max_bytes` is per file; a directory walk multiplies it by the file count,
    // so a second, aggregate ceiling is what actually bounds memory here.
    let mut remaining_total = lx_core::io::DEFAULT_MAX_TOTAL_INPUT_BYTES;
    for p in paths {
        collect_paths_from(p, root, max_bytes, &mut remaining_total, &mut files)?;
    }
    Ok(files)
}

fn collect_paths_from(
    p: &Path,
    root: &Path,
    max_bytes: usize,
    remaining_total: &mut usize,
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
            collect_paths_from(&child, root, max_bytes, remaining_total, files)?;
        }
    } else if meta.is_file() {
        // Refuse rather than silently stopping: the walk is in sorted order, so
        // quietly running out of budget would search the alphabetically-first
        // files and skip the rest — a positional relevance gate (§7.5). An
        // explicit error tells the user to narrow the path instead.
        if *remaining_total == 0 {
            return Err(LxError::BadUsage(format!(
                "input too large: this directory exceeds the {} MiB aggregate                  input ceiling; narrow the path or exclude build/vendor directories",
                lx_core::io::DEFAULT_MAX_TOTAL_INPUT_BYTES / (1024 * 1024)
            )));
        }
        let (display, _) = resolve_and_check_fsbound(p, root)?;
        let content = lx_core::io::read_file_limited(p, max_bytes.min(*remaining_total))?;
        *remaining_total = remaining_total.saturating_sub(content.len());
        files.push((display, content));
    }
    // Symlinks to non-file/non-dir: skip silently.
    Ok(())
}

// ── Shared block-building + completion ────────────────────────────────────────

/// Build candidate blocks across all files, allocating each file a
/// proportional share of the budget (per-file fairness) so a single large file
/// cannot starve every other file out of the budget.
///
/// The budget depends on the input shape. Record lists (see
/// [`looks_line_oriented`]) are sampled one line at a time against
/// `MAX_CANDIDATE_LINES`; everything else keeps the context-block strategy and
/// `MAX_CANDIDATE_BLOCKS`. The decision is per file, so a mixed set (a path list
/// plus a source file) treats each appropriately. This is still volume control
/// only — neither path decides relevance.
fn build_blocks(
    file_content_pairs: &[(&str, &str)],
    keywords: &[String],
) -> (Vec<CandidateBlock>, bool) {
    let n_files = file_content_pairs.len();
    if n_files == 0 {
        return (vec![], false);
    }
    let per_file_blocks = (MAX_CANDIDATE_BLOCKS / n_files).max(1);
    let per_file_lines = (MAX_CANDIDATE_LINES / n_files).max(1);

    let mut all_blocks = Vec::new();
    let mut any_capped = false;
    let mut any_line_oriented = false;
    for (display, content) in file_content_pairs {
        let (blocks, capped) = if looks_line_oriented(content) {
            any_line_oriented = true;
            candidate_lines_for_file(content, display, keywords, per_file_lines)
        } else {
            candidate_blocks_for_file(content, display, keywords, per_file_blocks)
        };
        any_capped |= capped;
        all_blocks.extend(blocks);
    }

    // Cost guardrail: even with per-file fairness, clamp to the global cap. A
    // line-oriented candidate is a single line, so it is bounded by the larger
    // line cap; mixed inputs use the larger of the two, since the block path is
    // already bounded by its own per-file share.
    let global_cap = if any_line_oriented {
        MAX_CANDIDATE_LINES
    } else {
        MAX_CANDIDATE_BLOCKS
    };
    if all_blocks.len() > global_cap {
        any_capped = true;
        // Thin evenly, never truncate: blocks arrive in file order and within a
        // file in line order, so cutting to a prefix would drop the tail of the
        // last file *and* whole files at the end of the list — the same
        // positional gate the per-file samplers avoid (§7.5).
        all_blocks = downsample_evenly(all_blocks, global_cap);
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
            // Input truncation is an I/O fact main.rs knows and run() does not.
            input_truncated: false,
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
            input_truncated: false,
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

    // ── §7.5: sampling must span the WHOLE input, never just its head ────────

    /// Builds a record list of `n` lines where every `hit_every`-th line
    /// contains the keyword. Returns the content.
    fn record_list(n: usize, hit_every: usize) -> String {
        let mut s = String::new();
        for i in 0..n {
            if i % hit_every == 0 {
                s.push_str(&format!(
                    "error at record {i}
"
                ));
            } else {
                s.push_str(&format!(
                    "record {i}
"
                ));
            }
        }
        s
    }

    #[test]
    fn line_sampler_reaches_the_tail_when_hits_exceed_the_budget() {
        // 20k hits against a 400 budget. Sorting the hits and truncating keeps
        // only the lowest line numbers, so the model would see the first ~2% of
        // the file and nothing after it — a positional relevance gate, which
        // §7.5 forbids.
        let content = record_list(200_000, 10);
        let kw = extract_keywords("error");
        let (blocks, capped) = candidate_lines_for_file(&content, "big.txt", &kw, 400);
        assert!(capped, "a 200k-line file against a 400 budget is capped");
        let max_line = blocks.iter().map(|b| b.start_line).max().unwrap();
        let min_line = blocks.iter().map(|b| b.start_line).min().unwrap();
        assert!(
            max_line > 190_000,
            "sampler must reach the tail; deepest line was {max_line}"
        );
        assert!(
            min_line < 1_000,
            "sampler must also cover the head; shallowest line was {min_line}"
        );
    }

    #[test]
    fn line_sampler_keeps_coverage_when_every_line_is_a_hit() {
        // Every line matches, so hits alone could fill the budget from the head.
        // The reserved coverage share must still spread candidates to the end.
        let content = record_list(50_000, 1);
        let kw = extract_keywords("error");
        let (blocks, _capped) = candidate_lines_for_file(&content, "big.txt", &kw, 400);
        let max_line = blocks.iter().map(|b| b.start_line).max().unwrap();
        assert!(
            max_line > 45_000,
            "even a 100% hit rate must not collapse to the head; deepest was {max_line}"
        );
    }

    #[test]
    fn block_sampler_reaches_the_tail_when_hits_exceed_the_budget() {
        // Same defect in the block path: blocks come back in ascending order and
        // `truncate(budget)` keeps the first ones.
        let mut content = String::new();
        for i in 0..20_000 {
            if i % 10 == 0 {
                content.push_str(&format!(
                    "    let x = error_handler({i});
"
                ));
            } else {
                content.push_str(&format!(
                    "    let y = compute({i});
"
                ));
            }
        }
        let kw = extract_keywords("error_handler");
        let (blocks, _capped) = candidate_blocks_for_file(&content, "big.rs", &kw, 40);
        let max_line = blocks.iter().map(|b| b.start_line).max().unwrap();
        assert!(
            max_line > 15_000,
            "block sampler must reach the tail; deepest was {max_line}"
        );
    }

    #[test]
    fn line_sampler_reaches_the_tail_when_hits_are_sparse() {
        // The real-world shape that a 10%-hit-rate test does not cover: a very
        // large list with only a handful of hits, so almost the whole budget is
        // filled by the coverage sampler. A denser top-up pass followed by a
        // plain truncate would pull every candidate back towards the head.
        let mut content = String::new();
        for i in 0..20_000 {
            if i % 1_500 == 0 {
                content.push_str(&format!(
                    "./src/build_{i}.rs
"
                ));
            } else {
                content.push_str(&format!(
                    "./src/module_{i}.rs
"
                ));
            }
        }
        let kw = extract_keywords("build related stuff");
        let (blocks, _) = candidate_lines_for_file(&content, "paths.txt", &kw, 400);
        let max_line = blocks.iter().map(|b| b.start_line).max().unwrap();
        assert!(
            max_line > 19_000,
            "sparse hits must still reach the tail; deepest was {max_line}"
        );
    }
}
