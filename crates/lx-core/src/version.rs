#![forbid(unsafe_code)]

/// Suite-level release label embedded in `--version` output.
/// Update this when cutting a coordinated suite release.
pub const LX_SUITE_LABEL: &str = "2026-07";

/// Build the canonical `--version` string for any lx tool.
///
/// Output: `<binary> <version> (lx-coreutils <LABEL>, <target>)`
///
/// # Example
/// ```
/// use lx_core::version::build_version_string;
/// let s = build_version_string("lxexplain", "1.0.0");
/// assert!(s.starts_with("lxexplain 1.0.0 (lx-coreutils "));
/// ```
pub fn build_version_string(binary: &str, version: &str) -> String {
    let target = crate::platform::target_triple();
    format!("{binary} {version} (lx-coreutils {LX_SUITE_LABEL}, {target})")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_string_format() {
        let s = build_version_string("lxtest", "1.2.3");
        assert!(s.starts_with("lxtest 1.2.3 (lx-coreutils "));
        assert!(s.contains(LX_SUITE_LABEL));
    }

    #[test]
    fn suite_label_is_nonempty() {
        assert!(!LX_SUITE_LABEL.is_empty());
    }
}
