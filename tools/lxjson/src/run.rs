use lx_config::Config;
use lx_core::error::LxError;
use lx_llm::{inject_lang, parse_response, LlmClient, Request};
use serde::{Deserialize, Serialize};

pub const SYSTEM_TEMPLATE: &str = include_str!("../prompts/system.txt");
const MAX_TOKENS: u32 = 1024;

/// Output of `lxjson`.
#[derive(Debug, Serialize, Deserialize)]
pub struct Output {
    /// The repaired, valid JSON string.
    pub json: String,
    /// How the repair was performed: "local" or "llm".
    pub method: String,
    /// Human-readable descriptions of what was changed.
    pub changes: Vec<String>,
}

impl Output {
    /// Return the repaired JSON for plain (non-JSON) output.
    /// The result IS the repaired JSON — pipe-safe.
    pub fn to_plain(&self) -> String {
        self.json.clone()
    }
}

// ── Local repair ──────────────────────────────────────────────────────────────

/// Attempt to repair JSON locally without an LLM call.
/// Returns `Ok(Some(output))` if repair succeeded, `Ok(None)` if it failed.
fn try_local_repair(input: &str) -> Option<Output> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }

    // Fast path: already valid JSON.
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(trimmed) {
        let compact = serde_json::to_string(&v).unwrap_or_else(|_| trimmed.to_owned());
        return Some(Output {
            json: compact,
            method: "local".to_string(),
            changes: vec![],
        });
    }

    let mut s = trimmed.to_owned();
    let mut changes: Vec<String> = Vec::new();

    // Pass 1: single quotes → double quotes (only outside already-valid strings).
    let replaced = replace_single_quotes(&s);
    if replaced != s {
        changes.push("converted single quotes to double quotes".to_string());
        s = replaced;
    }

    // Pass 2: unquoted keys → quoted keys.
    let quoted = quote_unquoted_keys(&s);
    if quoted != s {
        changes.push("added double quotes around unquoted keys".to_string());
        s = quoted;
    }

    // Pass 3: trailing commas before `}` or `]`.
    let no_trailing = remove_trailing_commas(&s);
    if no_trailing != s {
        changes.push("removed trailing comma(s) before closing bracket or brace".to_string());
        s = no_trailing;
    }

    // Pass 4: close unclosed brackets/braces.
    let closed = close_open_brackets(&s);
    if closed != s {
        changes.push("added missing closing brackets/braces".to_string());
        s = closed;
    }

    // Validate the result.
    match serde_json::from_str::<serde_json::Value>(&s) {
        Ok(v) => {
            let compact = serde_json::to_string(&v).unwrap_or(s);
            Some(Output {
                json: compact,
                method: "local".to_string(),
                changes,
            })
        }
        Err(_) => None,
    }
}

/// Replace single-quote delimiters with double-quote delimiters.
/// Uses a simple state machine; does not handle all edge cases (good enough
/// for common copy-paste JSON mistakes).
fn replace_single_quotes(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_double = false;
    let mut in_single = false;
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '\\' if in_double || in_single => {
                out.push(c);
                if let Some(next) = chars.next() {
                    out.push(next);
                }
            }
            '"' if !in_single => {
                in_double = !in_double;
                out.push(c);
            }
            '\'' if !in_double => {
                in_single = !in_single;
                out.push('"');
            }
            _ => out.push(c),
        }
    }
    out
}

/// Add double quotes around bare (unquoted) object keys.
/// Matches patterns like `{ key:` or `, key:` where `key` is a bare identifier.
fn quote_unquoted_keys(s: &str) -> String {
    // Regex-free: scan for patterns `[{,\s]identifier:` not already quoted.
    let mut out = String::with_capacity(s.len() + 32);
    let bytes = s.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        let ch = bytes[i] as char;

        // Inside a string literal — copy verbatim including escapes.
        if ch == '"' {
            out.push(ch);
            i += 1;
            while i < len {
                let sc = bytes[i] as char;
                out.push(sc);
                i += 1;
                if sc == '\\' && i < len {
                    out.push(bytes[i] as char);
                    i += 1;
                } else if sc == '"' {
                    break;
                }
            }
            continue;
        }

        // After `{` or `,` or at start, skip whitespace then check for bare key.
        if ch == '{' || ch == ',' || ch == '[' {
            out.push(ch);
            i += 1;

            // Skip whitespace.
            let ws_start = out.len();
            let _ = ws_start;
            let mut ws = String::new();
            while i < len && (bytes[i] as char).is_whitespace() {
                ws.push(bytes[i] as char);
                i += 1;
            }

            // If next char is a letter/underscore and NOT a double-quote → bare key.
            if i < len {
                let nc = bytes[i] as char;
                if nc.is_alphabetic() || nc == '_' || nc == '$' {
                    // Read identifier.
                    let mut ident = String::new();
                    while i < len {
                        let ic = bytes[i] as char;
                        if ic.is_alphanumeric() || ic == '_' || ic == '$' || ic == '-' {
                            ident.push(ic);
                            i += 1;
                        } else {
                            break;
                        }
                    }
                    // Check if followed by `:` (possibly with whitespace).
                    let mut after_ws = String::new();
                    let saved_i = i;
                    while i < len && (bytes[i] as char).is_whitespace() {
                        after_ws.push(bytes[i] as char);
                        i += 1;
                    }
                    if i < len && bytes[i] == b':' {
                        // It's a bare key — wrap it.
                        out.push_str(&ws);
                        out.push('"');
                        out.push_str(&ident);
                        out.push('"');
                        out.push_str(&after_ws);
                        // i is pointing at `:`, will be copied next iteration.
                        continue;
                    } else {
                        // Not a key, restore.
                        out.push_str(&ws);
                        out.push_str(&ident);
                        out.push_str(&after_ws);
                        i = saved_i; // re-scan from after ident (after_ws skipped)
                                     // after_ws is already in out, but i points before it — fix:
                                     // actually saved_i is after ident so we need to re-add after_ws
                                     // This path means it wasn't a key; just continue.
                        continue;
                    }
                }
            }
            out.push_str(&ws);
            continue;
        }

        out.push(ch);
        i += 1;
    }

    out
}

/// Remove trailing commas immediately before `}` or `]`.
fn remove_trailing_commas(s: &str) -> String {
    // Use a simple pass: find `,` followed by optional whitespace and then `}` or `]`.
    let mut out = String::with_capacity(s.len());
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len();
    let mut i = 0;
    let mut in_string = false;

    while i < len {
        let c = chars[i];

        if in_string {
            out.push(c);
            if c == '\\' && i + 1 < len {
                i += 1;
                out.push(chars[i]);
            } else if c == '"' {
                in_string = false;
            }
            i += 1;
            continue;
        }

        if c == '"' {
            in_string = true;
            out.push(c);
            i += 1;
            continue;
        }

        if c == ',' {
            // Look ahead past whitespace.
            let mut j = i + 1;
            while j < len && chars[j].is_whitespace() {
                j += 1;
            }
            if j < len && (chars[j] == '}' || chars[j] == ']') {
                // Skip the comma; whitespace will be copied normally.
                i += 1;
                continue;
            }
        }

        out.push(c);
        i += 1;
    }

    out
}

/// Count unclosed `{` and `[` and append the missing closing chars.
fn close_open_brackets(s: &str) -> String {
    let mut stack: Vec<char> = Vec::new();
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len();
    let mut i = 0;
    let mut in_string = false;

    while i < len {
        let c = chars[i];
        if in_string {
            if c == '\\' && i + 1 < len {
                i += 2;
                continue;
            }
            if c == '"' {
                in_string = false;
            }
            i += 1;
            continue;
        }
        match c {
            '"' => {
                in_string = true;
            }
            '{' => stack.push('}'),
            '[' => stack.push(']'),
            '}' | ']' if stack.last() == Some(&c) => {
                stack.pop();
            }
            _ => {}
        }
        i += 1;
    }

    if stack.is_empty() {
        return s.to_owned();
    }

    let mut out = s.to_owned();
    while let Some(close) = stack.pop() {
        out.push(close);
    }
    out
}

// ── Core function ─────────────────────────────────────────────────────────────

/// Core logic for lxjson.
///
/// Pure function: no I/O, no process::exit. Testable with MockLlmClient.
///
/// Strategy:
/// 1. Try local repair (fast, no network).
/// 2. If local repair fails, delegate to the LLM.
pub fn run(input: &str, config: &Config, client: &dyn LlmClient) -> Result<Output, LxError> {
    if input.trim().is_empty() {
        return Err(LxError::BadUsage("no input provided".to_string()));
    }

    // Attempt local repair first.
    if let Some(output) = try_local_repair(input) {
        return Ok(output);
    }

    // Fallback: ask the LLM.
    let system = inject_lang(SYSTEM_TEMPLATE, &config.output.lang);

    let req = Request {
        system: &system,
        user: input.trim(),
        max_tokens: MAX_TOKENS,
        temperature: 0.0,
        image: None,
    };

    let resp = client.complete(&req).map_err(LxError::from)?;

    parse_response::<Output>(&resp.content)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn already_valid_json_is_passed_through() {
        let out = try_local_repair(r#"{"key":"value"}"#).unwrap();
        assert_eq!(out.method, "local");
        // Parse to verify it's still valid.
        serde_json::from_str::<serde_json::Value>(&out.json).unwrap();
    }

    #[test]
    fn trailing_comma_is_fixed_locally() {
        let out = try_local_repair(r#"{"a":1,"b":2,}"#).unwrap();
        assert_eq!(out.method, "local");
        let v: serde_json::Value = serde_json::from_str(&out.json).unwrap();
        assert_eq!(v["a"], 1);
        assert_eq!(v["b"], 2);
    }

    #[test]
    fn single_quotes_are_fixed_locally() {
        let out = try_local_repair("{'host': 'localhost'}").unwrap();
        assert_eq!(out.method, "local");
        serde_json::from_str::<serde_json::Value>(&out.json).unwrap();
    }

    #[test]
    fn missing_bracket_is_closed_locally() {
        let out = try_local_repair(r#"{"items":[1,2,3"#).unwrap();
        assert_eq!(out.method, "local");
        serde_json::from_str::<serde_json::Value>(&out.json).unwrap();
    }

    #[test]
    fn replace_single_quotes_basic() {
        assert_eq!(replace_single_quotes("{'a':'b'}"), r#"{"a":"b"}"#);
    }

    #[test]
    fn remove_trailing_commas_basic() {
        assert_eq!(remove_trailing_commas(r#"{"a":1,}"#), r#"{"a":1}"#);
        assert_eq!(remove_trailing_commas(r#"[1,2,]"#), r#"[1,2]"#);
    }

    #[test]
    fn close_open_brackets_basic() {
        assert_eq!(close_open_brackets(r#"{"a":1"#), r#"{"a":1}"#);
        assert_eq!(close_open_brackets(r#"[1,2"#), r#"[1,2]"#);
        // Nested.
        assert_eq!(close_open_brackets(r#"{"a":[1"#), r#"{"a":[1]}"#);
    }
}
