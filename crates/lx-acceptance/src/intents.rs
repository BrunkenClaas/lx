//! Intents data file: schema and loader.
//!
//! Intents live in `intents/intents.toml` as an array of `[[intent]]` tables.
//! Each intent is one tool invocation plus the assertions that grade its output.
//! Assertions must be *necessary truths* a human has confirmed — never an LLM's
//! exact expected answer.

use serde::Deserialize;

/// Top-level TOML structure: `intent = [ { ... }, { ... } ]`.
#[derive(Debug, Deserialize)]
pub struct IntentsFile {
    #[serde(default)]
    pub intent: Vec<Intent>,
}

/// One graded tool invocation.
#[derive(Debug, Deserialize)]
pub struct Intent {
    /// Tool binary name, e.g. "lxdockercmd".
    pub tool: String,
    /// Unique-ish label for the report, e.g. "update-all-images-to-newest".
    pub name: String,
    /// Positional intent text passed as an argument. None for stdin-only tools.
    pub arg: Option<String>,
    /// Extra CLI flags, e.g. ["--headline"] or ["--target", "linux"].
    #[serde(default)]
    pub args: Vec<String>,
    /// Fixture path relative to `acceptance/fixtures`, fed on stdin.
    pub stdin: Option<String>,

    /// Substrings that must all appear in the checked field.
    #[serde(default)]
    pub must_contain: Vec<String>,
    /// Substrings that must not appear in the checked field.
    #[serde(default)]
    pub must_not_contain: Vec<String>,
    /// Regex that must match the checked field.
    pub must_match: Option<String>,
    /// Which JSON field to check. Defaults to the tool's main field (fieldmap).
    pub json_field: Option<String>,
    /// Expected process exit code. Defaults to 0.
    #[serde(default = "default_exit")]
    pub expect_exit: i32,
    /// Expected danger state. `Some(true)` => danger field true AND exit 3
    /// (unless `expect_exit` is explicitly overridden). `Some(false)` => danger
    /// field false. `None` => not checked.
    pub expect_dangerous: Option<bool>,
    /// Lowercase both sides for substring checks. Default false (case matters
    /// for commands, regexes, SQL).
    #[serde(default)]
    pub case_insensitive: bool,

    /// Execution oracle to run against the output (see oracle.rs).
    #[serde(default)]
    pub oracle: Oracle,
    /// For the regex/jq/sed oracles: inputs the output must accept/match.
    #[serde(default)]
    pub should_match: Vec<String>,
    /// For the regex oracle: inputs the pattern must NOT match.
    #[serde(default)]
    pub should_not_match: Vec<String>,
    /// Fixture (relative to acceptance/fixtures) fed to the jq/sed oracle on stdin.
    pub oracle_stdin: Option<String>,

    /// Marks a prose tool: structural-only grading, eligible for `--judge`.
    #[serde(default)]
    pub prose: bool,
    /// Marks a structured-output intent whose only deterministic guarantee is a
    /// present, non-empty checked field (e.g. an extraction returned *some*
    /// records). Like `prose` but NOT eligible for the prose `--judge` pass.
    #[serde(default)]
    pub structural: bool,
    /// Extended-only intent: skipped unless --extended is passed. Used for the
    /// 2nd/3rd intent of a class (N/P/J scale to 3 in extended mode).
    #[serde(default)]
    pub extended: bool,
    /// Deliberately re-uses (or closely paraphrases) a few-shot `Input:` line
    /// from the tool's `system.txt`. Exempts this intent from the few-shot
    /// contamination guard (`fewshot::guard`). Allowed ONLY for genuinely
    /// destructive regression cases where the canonical phrasing is the point of
    /// the test (e.g. lxdockercmd "update images" must mean pull, never prune).
    /// Every use MUST carry a comment in intents.toml explaining why.
    #[serde(default)]
    #[cfg_attr(not(test), allow(dead_code))]
    pub allow_fewshot_overlap: bool,
}

fn default_exit() -> i32 {
    0
}

/// Execution oracle kinds. `none` is structural assertions only.
#[derive(Debug, Deserialize, Default, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum Oracle {
    #[default]
    None,
    Regex,
    Json,
    Jq,
    Sed,
}

impl Intent {
    /// True if this intent carries at least one check. Intents with no checks
    /// are silent no-ops and are rejected at load time.
    ///
    /// A `prose` intent counts as a check on its own: the engine enforces the
    /// structural invariants (exit 0, valid JSON, the checked field present and
    /// non-empty) for it. That is the deepest deterministic grading possible for
    /// free-text output; `--judge` adds an advisory sanity-gate on top.
    pub fn has_checks(&self) -> bool {
        self.prose
            || self.structural
            || !self.must_contain.is_empty()
            || !self.must_not_contain.is_empty()
            || self.must_match.is_some()
            || self.expect_dangerous.is_some()
            || self.expect_exit != 0
            || self.oracle != Oracle::None
    }
}

/// Parses an intents TOML string, validating that every intent has checks.
pub fn parse(src: &str) -> Result<Vec<Intent>, String> {
    let file: IntentsFile =
        toml::from_str(src).map_err(|e| format!("invalid intents.toml: {e}"))?;
    for it in &file.intent {
        if !it.has_checks() {
            return Err(format!(
                "intent '{}/{}' has no assertions (would be a silent no-op)",
                it.tool, it.name
            ));
        }
    }
    Ok(file.intent)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_basic_intent() {
        let src = r#"
[[intent]]
tool = "lxdockercmd"
name = "update-all-images"
arg = "update all images to newest version"
must_contain = ["docker", "pull"]
must_not_contain = ["prune"]
expect_dangerous = false
"#;
        let v = parse(src).unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].tool, "lxdockercmd");
        assert_eq!(v[0].must_contain, vec!["docker", "pull"]);
        assert_eq!(v[0].expect_dangerous, Some(false));
    }

    #[test]
    fn rejects_checkless_intent() {
        let src = r#"
[[intent]]
tool = "lxsh"
name = "empty"
arg = "list files"
"#;
        assert!(parse(src).is_err());
    }

    #[test]
    fn parses_oracle_enum() {
        let src = r#"
[[intent]]
tool = "lxregex"
name = "iso-date"
arg = "match ISO 8601 dates"
oracle = "regex"
should_match = ["2024-01-15"]
should_not_match = ["nope"]
"#;
        let v = parse(src).unwrap();
        assert_eq!(v[0].oracle, Oracle::Regex);
    }
}
