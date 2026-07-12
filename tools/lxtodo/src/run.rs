use lx_config::Config;
use lx_core::error::LxError;
use lx_llm::{inject_lang, parse_response, LlmClient, Request};
use serde::{Deserialize, Serialize};

pub const SYSTEM_TEMPLATE: &str = include_str!("../prompts/system.txt");
/// Tight token limit: each TODO item is short; allow up to ~50 items.
const MAX_TOKENS: u32 = 1024;

// ── TODO patterns for local pre-processing ───────────────────────────────────

/// Keywords that mark action items in code comments.
const TODO_KEYWORDS: &[&str] = &[
    "TODO", "FIXME", "HACK", "XXX", "NOTE", "OPTIMIZE", "BUG", "REVIEW",
];

// ── Output types ─────────────────────────────────────────────────────────────

/// A single extracted TODO item.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TodoItem {
    /// Source file, if known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    /// Line number (1-based), if known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u64>,
    /// The TODO text, trimmed.
    pub text: String,
}

/// Output of `lxtodo`.
#[derive(Debug, Serialize, Deserialize)]
pub struct Output {
    pub todos: Vec<TodoItem>,
}

impl Output {
    /// Render as human-readable plain text for stdout.
    ///
    /// Format: `file:line: text` if file/line known, else just `text`.
    pub fn to_plain(&self) -> String {
        if self.todos.is_empty() {
            return String::new();
        }
        self.todos
            .iter()
            .map(|t| match (&t.file, t.line) {
                (Some(f), Some(l)) => format!("{f}:{l}: {}", t.text),
                (Some(f), None) => format!("{f}: {}", t.text),
                (None, Some(l)) => format!("{l}: {}", t.text),
                (None, None) => t.text.clone(),
            })
            .collect::<Vec<_>>()
            .join("\n")
            + "\n"
    }
}

// ── Local pre-processing ──────────────────────────────────────────────────────

/// Quickly scan input for lines that contain TODO-style keywords.
///
/// Returns a list of `(line_number, stripped_text)` for any matching lines.
/// This is used to give the LLM a focused, pre-filtered view of the input,
/// and also to provide accurate line numbers that the LLM alone cannot reliably
/// infer.
pub fn local_scan(input: &str) -> Vec<(u64, String)> {
    let mut hits = Vec::new();
    for (idx, line) in input.lines().enumerate() {
        let upper = line.to_uppercase();
        if TODO_KEYWORDS
            .iter()
            .any(|kw| upper.contains(&format!("{kw}:")))
        {
            // Strip leading comment characters and whitespace.
            let text = strip_comment_prefix(line.trim());
            hits.push((idx as u64 + 1, text.to_string()));
        }
    }
    hits
}

/// Remove common comment prefixes (`//`, `#`, `*`, `/*`, `--`) and leading
/// whitespace so the LLM gets clean text.
fn strip_comment_prefix(line: &str) -> &str {
    let s = line.trim_start_matches(|c: char| c.is_whitespace());
    // Try each prefix in order; take the first match.
    for prefix in ["//", "/*", "#", "* ", "--", "*"] {
        if let Some(rest) = s.strip_prefix(prefix) {
            return rest.trim_start();
        }
    }
    s
}

// ── Public run() function ─────────────────────────────────────────────────────

/// Core logic for lxtodo.
///
/// Security properties (`fsbound`, `untrusted`):
/// - `untrusted`: The system prompt instructs the model to ignore any
///   instructions embedded in the user data.
/// - `fsbound`: File path enforcement is handled in `main.rs` via
///   `lx_core::io::read_file` with an `allowed_root`; `run()` receives only
///   the already-read text.
///
/// Local pre-processing:
/// 1. Scan for standard TODO keywords locally using `local_scan()`.
/// 2. If the input is non-trivially large, build a focused context for the LLM
///    that includes only the hit lines with their line numbers.
/// 3. Send to LLM for semantic enrichment (catches non-standard markers,
///    implicit action items, etc.).
pub fn run(input: &str, config: &Config, client: &dyn LlmClient) -> Result<Output, LxError> {
    if input.trim().is_empty() {
        return Ok(Output { todos: vec![] });
    }

    // Local scan: extract line numbers and candidate text.
    let local_hits = local_scan(input);

    // Build the LLM user message: if we found local hits, annotate them with
    // line numbers so the LLM can echo them back. Otherwise send the raw input.
    let user_message: String = if local_hits.is_empty() {
        // No standard keywords found — ask LLM to find non-standard items.
        input.trim().to_string()
    } else {
        // Provide pre-extracted lines with line numbers for accuracy.
        local_hits
            .iter()
            .map(|(ln, text)| format!("line {ln}: {text}"))
            .collect::<Vec<_>>()
            .join("\n")
    };

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

    parse_response::<Output>(&resp.content)
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_scan_finds_todo() {
        let hits = local_scan("// TODO: fix this\nlet x = 1;");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].0, 1);
        assert!(hits[0].1.contains("TODO"));
    }

    #[test]
    fn local_scan_finds_multiple_keywords() {
        let input = "// FIXME: broken\n// HACK: workaround\nclean line\n// TODO: cleanup";
        let hits = local_scan(input);
        assert_eq!(hits.len(), 3);
    }

    #[test]
    fn local_scan_empty_returns_empty() {
        let hits = local_scan("");
        assert!(hits.is_empty());
    }

    #[test]
    fn local_scan_no_keywords_returns_empty() {
        let hits = local_scan("fn main() { println!(\"hello\"); }");
        assert!(hits.is_empty());
    }

    #[test]
    fn to_plain_with_file_and_line() {
        let out = Output {
            todos: vec![TodoItem {
                file: Some("src/main.rs".to_string()),
                line: Some(42),
                text: "TODO: fix this".to_string(),
            }],
        };
        let plain = out.to_plain();
        assert_eq!(plain.trim(), "src/main.rs:42: TODO: fix this");
    }

    #[test]
    fn to_plain_without_location() {
        let out = Output {
            todos: vec![TodoItem {
                file: None,
                line: None,
                text: "FIXME: broken".to_string(),
            }],
        };
        assert_eq!(out.to_plain().trim(), "FIXME: broken");
    }

    #[test]
    fn to_plain_empty_todos_returns_empty() {
        let out = Output { todos: vec![] };
        assert_eq!(out.to_plain(), String::new());
    }

    #[test]
    fn strip_comment_prefix_removes_slashes() {
        assert_eq!(strip_comment_prefix("// TODO: fix"), "TODO: fix");
        assert_eq!(strip_comment_prefix("# TODO: fix"), "TODO: fix");
        assert_eq!(strip_comment_prefix("* TODO: fix"), "TODO: fix");
    }
}
