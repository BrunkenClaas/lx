#![forbid(unsafe_code)]

use thiserror::Error;

// ── Exit code constants ───────────────────────────────────────────────────────

/// Process exited successfully.
pub const SUCCESS: i32 = 0;
/// Logical/semantic error (e.g. model returned "nothing found").
pub const LOGICAL_ERROR: i32 = 1;
/// Bad usage — wrong arguments, missing required input.
pub const BAD_USAGE: i32 = 2;
/// Dangerous output — tool produced output flagged as dangerous; use --allow-dangerous to suppress.
pub const DANGEROUS: i32 = 3;
/// Security abort — redaction failure, path escape, dangerous pattern detected.
pub const SECURITY_ABORT: i32 = 5;

// Spec-canonical aliases (EXIT_* prefix).
pub const EXIT_OK: i32 = SUCCESS;
pub const EXIT_ERROR: i32 = LOGICAL_ERROR;
pub const EXIT_USAGE: i32 = BAD_USAGE;
pub const EXIT_DANGEROUS: i32 = DANGEROUS;
pub const EXIT_SECURITY: i32 = SECURITY_ABORT;

// ── LxError ───────────────────────────────────────────────────────────────────

/// The unified error type for all lx tools.
/// Each variant maps to a specific exit code (see `docs/design_document.md §9.5`).
#[derive(Debug, Error)]
pub enum LxError {
    /// Exit 1 — expected logical failure (e.g. "no results found").
    #[error("{0}")]
    LogicalError(String),

    /// Exit 2 — caller supplied bad arguments or no input.
    #[error("{0}")]
    BadUsage(String),

    /// Exit 1 — missing API key or invalid configuration (maps to LOGICAL_ERROR).
    #[error("{0}")]
    ConfigAuth(String),

    /// Exit 1 — network / LLM error (maps to LOGICAL_ERROR).
    #[error("{0}")]
    NetworkLlm(String),

    /// Exit 5 — security abort (redaction failure, path escape, …).
    #[error("{0}")]
    SecurityAbort(String),
}

impl LxError {
    pub fn exit_code(&self) -> i32 {
        match self {
            LxError::LogicalError(_) => LOGICAL_ERROR,
            LxError::BadUsage(_) => BAD_USAGE,
            LxError::ConfigAuth(_) => LOGICAL_ERROR,
            LxError::NetworkLlm(_) => LOGICAL_ERROR,
            LxError::SecurityAbort(_) => SECURITY_ABORT,
        }
    }
}

impl From<LxError> for i32 {
    fn from(e: LxError) -> i32 {
        e.exit_code()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_codes_are_correct() {
        assert_eq!(LxError::LogicalError("".into()).exit_code(), 1);
        assert_eq!(LxError::BadUsage("".into()).exit_code(), 2);
        assert_eq!(LxError::ConfigAuth("".into()).exit_code(), 1);
        assert_eq!(LxError::NetworkLlm("".into()).exit_code(), 1);
        assert_eq!(LxError::SecurityAbort("".into()).exit_code(), 5);
    }

    #[test]
    fn alias_constants_match() {
        assert_eq!(EXIT_OK, SUCCESS);
        assert_eq!(EXIT_ERROR, LOGICAL_ERROR);
        assert_eq!(EXIT_USAGE, BAD_USAGE);
        assert_eq!(EXIT_DANGEROUS, DANGEROUS);
        assert_eq!(EXIT_SECURITY, SECURITY_ABORT);
    }

    #[test]
    fn dangerous_exit_code_is_3() {
        assert_eq!(DANGEROUS, 3);
    }

    #[test]
    fn from_lxerror_for_i32() {
        let code: i32 = LxError::BadUsage("bad".into()).into();
        assert_eq!(code, 2);
    }
}
