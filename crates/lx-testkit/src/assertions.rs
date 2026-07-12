#![forbid(unsafe_code)]

use crate::mock::CapturedRequest;

/// Assert the invariants that every tool's LLM request must satisfy.
///
/// Call this in every integration test immediately after `run()`:
///
/// ```rust,ignore
/// lx_testkit::assertions::assert_request_invariants(&client.last_request());
/// ```
pub fn assert_request_invariants(req: &CapturedRequest) {
    assert!(
        req.temperature == 0.0,
        "temperature must be 0.0 for determinism, got {}",
        req.temperature
    );
    assert!(!req.system.is_empty(), "system prompt must not be empty");
    assert!(
        req.max_tokens > 0 && req.max_tokens <= 4096,
        "max_tokens {} out of expected range (1..=4096)",
        req.max_tokens
    );
}

/// Assert that no known secret patterns appear in the request's user field.
///
/// For tools with the `redact` security flag. Call after `run()`:
///
/// ```rust,ignore
/// lx_testkit::assertions::assert_no_secrets_in_request(&client.last_request());
/// ```
pub fn assert_no_secrets_in_request(req: &CapturedRequest) {
    assert!(
        !lx_redact::has_secrets(&req.user),
        "secret pattern found in request user field — redaction failed\n\
         user field (first 120 chars): {:?}",
        &req.user[..req.user.len().min(120)]
    );
    assert!(
        !lx_redact::has_secrets(&req.system),
        "secret pattern found in request system field — system prompt must not contain secrets"
    );
}

/// Assert that the captured request contains image data (for `lximg` tests).
pub fn assert_image_in_request(req: &CapturedRequest) {
    let img = req
        .image
        .as_ref()
        .expect("lximg must attach image data to request");
    assert!(
        !img.base64.is_empty(),
        "image base64 data must not be empty"
    );
    assert!(
        !img.media_type.is_empty(),
        "image media_type must not be empty"
    );
}

/// Soft check: warn if the system prompt appears to be missing language handling.
///
/// Does not panic — some tools may handle language differently. Emits a
/// `eprintln!` warning so test output makes the omission visible.
pub fn assert_lang_placeholder_in_system(req: &CapturedRequest) {
    let has_lang = req.system.contains("Reply in") || req.system.contains("{lang}");
    if !has_lang {
        eprintln!(
            "warning [assert_lang_placeholder_in_system]: \
             system prompt does not contain \"Reply in\" or \"{{lang}}\" — \
             language injection may be missing"
        );
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn req(system: &str, user: &str, max_tokens: u32, temperature: f32) -> CapturedRequest {
        CapturedRequest {
            system: system.to_string(),
            user: user.to_string(),
            max_tokens,
            temperature,
            image: None,
        }
    }

    #[test]
    fn invariants_pass_for_valid_request() {
        assert_request_invariants(&req("You are a helper.", "explain ls", 256, 0.0));
    }

    #[test]
    #[should_panic(expected = "temperature must be 0.0")]
    fn invariants_fail_nonzero_temperature() {
        assert_request_invariants(&req("sys", "user", 256, 0.5));
    }

    #[test]
    #[should_panic(expected = "system prompt must not be empty")]
    fn invariants_fail_empty_system() {
        assert_request_invariants(&req("", "user", 256, 0.0));
    }

    #[test]
    #[should_panic(expected = "max_tokens")]
    fn invariants_fail_zero_max_tokens() {
        assert_request_invariants(&req("sys", "user", 0, 0.0));
    }

    #[test]
    #[should_panic(expected = "max_tokens")]
    fn invariants_fail_excessive_max_tokens() {
        assert_request_invariants(&req("sys", "user", 5000, 0.0));
    }

    #[test]
    fn no_secrets_passes_clean_input() {
        assert_no_secrets_in_request(&req("You are a helper.", "explain ls -la", 256, 0.0));
    }

    #[test]
    #[should_panic(expected = "secret pattern found in request user field")]
    fn no_secrets_fails_on_api_key_in_user() {
        // sk- followed by 20+ alphanum chars triggers the OpenAI key pattern.
        assert_no_secrets_in_request(&req("sys", "token sk-abcdefghijklmnopqrstu", 256, 0.0));
    }

    #[test]
    fn lang_placeholder_check_does_not_panic_on_missing() {
        // Should only warn, not panic.
        assert_lang_placeholder_in_system(&req("No language instruction here.", "u", 64, 0.0));
    }

    #[test]
    fn lang_placeholder_check_passes_when_present() {
        assert_lang_placeholder_in_system(&req("Reply in en.", "u", 64, 0.0));
        assert_lang_placeholder_in_system(&req("Output: {lang}.", "u", 64, 0.0));
    }
}
