// platform.rs needs unsafe for Windows console/locale syscalls; the
// forbid is applied per-module everywhere else.
pub mod error;
pub mod exit;
pub mod io;
pub mod output;
pub mod platform;
pub mod version;

/// Compatibility shim — locale detection now lives in `platform`.
/// Existing tool scaffolds reference `lx_core::locale::detect_lang()`;
/// this module keeps them compiling without mass edits.
pub mod locale {
    /// Re-export with the original name used by tool scaffolds.
    pub use crate::platform::locale as detect_lang;
    /// Re-export the suite label (previously defined here).
    pub use crate::version::LX_SUITE_LABEL as SUITE_LABEL;
}
