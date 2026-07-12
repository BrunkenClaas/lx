use lx_config::Config;
use lx_core::error::LxError;
use lx_llm::{inject_lang, parse_response, LlmClient, Request};
use serde::{Deserialize, Serialize};

pub const SYSTEM_TEMPLATE: &str = include_str!("../prompts/system.txt");
const MAX_TOKENS: u32 = 512;
/// Max bar width in characters for the ASCII chart.
const BAR_WIDTH: usize = 20;

/// Internal struct for the LLM response.
#[derive(Debug, Deserialize)]
struct LlmOutput {
    /// Chart type suggested by the LLM (e.g. "bar", "line", "scatter").
    /// Stored for JSON passthrough; rendering always uses bar style for now.
    #[allow(dead_code)]
    chart_type: String,
    series: Vec<String>,
}

/// A single data point (optional label + numeric value).
#[derive(Debug, Clone)]
pub struct DataPoint {
    pub label: String,
    pub value: f64,
}

/// Output of `lxgraph`.
#[derive(Debug, Serialize, Deserialize)]
pub struct Output {
    /// The ASCII chart rendered locally.
    pub chart: String,
    /// Series labels suggested by the LLM.
    pub series: Vec<String>,
}

impl Output {
    /// Returns the chart text for plain stdout.
    pub fn to_plain(&self) -> &str {
        &self.chart
    }
}

/// Parse the input text into a list of data points.
///
/// Accepted formats:
///  - One number per line: `42`
///  - Label,value per line: `Sales Q1,1200`
///  - Multi-column CSV: `region,product,q1,q2,…` — uses first string field as
///    label and first numeric field as value; skips an all-string header row.
///  - Space-separated numbers on one line: `10 20 30`
pub fn parse_input(input: &str) -> Result<Vec<DataPoint>, LxError> {
    let mut points: Vec<DataPoint> = Vec::new();
    let mut first_data_line = true;

    for line in input.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if line.contains(',') {
            let fields: Vec<&str> = line.split(',').map(str::trim).collect();

            // Find the first numeric field.
            let numeric_idx = fields.iter().position(|f| f.parse::<f64>().is_ok());

            match numeric_idx {
                None => {
                    // All fields are non-numeric.
                    if first_data_line {
                        // Treat as a header row and skip.
                        first_data_line = false;
                        continue;
                    }
                    // Subsequent all-string lines are skipped silently.
                    continue;
                }
                Some(idx) => {
                    first_data_line = false;
                    // Build a label from string fields before the first numeric field.
                    let label = if idx == 0 {
                        format!("Value {}", points.len() + 1)
                    } else {
                        fields[..idx].join(" ")
                    };
                    let value: f64 = fields[idx].parse().unwrap();
                    points.push(DataPoint { label, value });
                }
            }
        } else {
            // Try space-separated or single number
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.is_empty() {
                continue;
            }
            first_data_line = false;
            for part in parts {
                match part.parse::<f64>() {
                    Ok(value) => {
                        let label = format!("Value {}", points.len() + 1);
                        points.push(DataPoint { label, value });
                    }
                    Err(_) => {
                        return Err(LxError::BadUsage(format!(
                            "cannot parse '{}' as a number",
                            part
                        )));
                    }
                }
            }
        }
    }

    if points.is_empty() {
        return Err(LxError::BadUsage(
            "no numeric data found; provide numbers (one per line or label,value pairs)"
                .to_string(),
        ));
    }

    Ok(points)
}

/// Render a bar chart from data points using block characters.
///
/// Uses the LLM-suggested labels if they match in count; falls back to
/// the parsed labels otherwise.
pub fn render_bar_chart(points: &[DataPoint], llm_series: &[String]) -> String {
    let max_val = points
        .iter()
        .map(|p| p.value)
        .fold(f64::NEG_INFINITY, f64::max);

    if max_val <= 0.0 {
        return "(no positive values to chart)".to_string();
    }

    // Use LLM labels if they match data point count; otherwise fall back.
    let labels: Vec<&str> = if llm_series.len() == points.len() {
        llm_series.iter().map(|s| s.as_str()).collect()
    } else {
        points.iter().map(|p| p.label.as_str()).collect()
    };

    // Compute label column width.
    let label_width = labels.iter().map(|l| l.len()).max().unwrap_or(0);

    let mut lines = Vec::new();
    for (i, point) in points.iter().enumerate() {
        let label = labels[i];
        let ratio = if max_val > 0.0 {
            (point.value / max_val).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let filled = (ratio * BAR_WIDTH as f64).round() as usize;
        let empty = BAR_WIDTH.saturating_sub(filled);

        let bar = format!("{}{}", "█".repeat(filled), "░".repeat(empty));
        // Format value: use integer display if the value is whole, else 2 d.p.
        let val_str = if point.value.fract() == 0.0 {
            format!("{:.0}", point.value)
        } else {
            format!("{:.2}", point.value)
        };

        lines.push(format!(
            "{:>label_width$} | {} {}",
            label,
            bar,
            val_str,
            label_width = label_width
        ));
    }

    lines.join("\n")
}

/// Core logic for `lxgraph`.
///
/// Parses numeric data locally, renders an ASCII chart locally, and calls
/// the LLM only to suggest axis labels and chart type. Pure: no I/O, no exit.
pub fn run(input: &str, config: &Config, client: &dyn LlmClient) -> Result<Output, LxError> {
    if input.trim().is_empty() {
        return Err(LxError::BadUsage(
            "no input provided; pipe numbers or label,value pairs into lxgraph".to_string(),
        ));
    }

    // Parse data locally — this never touches the network.
    let points = parse_input(input)?;

    // Ask the LLM for axis labels / chart type suggestion.
    let system = inject_lang(SYSTEM_TEMPLATE, &config.output.lang);

    let req = Request {
        system: &system,
        user: input.trim(),
        max_tokens: MAX_TOKENS,
        temperature: 0.0,
        image: None,
    };

    let resp = client
        .complete(&req)
        .map_err(lx_core::error::LxError::from)?;

    let llm_out = parse_response::<LlmOutput>(&resp.content)?;

    // Render chart locally using parsed data + LLM labels.
    let chart = render_bar_chart(&points, &llm_out.series);

    Ok(Output {
        chart,
        series: llm_out.series,
    })
}
