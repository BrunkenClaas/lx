#![forbid(unsafe_code)]

// Re-export so existing code using `lx_core::error::LxError` keeps compiling.
// The canonical home is `lx_core::exit::LxError`.
pub use crate::exit::LxError;

/// Print a structured error message to stderr.
///
/// Plain:  `error[E<n>]: <message>\n  hint: <hint>`
/// JSON:   `{"error":{"code":<n>,"message":"…","hint":"…"}}`
///
/// Always writes to stderr; never to stdout.
pub fn print_error(err: &LxError, json: bool) {
    let code = err.exit_code();
    let message = err.to_string();
    let hint = hint_for(err);

    if json {
        let msg_esc = json_escape(&message);
        let hint_esc = json_escape(hint);
        eprintln!(r#"{{"error":{{"code":{code},"message":"{msg_esc}","hint":"{hint_esc}"}}}}"#);
    } else {
        eprintln!("error[E{code}]: {message}");
        if !hint.is_empty() {
            eprintln!("  hint: {hint}");
        }
    }
}

fn hint_for(err: &LxError) -> &'static str {
    match err {
        LxError::ConfigAuth(_) => "Set LX_API_KEY env var or configure the OS credential store",
        LxError::NetworkLlm(_) => "Check your network connection and LLM provider status",
        LxError::SecurityAbort(_) => {
            "Use --no-redact to bypass redaction (not recommended), or sanitize the input"
        }
        LxError::BadUsage(_) => "Run with --help for usage information",
        LxError::LogicalError(_) => "",
    }
}

fn json_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_escape_covers_all_chars() {
        let escaped = json_escape("a\\b\"c\nd\re\tf");
        assert_eq!(escaped, r#"a\\b\"c\nd\re\tf"#);
    }
}
