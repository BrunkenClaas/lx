#![forbid(unsafe_code)]

use lx_core::exit::LxError;
use serde_json::Value;

use crate::lang::strip_lang_fallback;

/// Strip markdown code fences from a model response if present.
///
/// Handles ` ```json\n...\n``` ` and ` ```\n...\n``` ` variants.
fn strip_code_fences(s: &str) -> &str {
    let s = s.trim();
    let s = if let Some(inner) = s.strip_prefix("```json") {
        inner.trim_start_matches('\n')
    } else if let Some(inner) = s.strip_prefix("```") {
        inner.trim_start_matches('\n')
    } else {
        return s;
    };
    if let Some(inner) = s.strip_suffix("```") {
        inner.trim_end()
    } else {
        s
    }
}

/// Extract the first complete, balanced JSON value (`{…}` or `[…]`) from `s`.
///
/// Tolerates leading prose ("Here is the JSON: {…}") and trailing junk or a
/// second object after the first value. Returns `None` if no balanced value is
/// found (including truncated input).
fn extract_json_value(s: &str) -> Option<&str> {
    let bytes = s.as_bytes();
    // Find the first structural opener.
    let start = bytes.iter().position(|&b| b == b'{' || b == b'[')?;
    let mut stack: Vec<u8> = Vec::new();
    let mut in_string = false;
    let mut escaped = false;

    for (i, &b) in bytes[start..].iter().enumerate() {
        let abs = start + i;
        if escaped {
            escaped = false;
            continue;
        }
        if in_string {
            match b {
                b'\\' => escaped = true,
                b'"' => in_string = false,
                _ => {}
            }
            continue;
        }
        match b {
            b'"' => in_string = true,
            b'{' | b'[' => stack.push(b),
            b'}' | b']' => {
                stack.pop();
                if stack.is_empty() {
                    return Some(&s[start..=abs]);
                }
            }
            _ => {}
        }
    }
    None // unbalanced (truncated)
}

/// Record a safe truncation end-offset at the given container depth, growing the
/// per-depth vector as needed.
fn record_safe(safe_end_at_depth: &mut Vec<Option<usize>>, depth: usize, end: usize) {
    if safe_end_at_depth.len() <= depth {
        safe_end_at_depth.resize(depth + 1, None);
    }
    safe_end_at_depth[depth] = Some(end);
}

/// Salvage a truncated JSON value (`{…` or `[…`) by closing it at the outermost
/// open collection.
///
/// Models that hit the `max_tokens` ceiling stop mid-response, leaving an
/// unbalanced value that `extract_json_value` cannot recover. This function
/// walks the value tracking container depth and records, per depth, the byte
/// offset right after each *complete* element/member. On reaching the truncated
/// end it salvages at the outermost open array (the top-level collection whose
/// repeated elements we want to preserve), drops any partial trailing element
/// there entirely, and appends the closers for every still-open container — so
/// the result is always valid JSON containing only complete elements. If no
/// array is open it falls back to the deepest object, keeping its complete
/// members.
///
/// Returns `None` if the input is not a truncated structure (already balanced,
/// no opener) or if nothing complete precedes the truncation point.
fn salvage_truncated_json(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    let start = bytes.iter().position(|&b| b == b'{' || b == b'[')?;

    // Stack of open structural characters (`{` or `[`).
    let mut stack: Vec<u8> = Vec::new();
    let mut in_string = false;
    let mut escaped = false;

    // For each container depth, the byte offset (exclusive end) right after the
    // most recent *complete* element/member at that depth. Indexed by
    // `stack.len()` at the moment the boundary (comma or nested close) occurred.
    // `safe_end_at_depth[d]` is the salvage point if depth `d` is the level we
    // close from. Deeper entries are cleared whenever we open a new container,
    // because any boundary recorded inside a now-abandoned element is invalid.
    let mut safe_end_at_depth: Vec<Option<usize>> = Vec::new();

    for (i, &b) in bytes[start..].iter().enumerate() {
        let abs = start + i;
        if escaped {
            escaped = false;
            continue;
        }
        if in_string {
            match b {
                b'\\' => escaped = true,
                b'"' => in_string = false,
                _ => {}
            }
            continue;
        }
        match b {
            b'"' => in_string = true,
            b'{' | b'[' => {
                stack.push(b);
                // Entering a new (empty) container: clear any stale safe
                // boundaries at this depth or deeper — they belonged to elements
                // inside a sibling container that we are no longer building.
                safe_end_at_depth.truncate(stack.len());
            }
            b'}' | b']' => {
                stack.pop();
                if stack.is_empty() {
                    // Already balanced — not truncated; let the normal path
                    // handle it.
                    return None;
                }
                // A nested structure just closed at the parent's depth: the
                // parent now has one more complete element ending here.
                record_safe(&mut safe_end_at_depth, stack.len(), abs + 1);
            }
            // A comma terminates a complete value at the current depth.
            b',' => record_safe(&mut safe_end_at_depth, stack.len(), abs), // exclude comma
            _ => {}
        }
    }

    // If the stack is empty we never had an unbalanced structure.
    if stack.is_empty() {
        return None;
    }

    // Salvage at the outermost array on the open stack — the top-level
    // collection whose repeated elements we want to preserve. Any partial
    // trailing element there (a half-built row/record, however deeply it was
    // nested) is dropped entirely. If no array is open (e.g. a truncated flat
    // object), fall back to the deepest open object so its complete members are
    // kept.
    let salvage_depth = match stack.iter().position(|&b| b == b'[') {
        Some(idx) => idx + 1, // depth is 1-based: stack[idx] sits at depth idx+1
        None => stack.len(),  // no array open — keep complete object members
    };

    let end = safe_end_at_depth.get(salvage_depth).copied().flatten()?;
    let mut salvaged = s[start..end].to_string();

    // Close every container from the salvage depth outward (innermost first).
    for &opener in stack[..salvage_depth].iter().rev() {
        salvaged.push(if opener == b'{' { '}' } else { ']' });
    }

    // Only accept the salvage if it actually parses.
    if serde_json::from_str::<Value>(&salvaged).is_ok() {
        Some(salvaged)
    } else {
        None
    }
}

/// Sanitize a model response so that serde_json can parse it.
///
/// Two classes of problems are fixed:
///
/// 1. **Bare control characters** (U+0000–U+001F) inside JSON string values —
///    local models sometimes emit a literal newline, tab, or other control byte
///    instead of the proper `\n` / `\t` escape.  Each is replaced with its
///    `\uXXXX` form (or the short aliases `\n`, `\r`, `\t`).  Outside strings,
///    control chars are harmless whitespace and are dropped.
///
/// 2. **Invalid backslash escapes** inside JSON string values — local models
///    sometimes emit `\p`, `\d`, `\1`, `\s`, `\w`, `\A`, etc. inside awk/sed
///    patterns or regex strings.  JSON only allows `\"`, `\\`, `\/`, `\b`,
///    `\f`, `\n`, `\r`, `\t`, and `\uXXXX`.  Any `\X` where X is not one of
///    those is fixed by doubling the backslash to `\\X`, turning it into a
///    literal backslash followed by the character — which is what the model
///    intended anyway.
///
/// IMPORTANT: unchanged spans are copied as `&str` slices, never byte-by-byte
/// via `b as char`, which would corrupt multi-byte UTF-8 sequences.
fn escape_control_chars(s: &str) -> std::borrow::Cow<'_, str> {
    let bytes = s.as_bytes();
    let mut in_string = false;
    // When `Some(next_byte)`, the previous byte was `\` inside a string; we
    // need to inspect the next byte to decide whether the escape is valid.
    let mut pending_backslash = false;

    // Fast path: scan for any byte that needs work before allocating.
    let needs_work = bytes.iter().any(|&b| b < 0x20) || {
        // Also check for invalid \X escapes inside strings.
        let mut in_s = false;
        let mut esc = false;
        let mut found = false;
        for &b in bytes {
            if esc {
                if in_s
                    && !matches!(
                        b,
                        b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't' | b'u'
                    )
                {
                    found = true;
                    break;
                }
                esc = false;
                continue;
            }
            match b {
                b'"' => in_s = !in_s,
                b'\\' if in_s => esc = true,
                _ => {}
            }
        }
        found
    };

    if !needs_work {
        return std::borrow::Cow::Borrowed(s);
    }

    let mut out = String::with_capacity(s.len() + 16);
    // `copy_start` tracks the beginning of the next verbatim slice to flush.
    let mut copy_start = 0usize;
    let mut i = 0usize;

    // Flush the verbatim slice s[copy_start..i] into `out`.
    macro_rules! flush {
        () => {
            if copy_start < i {
                out.push_str(&s[copy_start..i]);
            }
        };
    }

    // Byte index of the most recent `\` seen inside a string (used to flush
    // up-to-but-not-including it when we discover the escape is invalid).
    let mut backslash_pos = 0usize;

    while i < bytes.len() {
        let b = bytes[i];

        if pending_backslash {
            pending_backslash = false;
            // b is the character after `\` inside a string.
            if matches!(
                b,
                b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't' | b'u'
            ) {
                // Valid JSON escape — leave verbatim.
                i += 1;
                continue;
            } else {
                // Invalid escape: `\X` → `\\X`.
                // Flush verbatim span up to (not including) the backslash,
                // emit `\\`, then let X be included in the next verbatim span.
                let save_i = i;
                i = backslash_pos;
                flush!();
                out.push_str("\\\\");
                i = save_i;
                copy_start = i; // X will be included in the next verbatim span
                i += 1;
                continue;
            }
        }

        if in_string {
            match b {
                b'\\' => {
                    backslash_pos = i;
                    pending_backslash = true;
                    i += 1;
                    continue;
                }
                b'"' => {
                    in_string = false;
                    i += 1;
                    continue;
                }
                0x00..=0x1F => {
                    // Bare control character — flush verbatim span, then replace.
                    flush!();
                    match b {
                        b'\n' => out.push_str("\\n"),
                        b'\r' => out.push_str("\\r"),
                        b'\t' => out.push_str("\\t"),
                        other => out.push_str(&format!("\\u{:04x}", other)),
                    }
                    i += 1;
                    copy_start = i;
                    continue;
                }
                _ => {
                    // Multi-byte UTF-8 continuation bytes are >= 0x80, so they
                    // are >= 0x20 and fall here — left untouched in the verbatim
                    // slice.
                    i += 1;
                    continue;
                }
            }
        } else {
            match b {
                b'"' => {
                    in_string = true;
                    i += 1;
                    continue;
                }
                0x00..=0x1F => {
                    // Control char outside strings — drop it.
                    flush!();
                    i += 1;
                    copy_start = i;
                    continue;
                }
                _ => {
                    i += 1;
                    continue;
                }
            }
        }
    }

    // Flush any remaining verbatim tail.
    flush!();

    std::borrow::Cow::Owned(out)
}

/// Parse and validate a JSON response from the model.
///
/// Steps:
/// 1. Strip `[lang-fallback]` prefix (emits no warning here — caller decides).
/// 2. Strip markdown code fences.
/// 3. Escape bare control characters that local models sometimes emit inside
///    JSON string values without proper escaping.
/// 4. Parse as JSON; on failure try extracting the first balanced JSON value
///    (tolerates leading prose and trailing junk from the model).
/// 5. Verify every field in `required_fields` is present and non-null.
///
/// # Errors
/// Returns `LxError::LogicalError` with message
/// `"model returned invalid response: <reason>"` on any failure.
pub fn validate_json(response: &str, required_fields: &[&str]) -> Result<Value, LxError> {
    let (cleaned, _was_fallback) = strip_lang_fallback(response);
    let cleaned = strip_code_fences(&cleaned);
    let sanitized = escape_control_chars(cleaned);
    let cleaned = sanitized.as_ref();

    let value: Value = match serde_json::from_str::<Value>(cleaned) {
        Ok(v) => v,
        Err(first_err) => {
            // Fallback 1: extract first balanced value to tolerate preamble /
            // trailing text.
            if let Some(v) = extract_json_value(cleaned)
                .and_then(|slice| serde_json::from_str::<Value>(slice).ok())
            {
                v
            } else if first_err.is_eof() {
                // Fallback 2: the response was truncated (model hit max_tokens
                // mid-value). Salvage the largest valid prefix rather than
                // failing outright — the user gets most of their result.
                match salvage_truncated_json(cleaned)
                    .and_then(|s| serde_json::from_str::<Value>(&s).ok())
                {
                    Some(v) => {
                        eprintln!(
                            "[lx-llm] warning: response truncated at max_tokens; recovered partial result (some items dropped). Raise the tool's limit or narrow the input for complete output."
                        );
                        v
                    }
                    None => {
                        return Err(LxError::LogicalError(format!(
                            "model returned invalid response: failed to parse JSON: {first_err}\n  hint: try --verbose for request diagnostics\n  hint: response may have been truncated; the tool's max_tokens may be too low"
                        )));
                    }
                }
            } else {
                return Err(LxError::LogicalError(format!(
                    "model returned invalid response: failed to parse JSON: {first_err}\n  hint: try --verbose for request diagnostics"
                )));
            }
        }
    };

    for field in required_fields {
        match value.get(field) {
            None => {
                return Err(LxError::LogicalError(format!(
                    "model returned invalid response: missing required field \"{field}\"\n  hint: try --verbose for request diagnostics"
                )));
            }
            Some(Value::Null) => {
                return Err(LxError::LogicalError(format!(
                    "model returned invalid response: required field \"{field}\" is null\n  hint: try --verbose for request diagnostics"
                )));
            }
            Some(_) => {}
        }
    }

    Ok(value)
}

/// Parse and deserialize a JSON response from the model into a typed struct.
///
/// Convenience wrapper used by all tool scaffolds:
/// ```ignore
/// let out = parse_response::<MyOutput>(&resp.content)?;
/// ```
/// Strips code fences and `[lang-fallback]` prefix before deserializing.
pub fn parse_response<T: serde::de::DeserializeOwned>(response: &str) -> Result<T, LxError> {
    let value = validate_json(response, &[])?;
    serde_json::from_value(value).map_err(|e| {
        LxError::LogicalError(format!(
            "model returned invalid response: schema mismatch: {e}\n  hint: try --verbose for request diagnostics"
        ))
    })
}

/// Extract plain text from a model response.
///
/// For tools that expect prose rather than JSON: strips leading/trailing
/// whitespace and the `[lang-fallback]` prefix if present.
pub fn extract_text(response: &str) -> String {
    let (text, _) = strip_lang_fallback(response.trim());
    text.trim().to_string()
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_json_clean() {
        let json = r#"{"summary":"hello","details":["a","b"]}"#;
        let v = validate_json(json, &["summary", "details"]).unwrap();
        assert_eq!(v["summary"], "hello");
    }

    #[test]
    fn validate_json_strips_fences() {
        let json = "```json\n{\"answer\":\"42\"}\n```";
        let v = validate_json(json, &["answer"]).unwrap();
        assert_eq!(v["answer"], "42");
    }

    #[test]
    fn validate_json_strips_plain_fences() {
        let json = "```\n{\"answer\":\"42\"}\n```";
        let v = validate_json(json, &["answer"]).unwrap();
        assert_eq!(v["answer"], "42");
    }

    #[test]
    fn validate_json_strips_lang_fallback() {
        let json = r#"[lang-fallback] {"field":"value"}"#;
        let v = validate_json(json, &["field"]).unwrap();
        assert_eq!(v["field"], "value");
    }

    #[test]
    fn validate_json_missing_field_errors() {
        let json = r#"{"summary":"hello"}"#;
        let err = validate_json(json, &["summary", "details"]).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("missing required field"), "got: {msg}");
        assert!(msg.contains("details"), "got: {msg}");
    }

    #[test]
    fn validate_json_null_field_errors() {
        let json = r#"{"summary":null}"#;
        let err = validate_json(json, &["summary"]).unwrap_err();
        assert!(err.to_string().contains("null"));
    }

    #[test]
    fn validate_json_invalid_json_errors() {
        let err = validate_json("not json at all", &[]).unwrap_err();
        assert!(err.to_string().contains("failed to parse JSON"));
    }

    #[test]
    fn validate_json_no_required_fields() {
        let json = r#"{"anything":1}"#;
        validate_json(json, &[]).unwrap();
    }

    // ── Robustness: trailing/leading junk ────────────────────────────────────

    #[test]
    fn validate_json_trailing_newline() {
        let v = validate_json("{\"answer\":\"42\"}\n", &["answer"]).unwrap();
        assert_eq!(v["answer"], "42");
    }

    #[test]
    fn validate_json_trailing_prose() {
        let v = validate_json("{\"answer\":\"42\"}\nThat is the answer.", &["answer"]).unwrap();
        assert_eq!(v["answer"], "42");
    }

    #[test]
    fn validate_json_trailing_second_object() {
        let v = validate_json("{\"answer\":\"42\"}{\"x\":1}", &["answer"]).unwrap();
        assert_eq!(v["answer"], "42");
    }

    #[test]
    fn validate_json_leading_preamble() {
        let v = validate_json("Here is the JSON: {\"answer\":\"42\"}", &["answer"]).unwrap();
        assert_eq!(v["answer"], "42");
    }

    #[test]
    fn validate_json_preamble_and_trailing() {
        let v = validate_json("Sure!\n{\"answer\":\"42\"}\nDone.", &["answer"]).unwrap();
        assert_eq!(v["answer"], "42");
    }

    #[test]
    fn validate_json_fenced_with_trailing() {
        let input = "```json\n{\"answer\":\"42\"}\n```\nextra";
        let v = validate_json(input, &["answer"]).unwrap();
        assert_eq!(v["answer"], "42");
    }

    #[test]
    fn validate_json_array_root_trailing() {
        let v = validate_json("[1,2,3] trailing", &[]).unwrap();
        assert_eq!(v, serde_json::json!([1, 2, 3]));
    }

    #[test]
    fn validate_json_brace_in_string() {
        let v = validate_json(r#"{"answer":"a}b{c"}"#, &["answer"]).unwrap();
        assert_eq!(v["answer"], "a}b{c");
    }

    #[test]
    fn validate_json_escaped_quote_in_string() {
        let v = validate_json(r#"{"answer":"a\"b"}"#, &["answer"]).unwrap();
        assert_eq!(v["answer"], "a\"b");
    }

    #[test]
    fn validate_json_nested_mixed() {
        let v = validate_json(r#"{"a":[1,{"b":2}]} junk"#, &["a"]).unwrap();
        assert_eq!(v["a"][1]["b"], 2);
    }

    #[test]
    fn validate_json_truncated_scalar_still_errors() {
        // A truncated top-level object whose only member value is cut mid-string
        // has no salvageable complete member, so it still errors with the hint.
        let err = validate_json(r#"{"answer":"4"#, &[]).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("truncated"), "got: {msg}");
        assert!(msg.contains("max_tokens"), "got: {msg}");
    }

    // ── Truncation salvage ──────────────────────────────────────────────────

    #[test]
    fn salvage_truncated_array_of_objects() {
        // Model produced two complete rows then was cut mid-third.
        let input = r#"{"rows":[["a","b"],["c","d"],["e","#;
        let v = validate_json(input, &[]).unwrap();
        let rows = v["rows"].as_array().unwrap();
        assert_eq!(rows.len(), 2, "should keep the two complete rows: {v}");
        assert_eq!(rows[0], serde_json::json!(["a", "b"]));
        assert_eq!(rows[1], serde_json::json!(["c", "d"]));
    }

    #[test]
    fn salvage_truncated_object_members() {
        // Cut after a complete member, with a dangling comma.
        let input = r#"{"a":"x","b":"y","#;
        let v = validate_json(input, &[]).unwrap();
        assert_eq!(v["a"], "x");
        assert_eq!(v["b"], "y");
    }

    #[test]
    fn salvage_truncated_string_array() {
        let input = r#"{"paths":["one","two","thr"#;
        let v = validate_json(input, &[]).unwrap();
        let paths = v["paths"].as_array().unwrap();
        assert_eq!(paths.len(), 2, "drop the incomplete third string: {v}");
    }

    #[test]
    fn salvage_does_not_fire_on_balanced_json() {
        // Balanced input must take the normal path, not salvage.
        let v = validate_json(r#"{"rows":[["a","b"]]}"#, &[]).unwrap();
        assert_eq!(v["rows"][0], serde_json::json!(["a", "b"]));
    }

    #[test]
    fn salvage_top_level_array() {
        // Root is an array; third element cut mid-object.
        let input = r#"[{"a":1},{"a":2},{"a":"#;
        let v = validate_json(input, &[]).unwrap();
        let arr = v.as_array().unwrap();
        assert_eq!(arr.len(), 2, "drop the partial third element: {v}");
        assert_eq!(arr[0]["a"], 1);
        assert_eq!(arr[1]["a"], 2);
    }

    #[test]
    fn salvage_lxtable_shape() {
        // Realistic lxtable output: columns + rows, truncated mid-row.
        let input = r#"{"columns":["name","kind"],"rows":[["lxsum","tier1"],["lxgrep","ti"#;
        let v = validate_json(input, &[]).unwrap();
        assert_eq!(v["columns"], serde_json::json!(["name", "kind"]));
        let rows = v["rows"].as_array().unwrap();
        assert_eq!(rows.len(), 1, "keep only the one complete row: {v}");
        assert_eq!(rows[0], serde_json::json!(["lxsum", "tier1"]));
    }

    #[test]
    fn salvage_object_with_array_keeps_array_siblings() {
        // The array member completes some rows; a later scalar member is cut.
        // Salvage targets the array (outermost collection) and drops the rest.
        let input = r#"{"rows":[["a"],["b"]],"note":"incomp"#;
        let v = validate_json(input, &[]).unwrap();
        let rows = v["rows"].as_array().unwrap();
        assert_eq!(rows.len(), 2, "{v}");
    }

    #[test]
    fn validate_json_required_field_after_extraction() {
        // Extraction works, then required-field check runs.
        let v = validate_json("{\"summary\":\"x\"}\nprose", &["summary"]).unwrap();
        assert_eq!(v["summary"], "x");
    }

    #[test]
    fn validate_json_garbage_still_errors() {
        let err = validate_json("not json at all", &[]).unwrap_err();
        assert!(
            err.to_string().contains("failed to parse JSON"),
            "regression: {err}"
        );
    }

    // ── Control-character sanitization ──────────────────────────────────────

    #[test]
    fn validate_json_bare_newline_in_string() {
        // Qwen-style: literal newline inside a JSON string value.
        let input = "{\"text\":\"line one\nline two\"}";
        let v = validate_json(input, &["text"]).unwrap();
        assert_eq!(v["text"], "line one\nline two");
    }

    #[test]
    fn validate_json_bare_cr_in_string() {
        let input = "{\"text\":\"a\rb\"}";
        let v = validate_json(input, &["text"]).unwrap();
        assert_eq!(v["text"], "a\rb");
    }

    #[test]
    fn validate_json_bare_tab_in_string() {
        let input = "{\"text\":\"a\tb\"}";
        let v = validate_json(input, &["text"]).unwrap();
        assert_eq!(v["text"], "a\tb");
    }

    #[test]
    fn validate_json_control_char_outside_string_dropped() {
        // Control chars outside strings are harmless and should be skipped.
        let input = "{\x00\"key\":\"val\"}";
        let v = validate_json(input, &["key"]).unwrap();
        assert_eq!(v["key"], "val");
    }

    #[test]
    fn validate_json_already_escaped_newline_untouched() {
        // A properly escaped \n must not be double-escaped.
        let input = r#"{"text":"line one\nline two"}"#;
        let v = validate_json(input, &["text"]).unwrap();
        assert_eq!(v["text"], "line one\nline two");
    }

    #[test]
    fn validate_json_mixed_bare_and_escaped() {
        // Mix of bare newline and proper \n in the same string.
        let input = "{\"text\":\"a\nb\\nc\"}";
        let v = validate_json(input, &["text"]).unwrap();
        assert_eq!(v["text"], "a\nb\nc");
    }

    #[test]
    fn escape_control_chars_preserves_multibyte_utf8() {
        // A bare newline triggers the out-buffer; subsequent multi-byte UTF-8
        // chars (ü = U+00FC, € = U+20AC, ß = U+00DF) must not be double-encoded.
        let input = "{\"body\":\"line\none\nüber den wolken, spaß macht €\"}";
        let v = validate_json(input, &["body"]).unwrap();
        let body = v["body"].as_str().unwrap();
        assert!(body.contains('ü'), "ü was corrupted: {body}");
        assert!(body.contains('ß'), "ß was corrupted: {body}");
        assert!(body.contains('€'), "€ was corrupted: {body}");
    }

    #[test]
    fn escape_control_chars_pure_ascii_unchanged() {
        // No control chars — function must return Borrowed (no allocation).
        let input = r#"{"key":"hello world"}"#;
        let v = validate_json(input, &["key"]).unwrap();
        assert_eq!(v["key"], "hello world");
    }

    // ── Invalid backslash escape fixup ──────────────────────────────────────

    #[test]
    fn invalid_escape_awk_print_column() {
        // qwen emits: {"command":"awk '{if ($1==\"ERROR\") print $3}'"} where
        // the inner \$ or \1 is sometimes written as a raw \d or similar.
        // Simulate: \p (not a valid JSON escape).
        let input = r#"{"command":"awk '\p{3}'"}"#;
        let v = validate_json(input, &["command"]).unwrap();
        // The backslash should be preserved as a literal backslash-p.
        assert!(
            v["command"].as_str().unwrap().contains("\\p"),
            "got: {}",
            v["command"]
        );
    }

    #[test]
    fn invalid_escape_sed_digit_backreference() {
        // sed 's/\(foo\)/\1/' — qwen emits literal \1 without doubling.
        let input = "{\"command\":\"sed 's/\\1/bar/'\"}";
        let v = validate_json(input, &["command"]).unwrap();
        assert!(
            v["command"].as_str().unwrap().contains("\\1"),
            "got: {}",
            v["command"]
        );
    }

    #[test]
    fn invalid_escape_at_start_of_string() {
        // \d at position 1 (column 10 in JSON) — caught by early column errors.
        let input = r#"{"regex":"\d+"}"#;
        let v = validate_json(input, &["regex"]).unwrap();
        assert!(
            v["regex"].as_str().unwrap().contains("\\d"),
            "got: {}",
            v["regex"]
        );
    }

    #[test]
    fn valid_escapes_are_not_doubled() {
        // \n \t \r \" \\ \/ \b \f \uXXXX must be left untouched.
        let input = r#"{"text":"line1\nline2\ttab\\backslash\/slash\b\f\rA"}"#;
        let v = validate_json(input, &["text"]).unwrap();
        let t = v["text"].as_str().unwrap();
        // After serde round-trip these are the actual characters, not escape sequences.
        assert!(t.contains('\n'));
        assert!(t.contains('\t'));
        assert!(t.contains('\\'));
        assert!(t.contains('A')); // A = A
    }

    #[test]
    fn invalid_escape_mixed_with_valid() {
        // A string with both valid (\n) and invalid (\p) escapes.
        let input = r#"{"cmd":"echo\nhello\pworld"}"#;
        let v = validate_json(input, &["cmd"]).unwrap();
        let t = v["cmd"].as_str().unwrap();
        assert!(t.contains('\n'), "\\n must be a newline: {t}");
        assert!(t.contains("\\p"), "\\p must become literal \\p: {t}");
    }

    #[test]
    fn extract_text_strips_whitespace() {
        assert_eq!(extract_text("  hello world  "), "hello world");
    }

    #[test]
    fn extract_text_strips_fallback_prefix() {
        assert_eq!(extract_text("[lang-fallback] hello"), "hello");
    }

    #[test]
    fn extract_text_clean_passthrough() {
        assert_eq!(extract_text("hello"), "hello");
    }
}
