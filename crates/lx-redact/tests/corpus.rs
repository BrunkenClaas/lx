//! Corpus tests for lx-redact.
//!
//! Each pattern block has ≥3 positive examples (must be redacted) and
//! ≥2 negative examples (must NOT be redacted, even if they look similar).

use lx_redact::{has_secrets, redact, RedactLevel};

const STD: RedactLevel = RedactLevel::Standard;
const STRICT: RedactLevel = RedactLevel::Strict;

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Assert the output contains `[REDACTED]` and NOT the original secret.
fn assert_redacted(input: &str, secret: &str) {
    let output = redact(input, STD).expect("redact should not fail");
    assert!(
        output.contains("[REDACTED]"),
        "expected [REDACTED] in output for input: {input:?}\ngot: {output:?}"
    );
    assert!(
        !output.contains(secret),
        "secret still present in output for input: {input:?}\ngot: {output:?}"
    );
}

/// Assert the output is unchanged (no `[REDACTED]`, `[EMAIL]`, etc.).
fn assert_not_redacted(input: &str) {
    let output = redact(input, STD).expect("redact should not fail");
    for placeholder in &["[REDACTED]", "[EMAIL]", "[IP]", "[HOST]", "[PATH]"] {
        assert!(
            !output.contains(placeholder),
            "unexpected {placeholder} in output for input: {input:?}\ngot: {output:?}"
        );
    }
}

// ── OpenAI key ────────────────────────────────────────────────────────────────

#[test]
fn openai_key_positive() {
    assert_redacted("sk-abcdefghijklmnopqrstu", "sk-abcdefghijklmnopqrstu");
    assert_redacted("key = sk-proj-ABCDEFGHIJ1234567890xy", "sk-proj-ABCDEFGHIJ");
    assert_redacted(
        "export OPENAI_KEY=sk-T3BlbkFJ1234567890abcdefghij",
        "sk-T3BlbkFJ1234567890abcdefghij",
    );
}

#[test]
fn openai_key_negative() {
    // Too short — only 5 chars after "sk-"
    assert_not_redacted("sk-abc");
    // Word "sky-blue" contains "sk" but not the pattern prefix
    assert_not_redacted("color: sky-blue");
}

// ── Anthropic key ─────────────────────────────────────────────────────────────

#[test]
fn anthropic_key_positive() {
    // Real-looking high-entropy values (mixed case + digits).
    assert_redacted(
        "sk-ant-api03-aBcDeFgHiJkLmN1pQrStUvWxYz234567",
        "sk-ant-api03-",
    );
    assert_redacted(
        "LX_API_KEY=sk-ant-api01-zQ9xR7mT3kP5wL2nY8vU4jF6eG1hI0aB",
        "sk-ant-api01",
    );
    assert_redacted(
        r#"{"api_key": "sk-ant-api02-cD4eF7gH1iJ8kL3mN6oP0qR9sT2uV5wX"}"#,
        "sk-ant-api02",
    );
}

#[test]
fn anthropic_key_negative() {
    assert_not_redacted("sk-ant-"); // too short
    assert_not_redacted("sk-ant-short"); // under 20 chars after prefix
}

// ── AWS access key ────────────────────────────────────────────────────────────

#[test]
fn aws_access_key_positive() {
    // Real-looking AWS keys must be caught. The entropy gate rejects AWS's own
    // documentation example (AKIAIOSFODNN7EXAMPLE) because it is low-entropy by design —
    // that is the correct behaviour: documentation examples should not be redacted.
    assert_redacted("AKIAJ3MV4BNZC9X7PQRF", "AKIAJ3MV4BNZC9X7PQRF");
    assert_redacted(
        "aws_access_key_id = AKIAZK4L8NP2QX7BM3TF",
        "AKIAZK4L8NP2QX7BM3TF",
    );
    assert_redacted(
        "export AWS_KEY=AKIAI3MV9BNKC5X2PQZR",
        "AKIAI3MV9BNKC5X2PQZR",
    );
}

#[test]
fn aws_access_key_example_not_redacted() {
    // The official AWS documentation example is intentionally low-entropy and
    // must NOT be redacted — confirming the entropy gate works correctly.
    assert_not_redacted("AKIAIOSFODNN7EXAMPLE");
}

#[test]
fn aws_access_key_negative() {
    assert_not_redacted("AKIA"); // only prefix, no 16-char suffix
    assert_not_redacted("not-an-AKIA-key"); // AKIA not at word boundary
}

// ── GCP API key ───────────────────────────────────────────────────────────────

#[test]
fn gcp_key_positive() {
    assert_redacted(
        "key=AIzaSyD-9tSrke72I6e1234567890abcdefghij",
        "AIzaSyD-9tSrke72I6e1234567890abcdefghij",
    );
    assert_redacted(
        "GOOGLE_API_KEY=AIzaSyB_abcdefghijklmnopqrstuvwxyz12345",
        "AIzaSyB_",
    );
    // AIza + 35 chars = valid GCP key length
    assert_redacted(
        r#"{"gcpKey":"AIzaSyCabcdefghijklmnopqrstuvwxyz12345678"}"#,
        "AIzaSyCabc",
    );
}

#[test]
fn gcp_key_negative() {
    assert_not_redacted("AIza"); // prefix only
    assert_not_redacted("AIzaShort1234"); // under 35 chars after prefix
}

// ── GitHub tokens ─────────────────────────────────────────────────────────────

#[test]
fn github_token_positive() {
    // Real-looking high-entropy GitHub tokens (mixed case + digits, no sequential runs).
    assert_redacted(
        "ghp_R8mA9fL3kDe2nV0xPqWsYuIoBtJhMcZg5r6T",
        "ghp_R8mA9fL3kDe2nV0xPqWsYuIoBtJhMcZg5r6T",
    );
    assert_redacted(
        "GITHUB_TOKEN=gho_Kp7qR3sT9uV5wX1yZ2aB6cD4eF8gH0iJ",
        "gho_Kp7qR3sT9uV5wX1yZ2aB6cD4eF8gH0iJ",
    );
    assert_redacted(
        "token: ghs_mN4pQ8rS2tU6vW0xY3zA7bC1dE5fG9hI",
        "ghs_mN4pQ8rS2tU6vW0xY3zA7bC1dE5fG9hI",
    );
}

#[test]
fn github_token_negative() {
    assert_not_redacted("ghp_short"); // under 36 chars after prefix
    assert_not_redacted("github_pat_file"); // not a real token format
}

// ── Private key blocks ────────────────────────────────────────────────────────

#[test]
fn private_key_positive() {
    assert_redacted(
        "-----BEGIN RSA PRIVATE KEY-----\nMIIEpA==\n-----END RSA PRIVATE KEY-----",
        "MIIEpA==",
    );
    assert_redacted(
        "-----BEGIN PRIVATE KEY-----\nabc123==\n-----END PRIVATE KEY-----",
        "abc123",
    );
    assert_redacted(
        "-----BEGIN EC PRIVATE KEY-----\nxyz==\n-----END EC PRIVATE KEY-----",
        "xyz==",
    );
}

#[test]
fn private_key_negative() {
    assert_not_redacted("-----BEGIN CERTIFICATE-----"); // not a private key
    assert_not_redacted("BEGIN PRIVATE"); // not the full block marker
}

// ── JWT tokens ────────────────────────────────────────────────────────────────

#[test]
fn jwt_positive() {
    let jwt =
        "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJ1c2VyMTIzIn0.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c";
    let out = redact(jwt, STD).unwrap();
    assert!(out.contains("[REDACTED]"), "JWT not redacted: {out}");

    let jwt2 = "Bearer eyJhbGciOiJSUzI1NiJ9.eyJpc3MiOiJodHRwcyJ9.signature123";
    let out2 = redact(jwt2, STD).unwrap();
    assert!(
        out2.contains("[REDACTED]"),
        "JWT in Bearer not redacted: {out2}"
    );

    // JWT embedded in JSON
    let jwt3 = r#"{"token":"eyJhbGciOiJub25lIn0.eyJhZG1pbiI6dHJ1ZX0.fakesig123456"}"#;
    let out3 = redact(jwt3, STD).unwrap();
    assert!(
        out3.contains("[REDACTED]"),
        "JWT in JSON not redacted: {out3}"
    );
}

#[test]
fn jwt_negative() {
    // Only two segments — not a valid JWT
    assert_not_redacted("eyJhbGci.payload_only");
    // Random base64 without the eyJ prefix
    assert_not_redacted("aGVsbG8=.d29ybGQ=.c2lnbg==");
}

// ── Connection string passwords ───────────────────────────────────────────────

#[test]
fn conn_string_positive() {
    let s = "postgres://alice:S3cr3tP@$$w0rd@db.example.com:5432/mydb";
    let out = redact(s, STD).unwrap();
    assert!(
        out.contains("[REDACTED]"),
        "password not redacted in: {out}"
    );
    assert!(out.contains("alice"), "username should be preserved");

    let s2 = "mysql://root:hunter2@localhost/app";
    let out2 = redact(s2, STD).unwrap();
    assert!(out2.contains("[REDACTED]"), "mysql password not redacted");

    let s3 = "mongodb://admin:p@ssw0rd123@cluster.example.com/db";
    let out3 = redact(s3, STD).unwrap();
    assert!(out3.contains("[REDACTED]"), "mongodb password not redacted");
}

#[test]
fn conn_string_negative() {
    // No password in DSN
    assert_not_redacted("postgres://localhost/mydb");
    // Not a recognised scheme
    assert_not_redacted("ftp://user:pass@host/path");
}

// ── Context-keyed generic secrets ─────────────────────────────────────────────

#[test]
fn context_secret_positive() {
    assert_redacted("api_key=abcdefghijklmnop", "abcdefghijklmnop");
    assert_redacted("password: mysuperpassword123", "mysuperpassword");
    assert_redacted("token=eyJhbGciOiJub25lIn0", "eyJhbGciOiJub25lIn0");
}

#[test]
fn context_secret_negative() {
    // "key" in a package name — no assignment operator with a value
    assert_not_redacted("my-package-key");
    // "secret" in prose — no assignment
    assert_not_redacted("The secret to success is hard work.");
}

// ── High-entropy blobs ────────────────────────────────────────────────────────

#[test]
fn high_entropy_b64_positive() {
    // 44-char base64 string after assignment
    assert_redacted(
        "secret=dGhpcyBpcyBhIHNlY3JldCBzdHJpbmcgbG9uZw==",
        "dGhpcyBpcyBhIHNlY3JldCBzdHJpbmcgbG9uZw==",
    );
    assert_redacted(
        "auth: ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789abcd",
        "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789abcd",
    );
    assert_redacted(
        "credential=YWJjZGVmZ2hpamtsbW5vcHFyc3R1dnd4eXo=",
        "YWJjZGVmZ2hpamtsbW5vcHFyc3R1dnd4eXo=",
    );
}

#[test]
fn high_entropy_b64_negative() {
    // Short base64 (< 40 chars) — not treated as high-entropy secret
    assert_not_redacted("hash=aGVsbG8=");
    // Normal word (even if long)
    assert_not_redacted("description=averylongdescriptionwithoutspecialcharsatall");
}

// ── Email addresses ───────────────────────────────────────────────────────────

#[test]
fn email_positive() {
    let out = redact("contact: alice@example.com", STD).unwrap();
    assert!(out.contains("[EMAIL]"), "email not redacted: {out}");

    let out2 = redact("From: bob.smith+filter@corp.internal", STD).unwrap();
    assert!(out2.contains("[EMAIL]"), "email not redacted: {out2}");

    let out3 = redact("users = [\"admin@company.io\", \"dev@company.io\"]", STD).unwrap();
    assert!(
        out3.contains("[EMAIL]"),
        "emails in array not redacted: {out3}"
    );
}

#[test]
fn email_negative() {
    // Just an @ sign without a valid domain
    assert_not_redacted("not an email @ symbol");
    // Incomplete address
    assert_not_redacted("missing-tld@domain");
}

// ── Strict: IPv4 ──────────────────────────────────────────────────────────────

#[test]
fn ipv4_strict_positive() {
    let check = |s: &str| {
        let out = redact(s, STRICT).expect("redact ok");
        assert!(
            out.contains("[IP]"),
            "expected [IP] in: {out:?} for input: {s:?}"
        );
    };
    check("server = 192.168.1.1");
    check("host: 10.0.0.1");
    check("connecting to 172.16.254.1");
}

#[test]
fn ipv4_not_redacted_in_standard() {
    // IPs should NOT be redacted at Standard level.
    let out = redact("server: 192.168.1.1", STD).unwrap();
    assert!(
        !out.contains("[IP]"),
        "IP should not be redacted in Standard mode"
    );
}

// ── Strict: hostnames ─────────────────────────────────────────────────────────

#[test]
fn hostname_strict_positive() {
    let check = |s: &str| {
        let out = redact(s, STRICT).expect("redact ok");
        assert!(
            out.contains("[HOST]"),
            "expected [HOST] in: {out:?} for input: {s:?}"
        );
    };
    check("endpoint: api.example.com");
    check("host = db.internal");
    check("url: https://service.corp/path");
}

#[test]
fn hostname_not_redacted_in_standard() {
    let out = redact("host: api.example.com", STD).unwrap();
    assert!(
        !out.contains("[HOST]"),
        "hostname should not be redacted in Standard mode"
    );
}

// ── Strict: local paths ───────────────────────────────────────────────────────

#[test]
fn local_path_strict_positive() {
    let check = |s: &str| {
        let out = redact(s, STRICT).expect("redact ok");
        assert!(
            out.contains("[PATH]"),
            "expected [PATH] in: {out:?} for input: {s:?}"
        );
    };
    check("config: /home/alice/.ssh/config");
    check("keyfile = /home/bob/secrets/key.pem");
    check(r"profile: C:\Users\charlie\AppData\Roaming");
}

#[test]
fn local_path_not_redacted_in_standard() {
    let out = redact("/home/alice/.ssh/config", STD).unwrap();
    assert!(
        !out.contains("[PATH]"),
        "path should not be redacted in Standard mode"
    );
}

// ── Clean pass-through ────────────────────────────────────────────────────────

#[test]
fn typical_diff_hunk_unchanged() {
    // A realistic diff hunk with no secrets should pass through unmodified.
    let diff = r#"diff --git a/src/main.rs b/src/main.rs
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,4 +1,5 @@
 fn main() {
-    println!("hello");
+    println!("hello, world");
 }"#;
    let out = redact(diff, STD).unwrap();
    // No placeholders should appear.
    for p in &["[REDACTED]", "[EMAIL]", "[IP]", "[HOST]", "[PATH]"] {
        assert!(!out.contains(p), "unexpected {p} in diff output:\n{out}");
    }
}

#[test]
fn git_sha_not_redacted() {
    // A 40-char git SHA in a standard diff context must not be redacted.
    // (It's pure hex but doesn't appear after an assignment operator.)
    let commit_line = "commit a3f1e2d4c5b6789012345678901234567890abcd";
    let out = redact(commit_line, STD).unwrap();
    assert!(
        !out.contains("[REDACTED]"),
        "git SHA was incorrectly redacted:\n{out}"
    );
}

// ── has_secrets smoke tests ───────────────────────────────────────────────────

#[test]
fn has_secrets_detects_known_patterns() {
    assert!(has_secrets("sk-ABCDEFGHIJKLMNOPQRSTU"));
    // A high-entropy AWS key (not the AKIAIOSFODNN7EXAMPLE doc placeholder).
    assert!(has_secrets("AKIAZX7QW3RTY9KLMN2P"));
    assert!(has_secrets("token=supersecretvalue123"));
}

#[test]
fn has_secrets_clean_input() {
    assert!(!has_secrets("cargo test --workspace"));
    assert!(!has_secrets("The quick brown fox"));
    assert!(!has_secrets(""));
}

#[test]
fn has_secrets_agrees_with_redact_on_gated_values() {
    // has_secrets must not flag values that redact() deliberately leaves in place
    // via the entropy/placeholder gate — otherwise assert_no_secrets_in_request
    // fails on correctly-redacted input (see lxpr diff_with_secret fixture).
    for gated in [
        "AKIAIOSFODNN7EXAMPLE1234",  // canonical AWS docs example
        "API_KEY=your-api-key-here", // documentation placeholder
        "sk-your_api_key_here_xxxx", // placeholder token
    ] {
        assert!(
            !has_secrets(gated),
            "has_secrets flagged {gated:?} but redact() leaves it untouched"
        );
        assert_eq!(
            redact(gated, STD).unwrap(),
            gated,
            "redact() unexpectedly changed the gated value {gated:?}"
        );
    }
}
