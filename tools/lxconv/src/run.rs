#![forbid(unsafe_code)]

use lx_config::Config;
use lx_core::error::LxError;
use lx_llm::{inject_lang, parse_response, LlmClient, Request};
use serde::{Deserialize, Serialize};

pub const SYSTEM_TEMPLATE: &str = include_str!("../prompts/system.txt");
/// Generous limit — converted output can be large (e.g. XML from dense JSON).
const MAX_TOKENS: u32 = 4096;

/// Supported target formats.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Format {
    Json,
    Csv,
    Yaml,
    Xml,
}

impl Format {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "json" => Some(Format::Json),
            "csv" => Some(Format::Csv),
            "yaml" | "yml" => Some(Format::Yaml),
            "xml" => Some(Format::Xml),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Format::Json => "json",
            Format::Csv => "csv",
            Format::Yaml => "yaml",
            Format::Xml => "xml",
        }
    }
}

/// Output of `lxconv`.
///
/// `content` contains the fully converted data as a string.
/// In plain mode this goes directly to stdout (pipe-safe passthrough).
#[derive(Debug, Serialize, Deserialize)]
pub struct Output {
    /// The converted data.
    pub content: String,
    /// How the conversion was performed: "local" or "llm".
    /// The system prompt only asks the model for `content`; `method` is set
    /// locally after parsing, so it must default when absent from the LLM reply
    /// (otherwise a correct `{"content": "..."}` response fails to deserialize).
    #[serde(default)]
    pub method: String,
}

impl Output {
    /// Return the converted data for plain (non-JSON) output.
    /// The result IS the converted data — pipe-safe.
    pub fn to_plain(&self) -> String {
        self.content.clone()
    }
}

// ── Local conversion helpers ──────────────────────────────────────────────────

/// Detect the likely format of the input heuristically.
fn detect_format(input: &str) -> Option<Format> {
    let t = input.trim();
    if t.starts_with('{') || t.starts_with('[') {
        return Some(Format::Json);
    }
    if t.starts_with("<?xml") || t.starts_with('<') {
        return Some(Format::Xml);
    }
    // YAML indicators: lines starting with "key: value" pattern.
    let yaml_like = t
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .take(3)
        .all(|l| l.contains(": ") || l.starts_with("- ") || l.starts_with("  "));
    if yaml_like && !t.contains(',') {
        return Some(Format::Yaml);
    }
    // CSV: presence of commas in the first line suggests CSV.
    if let Some(first) = t.lines().next() {
        if first.contains(',') {
            return Some(Format::Csv);
        }
    }
    None
}

/// Attempt JSON → CSV conversion locally.
///
/// Expects the JSON to be an array of objects with uniform keys.
/// Returns `None` if the shape is not a flat array of objects.
fn json_to_csv(input: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(input.trim()).ok()?;
    let arr = value.as_array()?;
    if arr.is_empty() {
        return Some(String::new());
    }

    // Collect all keys from all objects (preserving first-seen order).
    let mut headers: Vec<String> = Vec::new();
    for item in arr {
        if let Some(obj) = item.as_object() {
            for key in obj.keys() {
                if !headers.contains(key) {
                    headers.push(key.clone());
                }
            }
        } else {
            // Non-object element — cannot do flat CSV.
            return None;
        }
    }

    let mut out = String::new();
    // Header row.
    out.push_str(&csv_row(headers.iter().map(String::as_str)));
    // Data rows.
    for item in arr {
        let obj = item.as_object()?;
        let cells: Vec<&str> = headers
            .iter()
            .map(|h| obj.get(h).and_then(|v| v.as_str()).unwrap_or(""))
            .collect();
        // For non-string values use their JSON representation.
        let row_vals: Vec<String> = headers
            .iter()
            .map(|h| match obj.get(h) {
                Some(serde_json::Value::String(s)) => s.clone(),
                Some(v) => v.to_string(),
                None => String::new(),
            })
            .collect();
        let _ = cells;
        out.push_str(&csv_row(row_vals.iter().map(String::as_str)));
    }

    Some(out)
}

/// Format a sequence of string values as a single CSV line (with CRLF stripped, LF terminated).
fn csv_row<'a>(fields: impl Iterator<Item = &'a str>) -> String {
    let cells: Vec<String> = fields
        .map(|f| {
            if f.contains(',') || f.contains('"') || f.contains('\n') || f.contains('\r') {
                format!("\"{}\"", f.replace('"', "\"\""))
            } else {
                f.to_string()
            }
        })
        .collect();
    format!("{}\n", cells.join(","))
}

/// Attempt CSV → JSON conversion locally.
///
/// Produces a JSON array of objects.
fn csv_to_json(input: &str) -> Option<String> {
    let mut lines = input.lines().filter(|l| !l.trim().is_empty());
    let header_line = lines.next()?;
    let headers = split_csv_line(header_line);
    if headers.is_empty() {
        return None;
    }

    let mut records: Vec<serde_json::Value> = Vec::new();
    for line in lines {
        let fields = split_csv_line(line);
        let mut obj = serde_json::Map::new();
        for (i, h) in headers.iter().enumerate() {
            let val = fields.get(i).cloned().unwrap_or_default();
            // Try to coerce numeric values.
            let json_val = if let Ok(n) = val.parse::<i64>() {
                serde_json::Value::Number(n.into())
            } else if let Ok(f) = val.parse::<f64>() {
                serde_json::Number::from_f64(f)
                    .map(serde_json::Value::Number)
                    .unwrap_or_else(|| serde_json::Value::String(val.clone()))
            } else {
                serde_json::Value::String(val)
            };
            obj.insert(h.clone(), json_val);
        }
        records.push(serde_json::Value::Object(obj));
    }

    serde_json::to_string(&records).ok()
}

/// Split a single CSV line respecting RFC 4180 double-quoted fields.
fn split_csv_line(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '"' if in_quotes => {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    current.push('"');
                } else {
                    in_quotes = false;
                }
            }
            '"' => {
                in_quotes = true;
            }
            ',' if !in_quotes => {
                fields.push(current.trim().to_string());
                current = String::new();
            }
            other => {
                current.push(other);
            }
        }
    }
    fields.push(current.trim().to_string());
    fields
}

/// Try a purely local conversion. Returns `None` when the conversion requires
/// the LLM (unsupported pair, non-flat JSON, etc.).
fn try_local_convert(input: &str, target: &Format) -> Option<String> {
    let source = detect_format(input)?;

    match (&source, target) {
        // JSON passthrough (already the right format).
        (Format::Json, Format::Json) => {
            // Re-serialise to normalise whitespace.
            let v: serde_json::Value = serde_json::from_str(input.trim()).ok()?;
            Some(serde_json::to_string_pretty(&v).ok()?)
        }
        // CSV passthrough.
        (Format::Csv, Format::Csv) => Some(input.to_string()),
        // JSON → CSV.
        (Format::Json, Format::Csv) => json_to_csv(input),
        // CSV → JSON.
        (Format::Csv, Format::Json) => csv_to_json(input),
        // All other conversions delegate to LLM.
        _ => None,
    }
}

// ── Core function ─────────────────────────────────────────────────────────────

/// Core logic for `lxconv`.
///
/// Pure function: no I/O, no process::exit. Testable with MockLlmClient.
///
/// Strategy:
/// 1. Validate inputs.
/// 2. Attempt local conversion (JSON↔CSV, same-format passthrough).
/// 3. If local conversion is not applicable, delegate to the LLM.
pub fn run(
    input: &str,
    target: &Format,
    config: &Config,
    client: &dyn LlmClient,
) -> Result<Output, LxError> {
    if input.trim().is_empty() {
        return Err(LxError::BadUsage("no input provided".to_string()));
    }

    // Attempt local conversion first (fast, no network).
    if let Some(content) = try_local_convert(input, target) {
        return Ok(Output {
            content,
            method: "local".to_string(),
        });
    }

    // Fallback: ask the LLM.
    // Replace {format} placeholder first, then inject lang.
    let system_with_format = SYSTEM_TEMPLATE.replace("{format}", target.as_str());
    let system = inject_lang(&system_with_format, &config.output.lang);

    let user_msg = format!(
        "TARGET_FORMAT: {}\nDATA:\n{}",
        target.as_str(),
        input.trim()
    );

    let req = Request {
        system: &system,
        user: &user_msg,
        max_tokens: MAX_TOKENS,
        temperature: 0.0,
        image: None,
    };

    let resp = client.complete(&req).map_err(LxError::from)?;

    let out = parse_response::<Output>(&resp.content)?;

    if out.content.is_empty() {
        return Err(LxError::LogicalError(
            "model returned empty content".to_string(),
        ));
    }

    Ok(Output {
        content: out.content,
        method: "llm".to_string(),
    })
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_json_array() {
        assert_eq!(detect_format(r#"[{"x":1}]"#), Some(Format::Json));
    }

    #[test]
    fn detect_json_object() {
        assert_eq!(detect_format(r#"{"x":1}"#), Some(Format::Json));
    }

    #[test]
    fn detect_csv() {
        assert_eq!(detect_format("name,age\nAlice,30"), Some(Format::Csv));
    }

    #[test]
    fn json_to_csv_basic() {
        let json = r#"[{"name":"Alice","age":30},{"name":"Bob","age":25}]"#;
        let csv = json_to_csv(json).unwrap();
        // serde_json Map uses alphabetical key order, so "age" comes before "name"
        let header_line = csv.lines().next().unwrap();
        let headers: Vec<&str> = header_line.split(',').collect();
        assert!(
            headers.contains(&"name") && headers.contains(&"age"),
            "got: {csv}"
        );
        assert!(csv.contains("Alice"), "got: {csv}");
        assert!(csv.contains("Bob"), "got: {csv}");
    }

    #[test]
    fn json_to_csv_empty_array() {
        let result = json_to_csv("[]");
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "");
    }

    #[test]
    fn csv_to_json_basic() {
        let csv = "city,pop\nBerlin,3700000\nParis,2100000\n";
        let json_str = csv_to_json(csv).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        let arr = v.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["city"], "Berlin");
    }

    #[test]
    fn csv_row_with_comma_is_quoted() {
        let row = csv_row(["hello, world", "42"].iter().copied());
        assert!(row.starts_with('"'), "got: {row}");
    }

    #[test]
    fn split_csv_line_quoted() {
        let fields = split_csv_line(r#""Smith, John",30"#);
        assert_eq!(fields[0], "Smith, John");
        assert_eq!(fields[1], "30");
    }

    #[test]
    fn format_parse_roundtrip() {
        assert_eq!(Format::parse("json"), Some(Format::Json));
        assert_eq!(Format::parse("CSV"), Some(Format::Csv));
        assert_eq!(Format::parse("yml"), Some(Format::Yaml));
        assert_eq!(Format::parse("xml"), Some(Format::Xml));
        assert_eq!(Format::parse("toml"), None);
    }
}
