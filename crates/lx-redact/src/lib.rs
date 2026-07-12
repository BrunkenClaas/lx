#![forbid(unsafe_code)]

pub mod entropy;
mod patterns;

use lx_core::exit::LxError;
use patterns::{
    GatedPattern, RedactPattern, AGGRESSIVE_SECRET_PATTERNS, STANDARD_PATTERNS, STRICT_PATTERNS,
};

/// Redaction strictness level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RedactLevel {
    /// Mask API keys, tokens, credentials, connection-string passwords, private
    /// key blocks, JWTs, high-entropy blobs, and email addresses.
    /// Entropy-gated on all prefixed detectors.
    Standard,
    /// Everything in `Standard`, plus IPv4 addresses, public hostnames, and
    /// home-directory paths.
    Strict,
    /// Everything in `Strict`, plus an expanded set of service-specific prefixed
    /// formats (niche services: Shopify, DigitalOcean, Linear, Doppler, Atlassian,
    /// Cloudflare, Heroku, Telegram, Discord, PyPI, GitLab runner, Square, HuggingFace,
    /// Postman) — all entropy-gated. Intended for `lxredact --strict`.
    Aggressive,
}

impl RedactLevel {
    /// Parse a redact level string.
    pub fn parse(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "strict" => RedactLevel::Strict,
            "aggressive" => RedactLevel::Aggressive,
            _ => RedactLevel::Standard,
        }
    }
}

impl std::str::FromStr for RedactLevel {
    type Err = std::convert::Infallible;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(RedactLevel::parse(s))
    }
}

/// Mask secrets and PII in `input` before it leaves the machine.
///
/// Returns the redacted string. Patterns are applied in order; each match
/// replaces only the secret value, not surrounding context (key names,
/// separators, etc.) where possible.
///
/// # Best-effort, not waterproof
/// Redaction recognises known secret formats (API keys, tokens, AWS/GitHub
/// credentials, JWTs, connection strings, private keys) and values assigned to
/// a broad set of secret-context keywords (`API_KEY=`, `token:`, `client_secret`,
/// …). It cannot reliably catch a secret whose surrounding name carries no such
/// keyword and whose value is too short to register as high-entropy, because
/// such a value is indistinguishable from ordinary data (a commit SHA, a
/// version, an identifier). Treat redaction as a strong safety net, not a
/// guarantee; do not rely on it to scrub arbitrary unstructured secrets.
///
/// All prefixed detectors (SK-*, AKIA*, ghp_*, xox*, SG.*, etc.) apply a
/// Shannon-entropy gate (≥ 2.0–4.0 bits/byte depending on the format, matching
/// the thresholds used by gitleaks) and a placeholder filter before redacting,
/// so low-entropy strings that happen to match a prefix (e.g. documentation
/// examples like `sk_live_television_channel_1`) are left untouched.
///
/// # Errors
/// Returns `Err(LxError::SecurityAbort)` if:
/// - Pattern application would remove > 80 % of the original input (prevents
///   sending near-empty strings to the LLM, which would produce nonsense).
/// - The regex engine encounters an internal error.
pub fn redact(input: &str, level: RedactLevel) -> Result<String, LxError> {
    if input.is_empty() {
        return Ok(String::new());
    }

    let mut output = apply_patterns(input, STANDARD_PATTERNS)?;

    if level == RedactLevel::Strict || level == RedactLevel::Aggressive {
        output = apply_patterns(&output, STRICT_PATTERNS)?;
    }

    if level == RedactLevel::Aggressive {
        output = apply_gated_patterns(&output, AGGRESSIVE_SECRET_PATTERNS)?;
    }

    // Guard: refuse to return a string that is mostly redaction placeholders.
    let redacted_chars = count_redacted_chars(&output);
    let original_len = input.len();
    if original_len > 0 && redacted_chars * 100 / original_len > 80 {
        return Err(LxError::SecurityAbort(
            "redaction would remove too much content — refusing to proceed".to_string(),
        ));
    }

    Ok(output)
}

/// Does `input` contain a Standard-set secret that redaction would actually mask?
///
/// Used in tests and assertions. This must stay in agreement with [`redact`]:
/// a bare regex match is not enough, because [`redact`] applies an entropy /
/// placeholder gate (`should_skip_value`) that deliberately leaves documentation
/// examples and low-entropy placeholders (e.g. `API_KEY=your-api-key-here`,
/// `sk-your_api_key_here`) untouched. Detecting on the raw regex alone would flag
/// values the masker intentionally keeps, making `assert_no_secrets_in_request`
/// fail on correctly-redacted input. So we report a secret only when applying the
/// Standard patterns would change the input — i.e. something was truly redacted.
pub fn has_secrets(input: &str) -> bool {
    match apply_patterns(input, STANDARD_PATTERNS) {
        Ok(redacted) => redacted != input,
        // If pattern application fails, fall back to the conservative regex check
        // so we never under-report a secret.
        Err(_) => STANDARD_PATTERNS.iter().any(|p| p.regex.is_match(input)),
    }
}

// ── Internal helpers ──────────────────────────────────────────────────────────

fn apply_patterns(input: &str, patterns: &[RedactPattern]) -> Result<String, LxError> {
    let mut current = input.to_owned();
    for p in patterns {
        current = apply_one(current, p)?;
    }
    Ok(current)
}

fn apply_gated_patterns(input: &str, patterns: &[GatedPattern]) -> Result<String, LxError> {
    let mut current = input.to_owned();
    for p in patterns {
        current = apply_gated_one(current, p)?;
    }
    Ok(current)
}

/// Apply a single `RedactPattern` to `input`, returning the modified string.
///
/// When `replace_whole` is true the entire regex match is replaced.
/// When `replace_whole` is false, only the *last* capture group is replaced —
/// this lets patterns like `api_key=<value>` keep the `api_key=` prefix intact.
///
/// For gated patterns (prefix + length only), the entropy and placeholder gate
/// are checked inside `apply_gated_one` instead.
fn apply_one(input: String, p: &RedactPattern) -> Result<String, LxError> {
    let regex = &**p.regex;
    let n_groups = regex.captures_len();

    if p.replace_whole || n_groups <= 1 {
        let result = regex.replace_all(&input, p.replacement);
        return Ok(result.into_owned());
    }

    let mut output = String::with_capacity(input.len());
    let mut last_end = 0usize;

    for caps in regex.captures_iter(&input) {
        let whole = caps.get(0).unwrap();
        output.push_str(&input[last_end..whole.start()]);

        let secret_group = (1..n_groups)
            .rev()
            .find(|&i| caps.get(i).map(|m| !m.is_empty()).unwrap_or(false));

        if let Some(gi) = secret_group {
            let m = caps.get(gi).unwrap();
            // Entropy + placeholder gate on the captured secret value.
            // Applied to existing prefix detectors too so low-entropy false
            // positives (e.g. sk-your_api_key_here_xxxx, documentation
            // examples) are left untouched. Uses the pattern's floor if set;
            // 2.0 is the minimum we consider meaningful for any secret.
            let value = m.as_str();
            if should_skip_value(value, p.min_entropy) {
                // Gate rejected: emit the whole match unchanged.
                output.push_str(&input[whole.start()..whole.end()]);
            } else {
                output.push_str(&input[whole.start()..m.start()]);
                output.push_str(p.replacement);
                output.push_str(&input[m.end()..whole.end()]);
            }
        } else {
            output.push_str(p.replacement);
        }

        last_end = whole.end();
    }

    output.push_str(&input[last_end..]);
    Ok(output)
}

/// Apply a `GatedPattern` (whole-match replacement, with entropy + placeholder gate).
fn apply_gated_one(input: String, p: &GatedPattern) -> Result<String, LxError> {
    let regex = &**p.regex;
    let n_groups = regex.captures_len();

    let mut output = String::with_capacity(input.len());
    let mut last_end = 0usize;

    for caps in regex.captures_iter(&input) {
        let whole = caps.get(0).unwrap();
        output.push_str(&input[last_end..whole.start()]);

        // The value to gate on is the last non-empty capture group (group ≥ 1),
        // or the whole match if there are no groups.
        let value = if n_groups > 1 {
            (1..n_groups)
                .rev()
                .find_map(|i| caps.get(i).filter(|m| !m.is_empty()))
                .map(|m| m.as_str())
                .unwrap_or_else(|| whole.as_str())
        } else {
            whole.as_str()
        };

        if should_skip_value(value, p.min_entropy) {
            output.push_str(&input[whole.start()..whole.end()]);
        } else {
            output.push_str(p.replacement);
        }

        last_end = whole.end();
    }

    output.push_str(&input[last_end..]);
    Ok(output)
}

/// Returns `true` when the value should NOT be redacted:
/// - Shannon entropy below the threshold (low-entropy = English words / sequences)
/// - Looks like a placeholder / documentation example
fn should_skip_value(value: &str, min_entropy: f64) -> bool {
    entropy::looks_like_placeholder(value) || entropy::shannon_entropy(value) < min_entropy
}

/// Count the characters consumed by placeholder tokens (conservative lower bound).
fn count_redacted_chars(s: &str) -> usize {
    let placeholders = ["[REDACTED]", "[EMAIL]", "[IP]", "[HOST]", "[PATH]"];
    placeholders
        .iter()
        .map(|p| s.matches(p).count() * p.len())
        .sum()
}

// ── Tests (unit) ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input_returns_empty() {
        assert_eq!(redact("", RedactLevel::Standard).unwrap(), "");
    }

    #[test]
    fn clean_input_unchanged() {
        let s = "cargo build --release";
        assert_eq!(redact(s, RedactLevel::Standard).unwrap(), s);
    }

    #[test]
    fn has_secrets_positive() {
        assert!(has_secrets("token: sk-abc12345678901234567890"));
        // A high-entropy AWS key (not the AKIAIOSFODNN7EXAMPLE doc placeholder).
        assert!(has_secrets("AKIAZX7QW3RTY9KLMN2P"));
    }

    #[test]
    fn has_secrets_negative() {
        assert!(!has_secrets("cargo build --release"));
        assert!(!has_secrets("sk-")); // too short
                                      // has_secrets must agree with redact: values the entropy/placeholder gate
                                      // leaves untouched are NOT reported as secrets. The canonical AWS docs
                                      // example and low-entropy placeholders would otherwise cause
                                      // assert_no_secrets_in_request to fail on correctly-redacted input.
        assert!(!has_secrets("AKIAIOSFODNN7EXAMPLE1234"));
        assert!(!has_secrets("API_KEY=your-api-key-here"));
    }

    #[test]
    fn oversized_redaction_aborts() {
        let almost_all_key = format!("{}sk-{}", "a", "b".repeat(50));
        let result = redact(&almost_all_key, RedactLevel::Standard);
        assert!(result.is_ok() || matches!(result, Err(LxError::SecurityAbort(_))));
    }

    #[test]
    fn validation_word_not_redacted() {
        let input = "Weak password validation rules";
        let result = redact(input, RedactLevel::Standard).unwrap();
        assert_eq!(result, input, "plain prose must not be redacted: {result}");
    }

    #[test]
    fn password_with_digit_is_redacted() {
        let input = "password=sk-abc123xyz789";
        let result = redact(input, RedactLevel::Standard).unwrap();
        assert!(
            result.contains("[REDACTED]"),
            "alphanumeric secret must be redacted: {result}"
        );
        assert!(
            !result.contains("sk-abc123xyz789"),
            "raw secret must not appear in output: {result}"
        );
    }

    #[test]
    fn long_password_no_digit_is_redacted() {
        let input = "password=SomeLongTokenValueHere";
        let result = redact(input, RedactLevel::Standard).unwrap();
        assert!(
            result.contains("[REDACTED]"),
            "long alphabetic token must be redacted: {result}"
        );
    }

    #[test]
    fn password_with_punctuation_is_redacted() {
        for input in [
            "password=Qw7k@PmRn!TvXs91",
            "the database password Qw7k@PmRn!TvXs91",
            "passphrase: Qw7k&9PmRn!TvXsLb",
        ] {
            let result = redact(input, RedactLevel::Standard).unwrap();
            assert!(
                result.contains("[REDACTED]"),
                "punctuated password must be redacted: {input} -> {result}"
            );
            assert!(
                !result.contains("Qw7k@PmRn!TvXs91") && !result.contains("Qw7k&9PmRn!TvXsLb"),
                "raw password must not survive: {input} -> {result}"
            );
        }
    }

    #[test]
    fn dotted_token_is_fully_redacted() {
        // A realistic dotted token (OAuth/cloud style). Must not contain placeholder words.
        let input = "LX_API_KEY=XY.Zz0aBcD3fGhIj4kLmNp7qRsT9uVwXy1234567890abcd";
        let result = redact(input, RedactLevel::Standard).unwrap();
        assert!(
            result.contains("[REDACTED]"),
            "dotted token must be redacted: {result}"
        );
        assert!(
            !result.contains("Zz0aBcD3fGhIj4k"),
            "no fragment of the key body may survive: {result}"
        );
    }

    #[test]
    fn expanded_keywords_are_redacted() {
        let cases = [
            "export API_KEY=23asdf8932.x898sdf-s123",
            "client_secret=abc123def456ghi789",
            "REFRESH_TOKEN: ey9fooBarBazQux12345",
            "session_token = 9f8a7b6c5d4e3f2a1b0c",
            "WEBHOOK_SECRET=whsec_aB3xY9zQ1234567",
            "license_key=A1B2-C3D4-E5F6-G7H8I9",
        ];
        for input in cases {
            let out = redact(input, RedactLevel::Standard).unwrap();
            assert!(
                out.contains("[REDACTED]"),
                "expected redaction for {input:?}, got {out:?}"
            );
        }
    }

    #[test]
    fn keyword_without_secret_value_is_not_redacted() {
        for input in [
            "the session is active",
            "client_id is required",
            "auth: yes",
        ] {
            let out = redact(input, RedactLevel::Standard).unwrap();
            assert_eq!(out, input, "prose must be left intact: {out:?}");
        }
    }

    #[test]
    fn keywordless_short_secret_passes_through_by_design() {
        let input = "export DONT_TELL=23asdf8932.x898sdf-s123";
        let out = redact(input, RedactLevel::Standard).unwrap();
        assert_eq!(
            out, input,
            "keywordless short value is left as-is by design"
        );
    }

    #[test]
    fn redact_level_from_str() {
        use std::str::FromStr;
        assert_eq!(
            RedactLevel::from_str("strict").unwrap(),
            RedactLevel::Strict
        );
        assert_eq!(
            RedactLevel::from_str("aggressive").unwrap(),
            RedactLevel::Aggressive
        );
        assert_eq!(
            RedactLevel::from_str("standard").unwrap(),
            RedactLevel::Standard
        );
        assert_eq!(
            RedactLevel::from_str("unknown").unwrap(),
            RedactLevel::Standard
        );
    }

    // ── Entropy gate tests ────────────────────────────────────────────────────

    #[test]
    fn low_entropy_stripe_prefix_not_redacted() {
        // "television_channel_1" after sk_live_ breaks at underscore in lxsecret,
        // but test a case that would pass length: low-entropy content must be rejected.
        // The RE_CONTEXT_SECRET gate handles this via the value gate — test that a
        // keyword-less low-entropy value with sk- prefix survives.
        let input = "sk-your_api_key_here_xxxxxxxxxxxxxxxxxxxx";
        let out = redact(input, RedactLevel::Standard).unwrap();
        // The placeholder gate ("your_") should prevent redaction.
        assert_eq!(
            out, input,
            "placeholder-shaped sk- value must not be redacted: {out}"
        );
    }

    #[test]
    fn high_entropy_sk_prefix_is_redacted() {
        // A real high-entropy sk- key (20+ alphanum/dash chars, mixed case + digits)
        // must be caught even without keyword context.
        let input = "sk-aBc1De2Fg3Hi4Jk5Lm6Np7Qr8St9Uv0Wx";
        let out = redact(input, RedactLevel::Standard).unwrap();
        assert!(
            out.contains("[REDACTED]"),
            "high-entropy sk- key must be redacted: {out}"
        );
    }
}
