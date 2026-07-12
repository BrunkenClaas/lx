#![forbid(unsafe_code)]

use crate::catalog::{Category, ToolEntry, TOOLS};

/// Detect terminal width: reads COLUMNS env var, falls back to 80.
pub fn term_columns() -> usize {
    std::env::var("COLUMNS")
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .filter(|&w| w >= 40)
        .unwrap_or(80)
}

/// ANSI escape sequences used in TTY output.
mod ansi {
    pub const BOLD: &str = "\x1b[1m";
    pub const DIM: &str = "\x1b[2m";
    pub const RESET: &str = "\x1b[0m";
    pub const CYAN: &str = "\x1b[36m";
}

/// Find tools matching a category filter string (short id or display name substring,
/// case-insensitive). Returns all tools if `filter` is `None`.
pub fn filter_by_cat<'a>(filter: Option<&str>) -> Vec<&'a ToolEntry> {
    match filter {
        None => TOOLS.iter().collect(),
        Some(f) => {
            let f_lower = f.to_lowercase();
            TOOLS
                .iter()
                .filter(|e| {
                    e.category.short_id().contains(&f_lower as &str)
                        || e.category
                            .display_name()
                            .to_lowercase()
                            .contains(&f_lower as &str)
                })
                .collect()
        }
    }
}

/// Find tools matching a substring in name or purpose, case-insensitive.
pub fn filter_by_keyword<'a>(keyword: &str) -> Vec<&'a ToolEntry> {
    let kw = keyword.to_lowercase();
    TOOLS
        .iter()
        .filter(|e| e.name.contains(&kw as &str) || e.purpose.to_lowercase().contains(&kw as &str))
        .collect()
}

/// Render a flat hit list (used for keyword search results).
/// Shows the full purpose text so the user can judge relevance.
pub fn render_hits(tools: &[&ToolEntry], color: bool) -> String {
    if tools.is_empty() {
        return String::new();
    }
    let name_width = tools.iter().map(|e| e.name.len()).max().unwrap_or(0) + 2;
    let mut out = String::new();
    for e in tools {
        if color {
            out.push_str(&format!(
                "  {}{:<width$}{}  {}{}{}\n",
                ansi::CYAN,
                e.name,
                ansi::RESET,
                ansi::DIM,
                e.purpose,
                ansi::RESET,
                width = name_width,
            ));
        } else {
            out.push_str(&format!(
                "  {:<width$}  {}\n",
                e.name,
                e.purpose,
                width = name_width
            ));
        }
    }
    out
}

/// Render a compact multi-column overview, grouped by category.
/// Columns contain `name  short` pairs; count is determined by terminal width.
pub fn render_grouped(tools: &[&ToolEntry], color: bool, width: usize) -> String {
    let mut out = String::new();

    // Determine column layout.
    // Each column: max_name_len + 2 + max_short_len + 4 (gap between columns).
    let max_name = tools.iter().map(|e| e.name.len()).max().unwrap_or(10);
    let max_short = tools.iter().map(|e| e.short.len()).max().unwrap_or(20);
    let col_width = max_name + 2 + max_short; // "lxcommit  commit message"
    let gap = 4;
    let n_cols = ((width + gap) / (col_width + gap)).max(1);

    let mut first_cat = true;
    for &cat in Category::all() {
        let cat_tools: Vec<&ToolEntry> = tools
            .iter()
            .copied()
            .filter(|e| e.category == cat)
            .collect();
        if cat_tools.is_empty() {
            continue;
        }
        if !first_cat {
            out.push('\n');
        }
        first_cat = false;

        if color {
            out.push_str(&format!(
                "{}{}{}\n",
                ansi::BOLD,
                cat.display_name(),
                ansi::RESET,
            ));
        } else {
            out.push_str(cat.display_name());
            out.push('\n');
        }

        for chunk in cat_tools.chunks(n_cols) {
            out.push_str("  ");
            for (i, e) in chunk.iter().enumerate() {
                let cell = format!(
                    "{:<name_w$}  {:<short_w$}",
                    e.name,
                    e.short,
                    name_w = max_name,
                    short_w = max_short
                );
                if color {
                    out.push_str(&format!(
                        "{}{:<name_w$}{}",
                        ansi::CYAN,
                        e.name,
                        ansi::RESET,
                        name_w = max_name
                    ));
                    out.push_str(&format!(
                        "  {}{:<short_w$}{}",
                        ansi::DIM,
                        e.short,
                        ansi::RESET,
                        short_w = max_short
                    ));
                } else {
                    out.push_str(&cell);
                }
                let is_last = i + 1 == chunk.len();
                if !is_last {
                    out.push_str(&" ".repeat(gap));
                }
            }
            out.push('\n');
        }
    }

    out
}

/// Render as JSON array of `{name, category, short, purpose}`.
pub fn render_json(tools: &[&ToolEntry]) -> String {
    serde_json::to_string_pretty(
        &tools
            .iter()
            .map(|e| {
                serde_json::json!({
                    "name": e.name,
                    "category": e.category.display_name(),
                    "short": e.short,
                    "purpose": e.purpose,
                })
            })
            .collect::<Vec<_>>(),
    )
    .unwrap_or_else(|_| "[]".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grouped_contains_all_categories() {
        let all: Vec<&ToolEntry> = TOOLS.iter().collect();
        let out = render_grouped(&all, false, 80);
        for cat in Category::all() {
            assert!(
                out.contains(cat.display_name()),
                "missing category: {}",
                cat.display_name()
            );
        }
    }

    #[test]
    fn keyword_filter_commit_finds_lxcommit() {
        let hits = filter_by_keyword("commit");
        assert!(hits.iter().any(|e| e.name == "lxcommit"));
    }

    #[test]
    fn keyword_filter_empty_string_returns_all() {
        let hits = filter_by_keyword("");
        assert_eq!(hits.len(), TOOLS.len());
    }

    #[test]
    fn keyword_filter_no_match_returns_empty() {
        let hits = filter_by_keyword("zzzznotfound9999");
        assert!(hits.is_empty());
    }

    #[test]
    fn cat_filter_code_returns_subset() {
        let hits = filter_by_cat(Some("code"));
        assert!(!hits.is_empty());
        assert!(hits.iter().all(|e| e.category == Category::CodeDevelopment));
    }

    #[test]
    fn hits_render_contains_purpose() {
        let hits = filter_by_keyword("commit");
        let out = render_hits(&hits, false);
        assert!(out.contains("Conventional Commit"));
    }

    #[test]
    fn json_render_is_valid() {
        let all: Vec<&ToolEntry> = TOOLS.iter().collect();
        let json = render_json(&all);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed.is_array());
        assert_eq!(parsed.as_array().unwrap().len(), 72);
    }
}
