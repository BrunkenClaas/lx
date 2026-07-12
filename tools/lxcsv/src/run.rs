#![forbid(unsafe_code)]

use lx_config::Config;
use lx_core::error::LxError;
use lx_llm::{inject_lang, parse_response, LlmClient, Request};
use lx_redact::{redact, RedactLevel};
use serde::{Deserialize, Serialize};

pub const SYSTEM_TEMPLATE: &str = include_str!("../prompts/system.txt");

/// Max tokens for the answer — answers are concise.
const MAX_TOKENS: u32 = 512;

/// Maximum number of sample rows sent to the LLM.
const MAX_SAMPLE_ROWS: usize = 50;

/// Output of `lxcsv`.
#[derive(Debug, Serialize, Deserialize)]
pub struct Output {
    /// The answer to the user's question about the CSV data.
    pub answer: String,
    /// Description of how many rows were used in the analysis.
    pub used_rows: String,
}

impl Output {
    /// Render the plain result — the answer is the result (stdout only).
    pub fn to_plain(&self) -> String {
        self.answer.clone()
    }
}

/// Column statistics for numeric columns.
struct ColStats {
    name: String,
    min: f64,
    max: f64,
    sum: f64,
    count: u64,
}

impl ColStats {
    fn mean(&self) -> f64 {
        if self.count == 0 {
            0.0
        } else {
            self.sum / self.count as f64
        }
    }
}

/// Parse a CSV string into headers and rows.
///
/// Returns (headers, rows) where each row is a Vec<String>.
/// Handles quoted fields with the basic RFC 4180 rule (double-quote escaping).
fn parse_csv(content: &str) -> (Vec<String>, Vec<Vec<String>>) {
    let mut lines = content.lines().filter(|l| !l.trim().is_empty());

    let header_line = match lines.next() {
        Some(h) => h,
        None => return (vec![], vec![]),
    };

    let headers = split_csv_line(header_line);
    let rows: Vec<Vec<String>> = lines.map(split_csv_line).collect();

    (headers, rows)
}

/// Split a single CSV line respecting double-quoted fields.
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

/// Compute per-column statistics for numeric columns.
fn compute_stats(headers: &[String], rows: &[Vec<String>]) -> Vec<ColStats> {
    let ncols = headers.len();
    let mut stats: Vec<Option<ColStats>> = (0..ncols).map(|_| None).collect();

    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            if i >= ncols {
                break;
            }
            if let Ok(v) = cell.parse::<f64>() {
                match &mut stats[i] {
                    Some(s) => {
                        if v < s.min {
                            s.min = v;
                        }
                        if v > s.max {
                            s.max = v;
                        }
                        s.sum += v;
                        s.count += 1;
                    }
                    None => {
                        stats[i] = Some(ColStats {
                            name: headers[i].clone(),
                            min: v,
                            max: v,
                            sum: v,
                            count: 1,
                        });
                    }
                }
            }
        }
    }

    stats.into_iter().flatten().collect()
}

/// Compute per-group totals for numeric columns, grouped by a single categorical column.
///
/// Returns a Vec of (group_value, Vec<(col_name, total)>) for the first categorical
/// column with 2–20 distinct values (a reasonable group-by candidate).
fn compute_group_totals(
    headers: &[String],
    rows: &[Vec<String>],
) -> Vec<(String, Vec<(String, f64)>)> {
    let ncols = headers.len();
    if ncols < 2 || rows.is_empty() {
        return vec![];
    }

    // Identify which columns are numeric (> 50 % parseable as f64).
    let numeric: Vec<bool> = (0..ncols)
        .map(|i| {
            let parseable = rows
                .iter()
                .filter(|r| i < r.len() && r[i].parse::<f64>().is_ok())
                .count();
            parseable * 2 > rows.len()
        })
        .collect();

    // Find the first categorical column with 2–20 distinct values.
    let group_col = (0..ncols).find(|&i| {
        if numeric[i] {
            return false;
        }
        let distinct: std::collections::HashSet<&str> = rows
            .iter()
            .filter_map(|r| r.get(i).map(|s| s.as_str()))
            .collect();
        let d = distinct.len();
        (2..=20).contains(&d)
    });

    let group_col = match group_col {
        Some(c) => c,
        None => return vec![],
    };

    // Numeric column indices (excluding the group column).
    let num_cols: Vec<usize> = (0..ncols)
        .filter(|&i| i != group_col && numeric[i])
        .collect();
    if num_cols.is_empty() {
        return vec![];
    }

    // Accumulate sums per group.
    let mut totals: std::collections::BTreeMap<String, Vec<f64>> =
        std::collections::BTreeMap::new();
    for row in rows {
        let group_val = row.get(group_col).cloned().unwrap_or_default();
        let entry = totals
            .entry(group_val)
            .or_insert_with(|| vec![0.0; num_cols.len()]);
        for (ti, &ci) in num_cols.iter().enumerate() {
            if let Some(v) = row.get(ci).and_then(|s| s.parse::<f64>().ok()) {
                entry[ti] += v;
            }
        }
    }

    totals
        .into_iter()
        .map(|(group, sums)| {
            let col_sums: Vec<(String, f64)> = num_cols
                .iter()
                .enumerate()
                .map(|(ti, &ci)| (headers[ci].clone(), sums[ti]))
                .collect();
            (group, col_sums)
        })
        .collect()
}

/// Build the user message for the LLM from CSV data + question.
///
/// Returns (user_message, used_rows_description).
/// Only sends aggregates/sample rows — not the entire dataset.
fn build_user_message(
    headers: &[String],
    rows: &[Vec<String>],
    question: &str,
) -> (String, String) {
    let total_rows = rows.len();
    let sample_count = total_rows.min(MAX_SAMPLE_ROWS);
    let sample = &rows[..sample_count];

    let col_names = headers.join(", ");
    let stats = compute_stats(headers, rows);
    let group_totals = compute_group_totals(headers, rows);

    let mut msg = String::new();
    msg.push_str(&format!("COLUMNS: {}\n", col_names));
    msg.push_str(&format!("ROW_COUNT: {}\n", total_rows));
    msg.push_str(&format!("SAMPLE ({} rows):\n", sample_count));
    for row in sample {
        msg.push_str(&row.join(","));
        msg.push('\n');
    }

    if !stats.is_empty() {
        msg.push_str("STATS: ");
        let stat_strs: Vec<String> = stats
            .iter()
            .map(|s| {
                format!(
                    "{} min={:.1} max={:.1} mean={:.1}",
                    s.name,
                    s.min,
                    s.max,
                    s.mean()
                )
            })
            .collect();
        msg.push_str(&stat_strs.join("; "));
        msg.push('\n');
    }

    if !group_totals.is_empty() {
        msg.push_str("GROUP_TOTALS:\n");
        for (group, col_sums) in &group_totals {
            let pairs: Vec<String> = col_sums
                .iter()
                .map(|(col, sum)| format!("{col}={sum:.0}"))
                .collect();
            msg.push_str(&format!("  {group}: {}\n", pairs.join(", ")));
        }
    }

    msg.push_str(&format!("QUESTION: {}\n", question.trim()));

    let used_rows = if sample_count == total_rows {
        format!("all {} rows used", total_rows)
    } else {
        format!("{} of {} rows sampled", sample_count, total_rows)
    };

    (msg, used_rows)
}

/// Core logic for lxcsv — with mandatory redaction (SEC: redact).
///
/// Parses the CSV, computes local statistics, sends only aggregates/sample
/// rows to the LLM (not the full dataset).
pub fn run(
    csv_content: &str,
    question: &str,
    config: &Config,
    client: &dyn LlmClient,
) -> Result<Output, LxError> {
    if csv_content.trim().is_empty() {
        return Err(LxError::BadUsage("no CSV data provided".to_string()));
    }
    if question.trim().is_empty() {
        return Err(LxError::BadUsage("no question provided".to_string()));
    }

    // MANDATORY: redact before LLM. §8.1 — CSV data may contain secrets/PII.
    let level = RedactLevel::parse(&config.redact.level);
    let redacted = redact(csv_content, level)
        .map_err(|e| LxError::SecurityAbort(format!("redaction failed: {e}")))?;

    run_with_content(&redacted, question, config, client)
}

/// Variant used when `--no-redact` is passed by the user.
pub fn run_no_redact(
    csv_content: &str,
    question: &str,
    config: &Config,
    client: &dyn LlmClient,
) -> Result<Output, LxError> {
    if csv_content.trim().is_empty() {
        return Err(LxError::BadUsage("no CSV data provided".to_string()));
    }
    if question.trim().is_empty() {
        return Err(LxError::BadUsage("no question provided".to_string()));
    }

    run_with_content(csv_content, question, config, client)
}

/// Internal: parse CSV, compute stats, send to LLM.
fn run_with_content(
    csv_content: &str,
    question: &str,
    config: &Config,
    client: &dyn LlmClient,
) -> Result<Output, LxError> {
    let (headers, rows) = parse_csv(csv_content);

    if headers.is_empty() {
        return Err(LxError::BadUsage(
            "could not parse CSV headers from input".to_string(),
        ));
    }

    let (user_msg, used_rows_desc) = build_user_message(&headers, &rows, question);

    let system = inject_lang(SYSTEM_TEMPLATE, &config.output.lang);

    let req = Request {
        system: &system,
        user: &user_msg,
        max_tokens: MAX_TOKENS,
        temperature: 0.0,
        image: None,
    };

    let resp = client.complete(&req).map_err(LxError::from)?;

    let mut out = parse_response::<Output>(&resp.content)?;

    if out.answer.is_empty() {
        return Err(LxError::LogicalError(
            "model returned empty answer".to_string(),
        ));
    }

    // Override used_rows with our locally computed value if the model left it empty.
    if out.used_rows.is_empty() {
        out.used_rows = used_rows_desc;
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_csv_simple() {
        let (headers, rows) = parse_csv("name,age,city\nAlice,30,Berlin\nBob,25,Paris\n");
        assert_eq!(headers, vec!["name", "age", "city"]);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0], vec!["Alice", "30", "Berlin"]);
    }

    #[test]
    fn parse_csv_quoted_fields() {
        let (headers, rows) = parse_csv("name,note\n\"Smith, John\",\"has a comma\"\n");
        assert_eq!(headers, vec!["name", "note"]);
        assert_eq!(rows[0][0], "Smith, John");
        assert_eq!(rows[0][1], "has a comma");
    }

    #[test]
    fn parse_csv_empty_returns_empty() {
        let (headers, rows) = parse_csv("   \n  \n");
        assert!(headers.is_empty());
        assert!(rows.is_empty());
    }

    #[test]
    fn compute_stats_numeric() {
        let headers = vec!["x".to_string(), "y".to_string()];
        let rows = vec![
            vec!["1".to_string(), "10".to_string()],
            vec!["3".to_string(), "20".to_string()],
            vec!["5".to_string(), "30".to_string()],
        ];
        let stats = compute_stats(&headers, &rows);
        assert_eq!(stats.len(), 2);
        let x = stats.iter().find(|s| s.name == "x").unwrap();
        assert!((x.min - 1.0).abs() < 1e-9);
        assert!((x.max - 5.0).abs() < 1e-9);
        assert!((x.mean() - 3.0).abs() < 1e-9);
    }

    #[test]
    fn compute_stats_skips_non_numeric() {
        let headers = vec!["name".to_string(), "score".to_string()];
        let rows = vec![
            vec!["Alice".to_string(), "95".to_string()],
            vec!["Bob".to_string(), "80".to_string()],
        ];
        let stats = compute_stats(&headers, &rows);
        // Only "score" is numeric.
        assert_eq!(stats.len(), 1);
        assert_eq!(stats[0].name, "score");
    }

    #[test]
    fn build_user_message_includes_question() {
        let headers = vec!["a".to_string(), "b".to_string()];
        let rows = vec![vec!["1".to_string(), "2".to_string()]];
        let (msg, _) = build_user_message(&headers, &rows, "What is the sum of a?");
        assert!(msg.contains("QUESTION: What is the sum of a?"));
        assert!(msg.contains("COLUMNS: a, b"));
        assert!(msg.contains("ROW_COUNT: 1"));
    }

    #[test]
    fn group_totals_sales_csv_south_correct() {
        // South rows from the acceptance fixture sales.csv:
        //   South, Widget A: q1=18200, q2=16900, q3=17500, q4=19800  → total 72400
        //   South, Widget B: q1=22100, q2=24500, q3=23800, q4=26700  → total 97100
        //   South, Widget C: q1=5900,  q2=7100,  q3=8200,  q4=9800   → total 31000
        // Grand South total across all quarters and products = 200500
        // (Bob handles Widget A/B; Frank handles Widget C)
        let csv = "region,product,q1_sales,q2_sales,q3_sales,q4_sales,salesperson\n\
                   North,Widget A,12500,15200,14800,18900,alice\n\
                   North,Widget B,8900,9200,9500,11200,alice\n\
                   South,Widget A,18200,16900,17500,19800,bob\n\
                   South,Widget B,22100,24500,23800,26700,bob\n\
                   East,Widget A,9800,10200,11100,13400,charlie\n\
                   East,Widget B,5600,6100,6800,7900,charlie\n\
                   West,Widget A,16700,17800,19200,21400,dave\n\
                   West,Widget B,19200,20100,21500,23700,dave\n\
                   North,Widget C,3400,4200,5100,6800,eve\n\
                   South,Widget C,5900,7100,8200,9800,frank\n\
                   East,Widget C,2300,2800,3400,4200,grace\n\
                   West,Widget C,8900,10200,11800,13500,henry\n";

        let (headers, rows) = parse_csv(csv);
        let groups = compute_group_totals(&headers, &rows);

        let south = groups
            .iter()
            .find(|(g, _)| g == "South")
            .expect("South group");
        let south_total: f64 = south.1.iter().map(|(_, v)| v).sum();
        assert!(
            (south_total - 200500.0).abs() < 0.5,
            "South total should be 200500, got {south_total}"
        );
    }

    #[test]
    fn build_user_message_includes_group_totals() {
        let headers = vec!["region".to_string(), "sales".to_string()];
        let rows = vec![
            vec!["North".to_string(), "100".to_string()],
            vec!["South".to_string(), "200".to_string()],
            vec!["North".to_string(), "50".to_string()],
        ];
        let (msg, _) = build_user_message(&headers, &rows, "Highest region?");
        assert!(msg.contains("GROUP_TOTALS:"), "should include GROUP_TOTALS");
        assert!(msg.contains("North"), "should include North group");
        assert!(msg.contains("South"), "should include South group");
    }
}
