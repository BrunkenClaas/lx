/// Dangerous mount patterns that must be flagged on stderr before output.
///
/// This check is deterministic and local — never delegated to the LLM.
static DANGEROUS_PATTERNS: &[(&str, &str)] = &[
    ("mkfs", "filesystem formatting — destroys existing data"),
    ("--bind /", "bind-mount of root filesystem"),
    ("--rbind /", "recursive bind-mount of root filesystem"),
    ("mount / ", "mounting directly over root filesystem"),
    ("mount -o remount,rw /", "remounting root read-write"),
    (
        "-t proc",
        "mounting proc filesystem — kernel interface exposure",
    ),
    ("/dev/sda ", "operating on primary disk device (sda)"),
    ("/dev/nvme0n1 ", "operating on primary NVMe device"),
    ("format", "disk format operation"),
    ("dd if=", "raw disk copy"),
    ("dd of=", "raw disk write"),
];

/// Check the command for dangerous patterns and print warnings to stderr.
///
/// Returns `true` if any dangerous pattern was found.
pub fn check_and_warn(command: &str) -> bool {
    let lower = command.to_lowercase();
    let mut found_dangerous = false;

    for (pattern, description) in DANGEROUS_PATTERNS {
        if lower.contains(&pattern.to_lowercase()) {
            eprintln!(
                "⚠  DANGER: command contains '{}' — {}",
                pattern, description
            );
            found_dangerous = true;
        }
    }

    if found_dangerous {
        eprintln!("   Review carefully before executing. This command was NOT run.");
    }

    found_dangerous
}
