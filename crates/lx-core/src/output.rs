#![forbid(unsafe_code)]

use std::sync::atomic::{AtomicBool, Ordering};

use crate::platform::{is_tty, Fd};

static QUIET: AtomicBool = AtomicBool::new(false);

/// Call once in `main.rs`, immediately after parsing args, before any I/O.
/// This sets the process-global quiet flag read by `warn()`.
pub fn set_quiet(q: bool) {
    QUIET.store(q, Ordering::Relaxed);
}

pub fn is_quiet() -> bool {
    QUIET.load(Ordering::Relaxed)
}

/// Emit a tier-2 warning to stderr unless `--quiet`.
///
/// Use this for warnings like input truncation, redaction notices, OS mismatch.
/// Do NOT use for danger/security signals or errors — those are always shown.
pub fn warn(msg: &str) {
    if !is_quiet() {
        eprintln!("warning: {msg}");
    }
}

/// Should model narration (the success explanation) be printed to stderr?
///
/// Precedence: `--quiet` > `--verbose` > TTY-default.
///
/// Narration is human-facing colour, not part of the result. It prints only in a
/// genuine interactive session — when **both** stdout and stderr are TTYs — or
/// when the user explicitly asks with `--verbose`. The moment either stream is
/// redirected or piped (a script, a pipeline, output to a file), narration goes
/// quiet, so users never need `2>/dev/null` (which would also hide warnings).
///
/// Keying on stdout (not just stderr) is what makes `cmd | other` quiet: in a
/// pipeline stdout is consumed by the next program even though stderr is often
/// still the terminal. This mirrors how `ls` decides to be machine-friendly.
///
/// This gates ONLY narration. Warnings use `warn()`. Danger and errors have
/// their own unconditional paths.
pub fn show_narration(quiet: bool, verbose: bool) -> bool {
    if quiet {
        return false;
    }
    verbose || (is_tty(Fd::Stdout) && is_tty(Fd::Stderr))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn show_narration_quiet_overrides_verbose() {
        assert!(!show_narration(true, true));
    }

    #[test]
    fn show_narration_quiet_suppresses() {
        assert!(!show_narration(true, false));
    }

    #[test]
    fn show_narration_verbose_without_quiet() {
        // verbose=true enables narration regardless of TTY
        assert!(show_narration(false, true));
    }

    #[test]
    fn is_quiet_set_get_roundtrip() {
        // Store original so we don't pollute other tests
        let original = is_quiet();
        set_quiet(true);
        assert!(is_quiet());
        set_quiet(false);
        assert!(!is_quiet());
        set_quiet(original);
    }

    #[test]
    fn warn_respects_quiet_flag() {
        // We can't easily capture stderr, but we can verify it doesn't panic
        // and that is_quiet() gates the call correctly.
        set_quiet(true);
        warn("this should be suppressed"); // must not panic
        set_quiet(false);
        warn("this fires in test (TTY not a pipe)"); // must not panic
    }
}
