//! Few-shot contamination guard.
//!
//! An acceptance intent that merely paraphrases a few-shot `Input:` line from
//! the tool's own `system.txt` does not test whether the *prompt generalises* —
//! it only tests whether the model can echo an example it was just shown. A
//! suite that is full of such intents reports a high pass rate while telling us
//! nothing about real-world inputs.
//!
//! The [`guard`] function — exercised by a unit test over the real
//! `intents.toml` — fails the build when any intent is contaminated, unless it is
//! explicitly marked `allow_fewshot_overlap = true`. It applies two checks:
//!
//! 1. **Short inputs** — the positional `arg` and content-carrying flag values
//!    like `--for` (see [`content_inputs`]) are compared to the prompt's
//!    single-line example inputs by word-set (Jaccard) similarity ≥ [`THRESHOLD`].
//! 2. **Fixtures** — a `stdin` fixture is compared to the prompt's *multi-line*
//!    example artifacts (see [`example_blocks`]) by a line-level near-duplicate
//!    test: a contiguous run of [`FIXTURE_RUN_THRESHOLD`]+ distinctive lines
//!    shared with an example signals copy-paste. Word-set similarity is useless
//!    here because a fixture and an example of the same kind (two diffs, two TLS
//!    errors) inevitably share domain vocabulary without either copying the other.

use std::path::PathBuf;

use crate::intents::Intent;

/// Similarity at or above this value is treated as contamination.
pub const THRESHOLD: f64 = 0.70;

/// Workspace root, derived from `CARGO_MANIFEST_DIR` (= `<root>/crates/lx-acceptance`).
fn workspace_root() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop(); // crates
    p.pop(); // root
    p
}

/// Reads `tools/<tool>/prompts/system.txt`. Returns `None` if absent (e.g. the
/// `lx` umbrella command has no prompt) — such intents are simply not checkable.
fn read_system_prompt(tool: &str) -> Option<String> {
    let mut p = workspace_root();
    p.push("tools");
    p.push(tool);
    p.push("prompts");
    p.push("system.txt");
    std::fs::read_to_string(p).ok()
}

/// Extracts the few-shot example inputs from a system prompt. Recognises lines
/// of the form `Input: ...` or `Input (flavor): ...`, which is the convention
/// every tool's prompt skeleton uses for its examples.
pub fn example_inputs(prompt: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in prompt.lines() {
        let t = line.trim_start();
        let Some(rest) = t.strip_prefix("Input") else {
            continue;
        };
        // Accept "Input:" and "Input (flavor):"; reject "Inputs", "Input file".
        let rest = rest.trim_start();
        let after_paren = if let Some(close) = rest.strip_prefix('(') {
            match close.split_once(')') {
                Some((_, r)) => r.trim_start(),
                None => continue,
            }
        } else {
            rest
        };
        if let Some(body) = after_paren.strip_prefix(':') {
            let body = body.trim();
            if !body.is_empty() {
                out.push(body.to_string());
            }
        }
    }
    out
}

/// Extracts the full multi-line example *artifacts* from a system prompt: the
/// text between an `Input:` marker and the following `Output:` / `Input:` /
/// `Example` boundary. Unlike [`example_inputs`] (single-line), this captures
/// artifact-style examples — diffs, git logs, cert dumps — that span many lines.
/// These are what stdin fixtures must not copy.
pub fn example_blocks(prompt: &str) -> Vec<String> {
    let lines: Vec<&str> = prompt.lines().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let t = lines[i].trim_start();
        // Reuse the same Input: / Input (flavor): recognition as example_inputs.
        let is_input = t.strip_prefix("Input").map(|r| {
            let r = r.trim_start();
            let r = if let Some(close) = r.strip_prefix('(') {
                close
                    .split_once(')')
                    .map(|(_, rest)| rest.trim_start())
                    .unwrap_or(r)
            } else {
                r
            };
            r.starts_with(':')
        }) == Some(true);
        if !is_input {
            i += 1;
            continue;
        }
        // First line: text after the colon (may be empty for block-style examples).
        let mut buf = Vec::new();
        if let Some((_, after)) = t.split_once(':') {
            if !after.trim().is_empty() {
                buf.push(after.trim().to_string());
            }
        }
        i += 1;
        while i < lines.len() {
            let l = lines[i].trim_start();
            if l.starts_with("Output") || l.starts_with("Input") || l.starts_with("Example") {
                break;
            }
            buf.push(lines[i].to_string());
            i += 1;
        }
        let block = buf.join("\n").trim().to_string();
        if !block.is_empty() {
            out.push(block);
        }
    }
    out
}

/// Normalises a single line for cross-text comparison: trim, collapse internal
/// whitespace, lowercase.
fn norm_line(line: &str) -> String {
    line.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

/// True for lines too generic to count toward a "distinctive" shared run:
/// blanks, diff scaffolding (`--- a/`, `+++ b/`, `@@`, `diff --git`, `index `),
/// and bare brackets/punctuation. Such lines are identical across all artifacts
/// of a kind and so must not, on their own, signal copying.
fn is_trivial_line(line: &str) -> bool {
    let n = norm_line(line);
    if n.len() < 8 {
        return true;
    }
    let t = line.trim_start();
    t.starts_with("--- ")
        || t.starts_with("+++ ")
        || t.starts_with("@@")
        || t.starts_with("diff --git")
        || t.starts_with("index ")
}

/// Longest run of *contiguous distinctive* lines from `example` that also appear
/// (normalized) anywhere in `fixture`. A run of 3+ is strong evidence the
/// fixture copied the example, as opposed to merely sharing domain vocabulary.
fn longest_shared_line_run(fixture: &str, example: &str) -> usize {
    let ex_lines: std::collections::HashSet<String> = example
        .lines()
        .map(norm_line)
        .filter(|l| !l.is_empty())
        .collect();
    let mut run = 0usize;
    let mut best = 0usize;
    for line in fixture.lines() {
        if !is_trivial_line(line) && ex_lines.contains(&norm_line(line)) {
            run += 1;
            best = best.max(run);
        } else {
            run = 0;
        }
    }
    best
}

/// A shared contiguous run of this many distinctive lines counts as fixture
/// contamination. Calibrated against the current corpus, whose maximum
/// incidental run is 2 (see the fixture-guard test).
pub const FIXTURE_RUN_THRESHOLD: usize = 3;

/// Normalises text to a lowercase word set: drops punctuation, collapses
/// whitespace. Mirrors the offline analysis used to find the contamination.
fn word_set(s: &str) -> std::collections::HashSet<String> {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .map(str::to_string)
        .collect()
}

/// Jaccard similarity of two strings' word sets, in `[0.0, 1.0]`.
pub fn jaccard(a: &str, b: &str) -> f64 {
    let (a, b) = (word_set(a), word_set(b));
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let inter = a.intersection(&b).count() as f64;
    let union = a.union(&b).count() as f64;
    inter / union
}

/// One contamination hit: an intent too close to a specific few-shot line.
#[derive(Debug)]
#[cfg_attr(not(test), allow(dead_code))]
pub struct Overlap {
    pub tool: String,
    pub name: String,
    pub arg: String,
    pub example: String,
    pub score: f64,
}

/// Flags whose *value* is user content the model reasons over (and so could
/// paraphrase a few-shot example), as opposed to a modifier like `--target` /
/// `--kind` / `--shell`. Currently just `lxman`'s `--for <command>`.
const CONTENT_FLAGS: &[&str] = &["--for"];

/// Collects every content-bearing input an intent feeds the model: the
/// positional `arg` plus the value of any [`CONTENT_FLAGS`] in `args`. These are
/// the strings that must not paraphrase a few-shot example. (Fixture `stdin` is
/// handled separately by [`find_fixture_overlaps`], which uses a line-level
/// near-duplicate test rather than word-set similarity.)
fn content_inputs(it: &Intent) -> Vec<String> {
    let mut v = Vec::new();
    if let Some(a) = it.arg.as_deref() {
        v.push(a.to_string());
    }
    let mut i = 0;
    while i < it.args.len() {
        if CONTENT_FLAGS.contains(&it.args[i].as_str()) {
            if let Some(val) = it.args.get(i + 1) {
                v.push(val.clone());
            }
            i += 2;
        } else {
            i += 1;
        }
    }
    v
}

/// Checks every intent's content inputs (see [`content_inputs`]) against its
/// tool's few-shot example inputs. Returns the list of overlaps at or above
/// [`THRESHOLD`], skipping intents flagged `allow_fewshot_overlap`. Intents with
/// no content input (stdin-only) and tools without a prompt are not checkable
/// and are silently skipped.
///
/// Consumed only by [`guard`] (and thus only by the build-time test); marked
/// `allow(dead_code)` for non-test builds of this binary crate.
#[cfg_attr(not(test), allow(dead_code))]
pub fn find_overlaps(intents: &[Intent]) -> Vec<Overlap> {
    let mut hits = Vec::new();
    for it in intents {
        if it.allow_fewshot_overlap {
            continue;
        }
        let inputs = content_inputs(it);
        if inputs.is_empty() {
            continue;
        }
        let Some(prompt) = read_system_prompt(&it.tool) else {
            continue;
        };
        let examples = example_inputs(&prompt);
        let mut best: Option<(f64, String, String)> = None;
        for arg in &inputs {
            for ex in &examples {
                let s = jaccard(arg, ex);
                if best.as_ref().map(|(b, _, _)| s > *b).unwrap_or(true) {
                    best = Some((s, arg.clone(), ex.clone()));
                }
            }
        }
        if let Some((score, arg, example)) = best {
            if score >= THRESHOLD {
                hits.push(Overlap {
                    tool: it.tool.clone(),
                    name: it.name.clone(),
                    arg,
                    example,
                    score,
                });
            }
        }
    }
    hits
}

/// One fixture-contamination hit: a stdin fixture that copies a contiguous run
/// of distinctive lines from one of its tool's prompt examples.
#[derive(Debug)]
#[cfg_attr(not(test), allow(dead_code))]
pub struct FixtureOverlap {
    pub tool: String,
    pub name: String,
    pub fixture: String,
    pub run: usize,
}

/// `acceptance/fixtures`, resolved relative to the workspace root.
fn fixtures_dir() -> PathBuf {
    let mut p = workspace_root();
    p.push("acceptance");
    p.push("fixtures");
    p
}

/// Checks every intent's `stdin` fixture against its tool's multi-line prompt
/// examples using a line-level near-duplicate test. Flags a fixture that shares
/// a contiguous run of [`FIXTURE_RUN_THRESHOLD`]+ distinctive lines with an
/// example — strong evidence of copy-paste, as opposed to the shared domain
/// vocabulary that makes word-set similarity useless here. Skips intents flagged
/// `allow_fewshot_overlap` and tools without a prompt.
///
/// Consumed only by [`guard`]; marked `allow(dead_code)` for non-test builds.
#[cfg_attr(not(test), allow(dead_code))]
pub fn find_fixture_overlaps(intents: &[Intent]) -> Vec<FixtureOverlap> {
    let mut hits = Vec::new();
    for it in intents {
        if it.allow_fewshot_overlap {
            continue;
        }
        let Some(stdin) = it.stdin.as_deref() else {
            continue;
        };
        let Some(prompt) = read_system_prompt(&it.tool) else {
            continue;
        };
        let fpath = fixtures_dir().join(stdin);
        let Ok(fixture) = std::fs::read_to_string(&fpath) else {
            // Missing/binary fixtures are not text-comparable; the engine will
            // surface a genuinely missing fixture at run time.
            continue;
        };
        let mut worst = 0usize;
        for block in example_blocks(&prompt) {
            worst = worst.max(longest_shared_line_run(&fixture, &block));
        }
        if worst >= FIXTURE_RUN_THRESHOLD {
            hits.push(FixtureOverlap {
                tool: it.tool.clone(),
                name: it.name.clone(),
                fixture: stdin.to_string(),
                run: worst,
            });
        }
    }
    hits
}

/// Build-time guard. `Ok(())` when no intent paraphrases a few-shot example —
/// neither via its `arg`/`--for` content (word-set similarity) nor via its
/// `stdin` fixture (line-level near-duplicate). Otherwise a multi-line error
/// naming every offender. Used by the unit test below so contamination is caught
/// by `cargo test`, not in production runs.
#[cfg_attr(not(test), allow(dead_code))]
pub fn guard(intents: &[Intent]) -> Result<(), String> {
    let arg_hits = find_overlaps(intents);
    let fix_hits = find_fixture_overlaps(intents);
    if arg_hits.is_empty() && fix_hits.is_empty() {
        return Ok(());
    }
    let mut msg = String::new();
    if !arg_hits.is_empty() {
        msg.push_str(&format!(
            "{} acceptance intent(s) are >= {:.0}% similar to a few-shot Input: \
             line in the tool's own system.txt. Such intents test prompt overfit, \
             not generalisation. Rewrite them to be lexically distinct, or — only \
             for genuinely destructive regression cases — set \
             `allow_fewshot_overlap = true` with a justifying comment.\n",
            arg_hits.len(),
            THRESHOLD * 100.0
        ));
        for h in &arg_hits {
            msg.push_str(&format!(
                "  [{:.2}] {}/{}\n        intent : {}\n        fewshot: {}\n",
                h.score, h.tool, h.name, h.arg, h.example
            ));
        }
    }
    if !fix_hits.is_empty() {
        msg.push_str(&format!(
            "{} acceptance fixture(s) copy a contiguous run of {}+ distinctive \
             lines from a few-shot example in the tool's system.txt. Such fixtures \
             test prompt overfit, not generalisation. Replace the fixture content \
             with a distinct realistic artifact.\n",
            fix_hits.len(),
            FIXTURE_RUN_THRESHOLD
        ));
        for h in &fix_hits {
            msg.push_str(&format!(
                "  [run={}] {}/{}\n        fixture: {}\n",
                h.run, h.tool, h.name, h.fixture
            ));
        }
    }
    Err(msg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_plain_and_flavored_inputs() {
        let p = "Examples:\nInput: list files\nInput (pcre): email address\n\
                 Input file is read from stdin\nInputs follow\n";
        let ex = example_inputs(p);
        assert_eq!(ex, vec!["list files", "email address"]);
    }

    #[test]
    fn jaccard_identical_is_one() {
        assert!(
            (jaccard("list all running containers", "list all running containers") - 1.0).abs()
                < 1e-9
        );
    }

    #[test]
    fn jaccard_disjoint_is_zero() {
        assert_eq!(jaccard("alpha beta", "gamma delta"), 0.0);
    }

    #[test]
    fn jaccard_punctuation_and_case_insensitive() {
        // "update all images to newest version" vs "...to the newest version"
        let s = jaccard(
            "update all images to newest version",
            "update all images to the newest version",
        );
        assert!(s > 0.8, "expected high similarity, got {s}");
    }

    #[test]
    fn content_flag_value_is_checked() {
        // An intent that passes its content via `--for` (lxman) must be caught,
        // not just positional `arg`. Build a synthetic intent mirroring lxman.
        let src = r#"
[[intent]]
tool = "lxman"
name = "echoes-fewshot-via-for"
args = ["--for", "grep"]
prose = true
"#;
        let intents = crate::intents::parse(src).unwrap();
        // lxman's system.txt has `Input: grep`, so this must register an overlap.
        let hits = find_overlaps(&intents);
        assert!(
            hits.iter()
                .any(|h| h.tool == "lxman" && h.score >= THRESHOLD),
            "guard must inspect --for values, not only positional arg"
        );
    }

    #[test]
    fn extracts_multiline_example_blocks() {
        let p = "Example 1:\n\nInput:\ndiff --git a/x b/x\n+added line one\n+added line two\n\
                 \nOutput: {\"k\":1}\n\nInput: short single line\nOutput: {}\n";
        let blocks = example_blocks(p);
        assert_eq!(blocks.len(), 2);
        assert!(blocks[0].contains("added line one"));
        assert!(blocks[0].contains("added line two"));
        assert!(!blocks[0].contains("Output"));
        assert_eq!(blocks[1], "short single line");
    }

    #[test]
    fn shared_run_ignores_vocabulary_counts_copies() {
        let example = "fn renew(&self) -> Result<Session> {\n\
                       let profile = self.provider.refresh();\n\
                       return Ok(profile.into_session());\n}";
        // A fixture sharing only diff scaffolding + generic words: no run.
        let vocab_only = "--- a/other.rs\n+++ b/other.rs\n@@ -1 +1 @@\n\
                          fn unrelated() -> u32 { 0 }";
        assert!(longest_shared_line_run(vocab_only, example) < FIXTURE_RUN_THRESHOLD);
        // A fixture that copies 3 contiguous distinctive lines verbatim: flagged.
        let copied = "prefix\nfn renew(&self) -> Result<Session> {\n\
                      let profile = self.provider.refresh();\n\
                      return Ok(profile.into_session());\nsuffix";
        assert!(longest_shared_line_run(copied, example) >= FIXTURE_RUN_THRESHOLD);
    }

    /// THE guard, over the real bundled intents. This is the test that makes
    /// few-shot contamination a build failure — both for arg/--for paraphrases
    /// and for stdin fixtures that copy prompt example artifacts.
    #[test]
    fn bundled_intents_are_not_fewshot_paraphrases() {
        let src = include_str!("../intents/intents.toml");
        let intents = crate::intents::parse(src).expect("intents.toml parses");
        if let Err(e) = guard(&intents) {
            panic!("\n{e}");
        }
    }

    /// Calibration guard: the current corpus's worst *incidental* fixture/example
    /// line run must stay safely below the threshold, so the threshold keeps its
    /// meaning. If this creeps up, the threshold (or the trivial-line filter)
    /// needs revisiting — it is not a licence to raise the bar silently.
    #[test]
    fn fixture_incidental_run_stays_below_threshold() {
        let src = include_str!("../intents/intents.toml");
        let intents = crate::intents::parse(src).expect("intents.toml parses");
        // With the corpus clean, find_fixture_overlaps returns nothing; this is
        // the partner assertion to bundled_intents_… spelling out the margin.
        assert!(
            find_fixture_overlaps(&intents).is_empty(),
            "a stdin fixture copies an example artifact; see guard() output"
        );
    }
}
