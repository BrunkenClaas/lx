/// Dangerous code patterns that must be flagged on stderr before output.
///
/// This check is deterministic and local — never delegated to the LLM (§8.3).
/// Covers shell, SQL, and script patterns that could cause irreversible damage.
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
    ("os.remove", "file deletion — verify path is correct"),
    ("shutil.rmtree", "recursive directory deletion (Python)"),
    ("fs.rmSync", "synchronous file/directory deletion (Node.js)"),
    ("fs.rmdirSync", "synchronous directory deletion (Node.js)"),
    ("FileUtils.rm_rf", "recursive deletion (Ruby)"),
];

/// Check generated code for dangerous patterns and print warnings to stderr.
///
/// Returns `true` if any dangerous pattern was found.
pub fn check_and_warn(code: &str) -> bool {
    let lower = code.to_lowercase();
    let mut found_dangerous = false;

    for (pattern, description) in DANGEROUS_PATTERNS {
        if lower.contains(&pattern.to_lowercase()) {
            eprintln!(
                "WARNING: generated code contains '{}' — {}",
                pattern, description
            );
            found_dangerous = true;
        }
    }

    if found_dangerous {
        eprintln!("   Review carefully before running. This code was NOT executed.");
    }

    found_dangerous
}
