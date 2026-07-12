/// Dangerous shell patterns that must be flagged on stderr before output.
///
/// This check is deterministic and local — never delegated to the LLM (§8.3).
static DANGEROUS_PATTERNS: &[(&str, &str)] = &[
    ("rm -rf /", "recursive deletion of root filesystem"),
    ("rm -rf ~", "recursive deletion of home directory"),
    ("rm -fr /", "recursive deletion of root filesystem"),
    ("dd of=/dev/", "direct write to block device"),
    ("mkfs", "filesystem creation — destroys existing data"),
    (":(){ :|:& };:", "fork bomb — will crash the system"),
    (":(){:|:&};:", "fork bomb variant"),
    ("curl | sh", "executing untrusted remote script"),
    ("curl|sh", "executing untrusted remote script"),
    ("wget | sh", "executing untrusted remote script"),
    ("wget|sh", "executing untrusted remote script"),
    ("curl | bash", "executing untrusted remote script"),
    ("curl|bash", "executing untrusted remote script"),
    (
        "iwr | iex",
        "executing untrusted remote script (PowerShell)",
    ),
    ("iwr|iex", "executing untrusted remote script (PowerShell)"),
    ("Invoke-Expression", "dynamic script execution (PowerShell)"),
    (
        "Remove-Item -Recurse /",
        "recursive deletion via PowerShell",
    ),
    (
        "Remove-Item -Recurse C:\\",
        "recursive deletion of system drive",
    ),
    ("DROP TABLE", "destructive SQL — permanently deletes table"),
    (
        "DROP DATABASE",
        "destructive SQL — permanently deletes database",
    ),
    (
        "DELETE FROM",
        "SQL deletion — may remove all rows if no WHERE clause",
    ),
    ("truncate table", "SQL truncation — removes all rows"),
    ("FORMAT C:", "formats the Windows system drive"),
    ("> /dev/sda", "direct write to disk device"),
    ("shred", "irreversible file shredding"),
];

/// A dangerous pattern found in a command: the matched pattern and why it is dangerous.
#[derive(Debug)]
pub struct Finding {
    pub pattern: &'static str,
    pub description: &'static str,
}

/// Scan the command for dangerous patterns. Pure — no I/O.
///
/// Returns every matched pattern. The caller (main.rs) is responsible for
/// emitting these as tier-3 danger warnings on stderr and for setting the exit
/// code; `run()` uses a non-empty result only to force `dangerous = true`.
pub fn check(command: &str) -> Vec<Finding> {
    let lower = command.to_lowercase();
    DANGEROUS_PATTERNS
        .iter()
        .filter(|(pattern, _)| lower.contains(&pattern.to_lowercase()))
        .map(|(pattern, description)| Finding {
            pattern,
            description,
        })
        .collect()
}

/// Emit danger findings on stderr (tier-3: always shown, never suppressed by --quiet).
///
/// Lives here rather than in main.rs so every entry point (normal run, --dry-run)
/// formats the warning identically.
pub fn warn_findings(findings: &[Finding]) {
    for f in findings {
        eprintln!(
            "⚠  DANGER: command contains '{}' — {}",
            f.pattern, f.description
        );
    }
    if !findings.is_empty() {
        eprintln!("   Review carefully before executing. This command was NOT run.");
    }
}
