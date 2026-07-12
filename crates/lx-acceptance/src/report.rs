//! Compact, CI-gateable report. Humans read only the FAILURES section.

use crate::engine::{Failure, Outcome};
use std::collections::BTreeMap;

/// One graded intent's result, kept for the report.
pub struct Row {
    pub tool: String,
    pub name: String,
    pub outcome: Outcome,
}

/// Accumulates results and renders the report.
#[derive(Default)]
pub struct Report {
    pub rows: Vec<Row>,
}

impl Report {
    pub fn push(&mut self, tool: &str, name: &str, outcome: Outcome) {
        self.rows.push(Row {
            tool: tool.to_string(),
            name: name.to_string(),
            outcome,
        });
    }

    pub fn pass_count(&self) -> usize {
        self.rows
            .iter()
            .filter(|r| matches!(r.outcome, Outcome::Pass))
            .count()
    }
    pub fn fail_count(&self) -> usize {
        self.rows
            .iter()
            .filter(|r| matches!(r.outcome, Outcome::Fail(_)))
            .count()
    }
    pub fn skip_count(&self) -> usize {
        self.rows
            .iter()
            .filter(|r| matches!(r.outcome, Outcome::Skipped(_)))
            .count()
    }

    /// Renders the human report to stdout. Returns the process exit code
    /// (1 if any intent failed, else 0; skips never fail).
    pub fn render(&self, header: &ReportHeader) -> i32 {
        println!("# LX Coreutils — Extended Acceptance Report");
        println!();
        println!("- Model: `{}`", header.model);
        println!("- Provider: `{}`", header.provider);
        println!("- Target OS: `{}`", header.target);
        println!("- Intents: {}", self.rows.len());
        println!();

        // Per-tool counts, sorted by tool name.
        let mut by_tool: BTreeMap<&str, (usize, usize, usize)> = BTreeMap::new();
        for r in &self.rows {
            let e = by_tool.entry(r.tool.as_str()).or_default();
            match &r.outcome {
                Outcome::Pass => e.0 += 1,
                Outcome::Fail(_) => e.1 += 1,
                Outcome::Skipped(_) => e.2 += 1,
            }
        }
        println!("## Per-tool");
        println!();
        for (tool, (p, f, s)) in &by_tool {
            let total = p + f + s;
            let mark = if *f > 0 {
                "❌"
            } else if *s > 0 {
                "⚠️ "
            } else {
                "✅"
            };
            let skipnote = if *s > 0 {
                format!(" ({s} skipped)")
            } else {
                String::new()
            };
            println!("- {mark} {tool}: {p}/{total} pass{skipnote}");
        }
        println!();

        // FAILURES — only failing intents.
        let failures: Vec<&Row> = self
            .rows
            .iter()
            .filter(|r| matches!(r.outcome, Outcome::Fail(_)))
            .collect();
        if !failures.is_empty() {
            println!("## FAILURES ({})", failures.len());
            println!();
            for r in &failures {
                println!("- **{} / {}**", r.tool, r.name);
                if let Outcome::Fail(fs) = &r.outcome {
                    for Failure { check_kind, detail } in fs {
                        println!("    [{check_kind}] {detail}");
                    }
                }
            }
            println!();
        }

        // Skipped — informational.
        let skips: Vec<&Row> = self
            .rows
            .iter()
            .filter(|r| matches!(r.outcome, Outcome::Skipped(_)))
            .collect();
        if !skips.is_empty() {
            println!("## Skipped ({})", skips.len());
            println!();
            for r in &skips {
                if let Outcome::Skipped(reason) = &r.outcome {
                    println!("- {} / {} — {reason}", r.tool, r.name);
                }
            }
            println!();
        }

        let (p, f, s) = (self.pass_count(), self.fail_count(), self.skip_count());
        let verdict = if f > 0 { "FAIL" } else { "OK" };
        println!("TOTAL: {p} pass, {f} fail, {s} skip  ->  {verdict}");

        if f > 0 {
            1
        } else {
            0
        }
    }
}

/// Run metadata for the report header.
pub struct ReportHeader {
    pub model: String,
    pub provider: String,
    pub target: String,
}
