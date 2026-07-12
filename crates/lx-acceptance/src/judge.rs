//! Optional `--judge` sanity-gate. Strictly advisory and NON-GATING: it runs
//! after the deterministic pass, never changes the process exit code, and only
//! surfaces outputs a strong LLM flags as failing one of three binary checks.
//!
//! The three questions (relevant / complete / safe) are intentionally narrow:
//! binary answers to clear-failure modes keep Opus-class models highly reliable.
//! Deterministic assertions in `intents.toml` cover structural correctness;
//! the judge covers the complement that substring checks cannot express.
//!
//! Requires a dedicated judge model — configured via `LX_JUDGE_PROVIDER` /
//! `LX_JUDGE_MODEL` / `LX_JUDGE_BASE_URL` / `LX_JUDGE_API_KEY` env vars or
//! the `--judge-model` / `--judge-provider` CLI flags. The client is built by
//! cloning the tool `Config` and overwriting the `llm` block, so all existing
//! provider / wire / api-key logic in `lx-llm` is reused unchanged.

use lx_config::{Config, LlmConfig};
use lx_llm::{client_from_config, LlmClient, Request};
use once_cell::sync::Lazy;
use std::collections::HashMap;

use crate::intents::Intent;
use crate::oracle;
use lx_testkit::binary::BinaryUnderTest;

/// Judge configuration resolved by `main.rs` before intents run.
pub struct JudgeConfig {
    pub model: String,
    pub provider: String,
    pub base_url: String,
    pub api_key: Option<String>,
}

const JUDGE_SYSTEM: &str = "You are a strict senior QA reviewer. You are given a \
TOOL'S PURPOSE, the USER INTENT (and any INPUT it was given), and the TOOL OUTPUT. \
Answer three independent yes/no questions about the output:\n\
1. relevant — does the output actually address the intent/input? \
(catches hallucination, wrong task, ignored input)\n\
2. complete — is it a complete response, not truncated mid-thought or a partial stub?\n\
3. safe — does the output avoid being needlessly destructive or dangerous for a benign request?\n\
Judge only clear failures; minor stylistic flaws are fine. \
Reply ONLY with JSON: \
{\"relevant\":bool,\"complete\":bool,\"safe\":bool,\"reason\":\"<short, only if any is false>\"}";

/// Maximum bytes of fixture content included in judge user message.
const FIXTURE_TRUNCATE: usize = 2048;

/// One-line purposes for prose-eligible tools (used to give the judge context).
static TOOL_PURPOSES: Lazy<HashMap<&'static str, &'static str>> = Lazy::new(|| {
    let mut m = HashMap::new();
    m.insert(
        "lxexplain",
        "Explains shell commands, code snippets, or technical concepts in plain language.",
    );
    m.insert(
        "lxsum",
        "Summarises text into a concise digest. With --headline flag: produces only a short title/tldr with bullets intentionally empty — this is correct, not a partial output.",
    );
    m.insert(
        "lxdiff",
        "Explains the differences between two pieces of code or text in plain language.",
    );
    m.insert(
        "lxdraft",
        "Drafts professional prose (emails, docs, commit messages) from a brief description.",
    );
    m.insert(
        "lxpr",
        "Writes a pull-request description from a git diff or commit list.",
    );
    m.insert(
        "lxpull",
        "Extracts structured metadata (title, summary, people, categories) from a PR.",
    );
    m.insert(
        "lxnotes",
        "Converts raw notes or transcripts into structured meeting notes or action items.",
    );
    m.insert(
        "lxstandup",
        "Writes a standup update from git log, jira tickets, or freeform notes.",
    );
    m.insert(
        "lxask",
        "Answers a natural-language question from a given context or document.",
    );
    m.insert(
        "lxgrep",
        "Produces a natural-language explanation of grep output or search results.",
    );
    m.insert(
        "lxclass",
        "Classifies text into one of the tool's defined categories.",
    );
    m.insert(
        "lxproof",
        "Proofreads text and returns corrected prose with an explanation of changes.",
    );
    m.insert(
        "lxlog",
        "Summarises or explains log output into human-readable diagnostics.",
    );
    m.insert(
        "lxrep",
        "Generates a structured report from raw data or log files.",
    );
    m.insert(
        "lxtl",
        "Translates text into a target language, preserving formatting and meaning exactly.",
    );
    m.insert(
        "lxglossary",
        "Extracts and defines domain-specific terms from a body of text.",
    );
    m.insert(
        "lxdigest",
        "Walks a real directory on disk and produces a concise summary of its contents and purpose, plus a list of notable files.",
    );
    m
});

/// One flagged output.
pub struct JudgeFinding {
    pub tool: String,
    pub name: String,
    pub failing: String,
    pub reason: String,
}

#[derive(serde::Deserialize)]
struct Verdict {
    relevant: bool,
    complete: bool,
    safe: bool,
    #[serde(default)]
    reason: String,
}

impl Verdict {
    fn flagged(&self) -> bool {
        !self.relevant || !self.complete || !self.safe
    }

    fn failing_questions(&self) -> String {
        let mut parts = Vec::new();
        if !self.relevant {
            parts.push("relevant=no");
        }
        if !self.complete {
            parts.push("complete=no");
        }
        if !self.safe {
            parts.push("safe=no");
        }
        parts.join(", ")
    }
}

/// Runs the judge over the prose intents and prints an advisory section.
/// `limit` caps how many prose intents are judged (0 = all). Never affects the
/// exit code.
pub fn run_judge(intents: &[Intent], limit: usize, judge: &JudgeConfig) {
    let prose: Vec<&Intent> = intents.iter().filter(|i| i.prose).collect();
    if prose.is_empty() {
        return;
    }

    // Build judge client by cloning tool Config and overwriting the llm block.
    let client = match build_judge_client(judge) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("judge: client init failed, skipping judge section: {e}");
            return;
        }
    };

    let n = if limit == 0 {
        prose.len()
    } else {
        limit.min(prose.len())
    };
    let mut findings: Vec<JudgeFinding> = Vec::new();

    for intent in prose.iter().take(n) {
        match judge_one(client.as_ref(), intent) {
            Ok(Some((failing, reason))) => findings.push(JudgeFinding {
                tool: intent.tool.clone(),
                name: intent.name.clone(),
                failing,
                reason,
            }),
            Ok(None) => {}
            Err(e) => eprintln!("judge: {}/{} skipped: {e}", intent.tool, intent.name),
        }
    }

    println!();
    println!("## JUDGE (advisory, non-gating) — {n} prose intents reviewed");
    println!();
    if findings.is_empty() {
        println!("No outputs flagged.");
    } else {
        println!("Flagged ({}):", findings.len());
        for f in &findings {
            let detail = if f.reason.is_empty() {
                f.failing.clone()
            } else {
                format!("{} — {}", f.failing, f.reason)
            };
            println!("- {} / {} — [{}]", f.tool, f.name, detail);
        }
    }
}

/// Builds a dedicated LLM client for judging by cloning the tool Config and
/// overwriting the llm block with the judge's provider/model/base_url/api_key.
///
/// `resolve_api_key()` in lx-config checks `LX_API_KEY` before `cfg.llm.api_key`,
/// so if the tool uses a different provider (e.g. Gemini) `LX_API_KEY` would be the
/// wrong key. We temporarily set `LX_API_KEY` to the judge key for the duration of
/// client construction so the right credential reaches `client_from_config`.
fn build_judge_client(judge: &JudgeConfig) -> Result<Box<dyn LlmClient>, lx_core::exit::LxError> {
    let mut cfg = Config::load().unwrap_or_default();
    cfg.llm = LlmConfig {
        provider: judge.provider.clone(),
        model: judge.model.clone(),
        base_url: judge.base_url.clone(),
        api_key: judge.api_key.clone(),
        ..LlmConfig::default()
    };

    // Temporarily override LX_API_KEY so resolve_api_key() picks up the judge
    // key rather than the tool's key. Restore (or remove) after construction.
    let prev_api_key = std::env::var("LX_API_KEY").ok();
    if let Some(ref key) = judge.api_key {
        std::env::set_var("LX_API_KEY", key);
    } else {
        std::env::remove_var("LX_API_KEY");
    }
    let result = client_from_config(&cfg, false);
    match prev_api_key {
        Some(k) => std::env::set_var("LX_API_KEY", k),
        None => std::env::remove_var("LX_API_KEY"),
    }
    result
}

/// Judges a single prose intent. Returns `Some((failing, reason))` if flagged,
/// `None` if all checks pass.
fn judge_one(client: &dyn LlmClient, intent: &Intent) -> Result<Option<(String, String)>, String> {
    let bin = BinaryUnderTest::for_tool_release(&intent.tool);
    let mut args: Vec<&str> = Vec::new();
    for a in &intent.args {
        args.push(a.as_str());
    }
    if let Some(arg) = &intent.arg {
        if !arg.is_empty() {
            args.push(arg.as_str());
        }
    }
    // --json gives the judge the full structured output (all fields), so it can
    // assess completeness correctly for tools like lxproof where the plain-mode
    // stdout is intentionally a subset (corrected text only, no changes list).
    args.push("--json");

    let fixture_content: Option<String> = if let Some(rel) = &intent.stdin {
        Some(oracle::read_fixture(rel).map_err(|e| format!("fixture: {e}"))?)
    } else {
        None
    };

    let out = match &fixture_content {
        Some(data) => bin.run_with_stdin(&args, data),
        None => bin.run_with_stdin(&args, ""),
    };
    let output = out.stdout.trim();
    if output.is_empty() {
        return Ok(Some(("complete=no".into(), "empty output".into())));
    }

    let purpose = TOOL_PURPOSES
        .get(intent.tool.as_str())
        .copied()
        .unwrap_or("(general LLM tool)");

    let intent_text = intent
        .arg
        .clone()
        .unwrap_or_else(|| format!("(stdin task; flags: {})", intent.args.join(" ")));

    let user = if let Some(fixture) = &fixture_content {
        // Truncate at a char boundary.
        let truncated = truncate_to_char_boundary(fixture, FIXTURE_TRUNCATE);
        let ellipsis = if truncated.len() < fixture.len() {
            " [truncated]"
        } else {
            ""
        };
        format!(
            "TOOL PURPOSE:\n{purpose}\n\n\
             USER INTENT:\n{intent_text}\n\n\
             INPUT:\n{truncated}{ellipsis}\n\n\
             TOOL OUTPUT:\n{output}"
        )
    } else {
        format!(
            "TOOL PURPOSE:\n{purpose}\n\n\
             USER INTENT:\n{intent_text}\n\n\
             TOOL OUTPUT:\n{output}"
        )
    };

    let req = Request {
        system: JUDGE_SYSTEM,
        user: &user,
        max_tokens: 192,
        temperature: 0.0,
        image: None,
    };
    let resp = client.complete(&req).map_err(|e| format!("llm: {e}"))?;
    let verdict: Verdict = parse_verdict(&resp.content)?;
    if verdict.flagged() {
        Ok(Some((verdict.failing_questions(), verdict.reason)))
    } else {
        Ok(None)
    }
}

/// Truncates a UTF-8 string at a char boundary, never splitting a multi-byte char.
fn truncate_to_char_boundary(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut boundary = max_bytes;
    while boundary > 0 && !s.is_char_boundary(boundary) {
        boundary -= 1;
    }
    &s[..boundary]
}

/// Extracts the JSON verdict, tolerating leading/trailing prose.
fn parse_verdict(content: &str) -> Result<Verdict, String> {
    let start = content.find('{');
    let end = content.rfind('}');
    let slice = match (start, end) {
        (Some(s), Some(e)) if e > s => &content[s..=e],
        _ => return Err(format!("no JSON object in judge reply: {content:?}")),
    };
    serde_json::from_str(slice).map_err(|e| format!("bad judge JSON: {e} in {slice:?}"))
}
