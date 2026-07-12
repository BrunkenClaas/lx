#![forbid(unsafe_code)]

const LANG_FALLBACK_PREFIX: &str = "[lang-fallback]";

const LANG_INSTRUCTION: &str =
    "\nReply in {lang}. If you cannot reliably produce output in {lang}, \
     reply in English and prepend your response with [lang-fallback].";

/// Inject the resolved OS name into a system prompt template.
///
/// - Replaces every `{os}` placeholder with `os_override` when provided, or
///   with the compile-time host OS (`platform::os()`) otherwise.
/// - If the template contains no `{os}` placeholder, returns the template unchanged.
pub fn inject_os(system_prompt: &str, os_override: &str) -> String {
    let resolved = if os_override.is_empty() || os_override == "auto" {
        lx_core::platform::os().to_string()
    } else {
        os_override.to_lowercase()
    };
    system_prompt.replace("{os}", &resolved)
}

/// Inject the resolved language into a system prompt template.
///
/// - Replaces every `{lang}` placeholder with `lang` (or with the
///   detected system locale when `lang == "auto"`).
/// - If the template contains no `{lang}` placeholder, appends the
///   standard language instruction as a new line.
pub fn inject_lang(system_prompt: &str, lang: &str) -> String {
    let resolved = if lang == "auto" {
        lx_core::platform::locale()
    } else {
        lang.to_string()
    };

    if system_prompt.contains("{lang}") {
        system_prompt.replace("{lang}", &resolved)
    } else {
        let instruction = LANG_INSTRUCTION.replace("{lang}", &resolved);
        format!("{system_prompt}{instruction}")
    }
}

/// Strip the `[lang-fallback]` prefix from a model response.
///
/// Returns `(cleaned_text, was_fallback)`. When `was_fallback` is true
/// the caller should emit a warning to stderr.
pub fn strip_lang_fallback(response: &str) -> (String, bool) {
    if let Some(rest) = response.strip_prefix(LANG_FALLBACK_PREFIX) {
        (rest.trim_start().to_string(), true)
    } else {
        (response.to_string(), false)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inject_lang_replaces_placeholder() {
        let tmpl = "You are a helper. Reply in {lang}.";
        let out = inject_lang(tmpl, "de");
        assert_eq!(out, "You are a helper. Reply in de.");
    }

    #[test]
    fn inject_lang_appends_when_no_placeholder() {
        let tmpl = "You are a helper.";
        let out = inject_lang(tmpl, "fr");
        assert!(out.starts_with("You are a helper."));
        assert!(out.contains("Reply in fr."));
        assert!(out.contains("[lang-fallback]"));
    }

    #[test]
    fn inject_lang_auto_returns_nonempty() {
        let tmpl = "Task. Reply in {lang}.";
        let out = inject_lang(tmpl, "auto");
        // The resolved locale must appear somewhere — not the literal "auto".
        assert!(!out.contains("auto"));
        assert!(out.contains("Reply in "));
    }

    #[test]
    fn strip_lang_fallback_detects_prefix() {
        let (text, was) = strip_lang_fallback("[lang-fallback] Hello world");
        assert!(was);
        assert_eq!(text, "Hello world");
    }

    #[test]
    fn strip_lang_fallback_clean_input() {
        let (text, was) = strip_lang_fallback("Hello world");
        assert!(!was);
        assert_eq!(text, "Hello world");
    }

    #[test]
    fn inject_os_replaces_placeholder() {
        let tmpl = "Generate commands for {os}.";
        let out = inject_os(tmpl, "windows");
        assert_eq!(out, "Generate commands for windows.");
    }

    #[test]
    fn inject_os_empty_override_uses_host_os() {
        let tmpl = "Target: {os}.";
        let out = inject_os(tmpl, "");
        assert!(
            matches!(
                out.as_str(),
                "Target: linux." | "Target: windows." | "Target: macos."
            ),
            "unexpected: {out}"
        );
    }

    #[test]
    fn inject_os_no_placeholder_returns_unchanged() {
        let tmpl = "No placeholder here.";
        assert_eq!(inject_os(tmpl, "linux"), tmpl);
    }

    #[test]
    fn inject_lang_replaces_all_occurrences() {
        let tmpl = "Reply in {lang}. Output language: {lang}.";
        let out = inject_lang(tmpl, "ja");
        assert_eq!(out, "Reply in ja. Output language: ja.");
    }
}
