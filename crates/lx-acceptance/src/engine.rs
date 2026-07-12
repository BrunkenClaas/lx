//! Assertion engine: run one intent against the release binary and grade it.

use lx_testkit::binary::BinaryUnderTest;
use serde_json::Value;

use crate::fieldmap;
use crate::intents::Intent;
use crate::oracle::{self, OracleResult};

/// One failed check within an intent.
pub struct Failure {
    pub check_kind: String,
    pub detail: String,
}

/// Result of grading one intent.
pub enum Outcome {
    Pass,
    Fail(Vec<Failure>),
    /// Oracle-driven skip (e.g. external binary missing). Counted separately;
    /// never fails the run.
    Skipped(String),
}

/// Grades one intent end-to-end.
pub fn evaluate(intent: &Intent) -> Outcome {
    let bin = BinaryUnderTest::for_tool_release(&intent.tool);

    // Build args: --json, then extra flags, then the positional intent text.
    let mut args: Vec<&str> = vec!["--json"];
    for a in &intent.args {
        args.push(a.as_str());
    }
    if let Some(arg) = &intent.arg {
        if !arg.is_empty() {
            args.push(arg.as_str());
        }
    }

    let out = match &intent.stdin {
        Some(rel) => match oracle::read_fixture(rel) {
            Ok(data) => bin.run_with_stdin(&args, &data),
            Err(e) => {
                return Outcome::Fail(vec![Failure {
                    check_kind: "stdin".into(),
                    detail: format!("fixture '{rel}' unreadable: {e}"),
                }])
            }
        },
        // No stdin: closed stdin (empty string) so stateful tools don't block.
        None => bin.run_with_stdin(&args, ""),
    };

    let mut fails: Vec<Failure> = Vec::new();

    // 1. Exit code. expect_dangerous=true implies exit 3 unless overridden.
    let expected_exit = effective_expected_exit(intent);
    if out.exit_code != expected_exit {
        fails.push(Failure {
            check_kind: "exit".into(),
            detail: format!(
                "expected exit {expected_exit}, got {}\nstderr: {}",
                out.exit_code,
                out.stderr.trim()
            ),
        });
    }

    // 2. Valid JSON. If stdout isn't JSON we can't run field checks; bail early
    //    with this single failure (but keep the exit failure if present).
    let json: Value = match serde_json::from_str(&out.stdout) {
        Ok(v) => v,
        Err(e) => {
            fails.push(Failure {
                check_kind: "json".into(),
                detail: format!(
                    "stdout is not valid JSON: {e}\nstdout: {}",
                    truncate(&out.stdout)
                ),
            });
            return finish(fails);
        }
    };

    // 3. Danger field.
    if let Some(want) = intent.expect_dangerous {
        let field = fieldmap::danger_field(&intent.tool);
        match json.get(field).and_then(|v| v.as_bool()) {
            Some(got) if got == want => {}
            Some(got) => fails.push(Failure {
                check_kind: "dangerous".into(),
                detail: format!("expected {field}={want}, got {got}"),
            }),
            None => fails.push(Failure {
                check_kind: "dangerous".into(),
                detail: format!("danger field '{field}' missing or not a bool"),
            }),
        }
    }

    // 4. Resolve the checked field.
    let field_name = intent
        .json_field
        .clone()
        .or_else(|| fieldmap::main_field(&intent.tool).map(String::from));
    let field_value: Option<String> = match &field_name {
        Some(name) => match json.get(name) {
            Some(v) => Some(stringify(v)),
            None => {
                fails.push(Failure {
                    check_kind: "field".into(),
                    detail: format!("field '{name}' missing in output"),
                });
                None
            }
        },
        None => {
            // Only an error if a content check needs a field.
            if needs_field(intent) || intent.prose || intent.structural {
                fails.push(Failure {
                    check_kind: "field".into(),
                    detail: format!(
                        "no field map entry for tool '{}' and no json_field set",
                        intent.tool
                    ),
                });
            }
            None
        }
    };

    // Prose / structural intents: the deterministic guarantee is a present,
    // non-empty checked field.
    if intent.prose || intent.structural {
        match &field_value {
            Some(v) if v.trim().is_empty() || v.trim() == "\"\"" || v.trim() == "[]" => {
                fails.push(Failure {
                    check_kind: "non_empty".into(),
                    detail: format!("{} is empty", disp(&field_name)),
                });
            }
            _ => {}
        }
    }

    // 5/6. Substring + regex checks on the field value.
    if let Some(value) = &field_value {
        let hay = if intent.case_insensitive {
            value.to_lowercase()
        } else {
            value.clone()
        };
        for needle in &intent.must_contain {
            let n = if intent.case_insensitive {
                needle.to_lowercase()
            } else {
                needle.clone()
            };
            if !hay.contains(&n) {
                fails.push(Failure {
                    check_kind: "must_contain".into(),
                    detail: format!(
                        "{needle:?} NOT FOUND in {}={}",
                        disp(&field_name),
                        truncate(value)
                    ),
                });
            }
        }
        for needle in &intent.must_not_contain {
            let n = if intent.case_insensitive {
                needle.to_lowercase()
            } else {
                needle.clone()
            };
            if hay.contains(&n) {
                fails.push(Failure {
                    check_kind: "must_not_contain".into(),
                    detail: format!(
                        "{needle:?} FOUND in {}={}",
                        disp(&field_name),
                        truncate(value)
                    ),
                });
            }
        }
        if let Some(pat) = &intent.must_match {
            match regex::Regex::new(pat) {
                Ok(re) => {
                    if !re.is_match(value) {
                        fails.push(Failure {
                            check_kind: "must_match".into(),
                            detail: format!(
                                "/{pat}/ did not match {}={}",
                                disp(&field_name),
                                truncate(value)
                            ),
                        });
                    }
                }
                Err(e) => fails.push(Failure {
                    check_kind: "must_match".into(),
                    detail: format!("HARNESS ERROR: bad must_match regex /{pat}/: {e}"),
                }),
            }
        }

        // 7. Oracle.
        match oracle::run(intent, value) {
            OracleResult::NotApplicable | OracleResult::Pass => {}
            OracleResult::Fail(msgs) => {
                for m in msgs {
                    fails.push(Failure {
                        check_kind: "oracle".into(),
                        detail: m,
                    });
                }
            }
            OracleResult::Skipped(reason) => {
                // An oracle skip with no other failures => the whole intent is a
                // skip (e.g. jq missing). With other failures, the failures win.
                if fails.is_empty() {
                    return Outcome::Skipped(reason);
                }
            }
        }
    }

    finish(fails)
}

fn finish(fails: Vec<Failure>) -> Outcome {
    if fails.is_empty() {
        Outcome::Pass
    } else {
        Outcome::Fail(fails)
    }
}

/// expect_dangerous=true defaults the expected exit to 3 (the DANGEROUS code),
/// unless the intent explicitly set a non-default expect_exit.
fn effective_expected_exit(intent: &Intent) -> i32 {
    if intent.expect_dangerous == Some(true) && intent.expect_exit == 0 {
        3
    } else {
        intent.expect_exit
    }
}

fn needs_field(intent: &Intent) -> bool {
    !intent.must_contain.is_empty()
        || !intent.must_not_contain.is_empty()
        || intent.must_match.is_some()
        || intent.oracle != crate::intents::Oracle::None
}

/// Renders a JSON value as a string for substring matching. Strings are taken
/// verbatim; everything else is compact-serialized so checks against arrays
/// (e.g. lxcve `vulns`) still work on the raw JSON text.
fn stringify(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn disp(name: &Option<String>) -> String {
    match name {
        Some(n) => format!("field({n})"),
        None => "field(?)".into(),
    }
}

fn truncate(s: &str) -> String {
    const MAX: usize = 240;
    let trimmed = s.trim();
    if trimmed.chars().count() <= MAX {
        trimmed.to_string()
    } else {
        let cut: String = trimmed.chars().take(MAX).collect();
        format!("{cut}…")
    }
}
