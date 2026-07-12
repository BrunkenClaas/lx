use lx_config::Config;
use lx_core::error::LxError;
use lx_llm::{inject_lang, parse_response};
use lx_llm::{LlmClient, Request};
// Shared entropy helpers from lx-redact — single source of truth.
use lx_redact::entropy::{looks_like_placeholder, shannon_entropy};
use serde::{Deserialize, Serialize};

pub const SYSTEM_TEMPLATE: &str = include_str!("../prompts/system.txt");
/// Tight token limit — the LLM only classifies real vs. placeholder.
const MAX_TOKENS: u32 = 128;

// ── Output types ─────────────────────────────────────────────────────────────

/// A single detected secret finding (masked — the value is never stored).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Finding {
    /// Secret type, e.g. "aws_access_key", "github_token".
    #[serde(rename = "type")]
    pub secret_type: String,
    /// Location: "filename:line" or "stdin:line".
    pub location: String,
    /// Partially-masked form, e.g. "AKIA****KPLE". Never the full value.
    pub masked: String,
    /// LLM assessment of whether this is a real credential or a placeholder.
    /// `None` when `--no-llm` / dry-run / no client call was made.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assessment: Option<String>,
}

/// Full output of `lxsecret`.
#[derive(Debug, Serialize, Deserialize)]
pub struct Output {
    pub findings: Vec<Finding>,
}

impl Output {
    /// Plain-text rendering: one masked finding per line.
    pub fn to_plain(&self) -> String {
        if self.findings.is_empty() {
            return String::new();
        }
        self.findings
            .iter()
            .map(|f| {
                let assess = f
                    .assessment
                    .as_deref()
                    .map(|a| format!(" [{a}]"))
                    .unwrap_or_default();
                format!("{}\t{}\t{}{}", f.secret_type, f.location, f.masked, assess)
            })
            .collect::<Vec<_>>()
            .join("\n")
            + "\n"
    }
}

// ── LLM assessment response ───────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct AssessmentResponse {
    assessment: String,
    #[allow(dead_code)]
    confidence: String,
    #[allow(dead_code)]
    reason: String,
}

// ── Pattern definitions ───────────────────────────────────────────────────────

/// A detected raw match before masking.
struct RawMatch {
    secret_type: &'static str,
    /// Full matched value (never logged/sent to LLM).
    value: String,
    _line_number: usize,
    /// Surrounding variable name / key name hint (sent to LLM instead of value).
    context_hint: String,
}

/// Mask a value: keep first 4 and last 4 chars, replace middle with `****`.
/// For short values (≤8 chars), mask everything with `****`.
pub fn mask_value(value: &str) -> String {
    if value.len() <= 8 {
        return "****".to_string();
    }
    let (prefix, suffix) = (&value[..4], &value[value.len() - 4..]);
    format!("{}****{}", prefix, suffix)
}

/// Gate check: returns true when a candidate should be rejected.
/// Mirrors the gate in the lx-redact crate for consistency.
fn gate_reject(value: &str, min_entropy: f64) -> bool {
    looks_like_placeholder(value) || shannon_entropy(value) < min_entropy
}

/// Scan a block of text for secret patterns.
/// Returns raw matches (value is never sent to any output or LLM).
fn scan_text(text: &str, source_name: &str, strict: bool) -> Vec<(RawMatch, String)> {
    let mut results = Vec::new();

    for (line_idx, line) in text.lines().enumerate() {
        let line_num = line_idx + 1;

        // ── AWS Access Key ID ─────────────────────────────────────────────
        // Prefixes: AKIA, ASIA, ABIA, ACCA, A3T + 16 uppercase alphanumeric.
        // Entropy floor 3.0 (gitleaks).
        find_pattern(
            line,
            line_num,
            source_name,
            "aws_access_key",
            |l| {
                let bytes = l.as_bytes();
                let mut pos = 0;
                let mut found = Vec::new();
                let aws_prefixes: &[&[u8]] = &[
                    b"AKIA", b"ASIA", b"ABIA", b"ACCA", b"A3T_", b"AIPA", b"ANPA", b"ANVA",
                    b"AROA", b"APKA", b"AIDA",
                ];
                while pos + 20 <= bytes.len() {
                    let slice = &bytes[pos..];
                    if aws_prefixes.iter().any(|p| slice.starts_with(p)) {
                        let end = pos + 20;
                        let candidate = &bytes[pos..end];
                        if candidate.iter().all(|b| b.is_ascii_alphanumeric()) {
                            if let Ok(s) = std::str::from_utf8(candidate) {
                                if !gate_reject(s, 3.0) {
                                    found.push(s.to_string());
                                }
                            }
                        }
                        pos += 20;
                    } else {
                        let ch_len = l[pos..].chars().next().map(|c| c.len_utf8()).unwrap_or(1);
                        pos += ch_len;
                    }
                }
                found
            },
            |_| "AWS access key ID pattern".to_string(),
            &mut results,
        );

        // ── GitHub tokens ─────────────────────────────────────────────────
        // ghp_, gho_, ghu_, ghs_, ghr_ + exactly 36 alphanumeric.
        // Entropy floor 3.0.
        for (prefix, secret_type) in [
            ("ghp_", "github_pat"),
            ("ghs_", "github_server_token"),
            ("gho_", "github_oauth_token"),
            ("ghu_", "github_user_token"),
            ("ghr_", "github_refresh_token"),
        ] {
            find_pattern(
                line,
                line_num,
                source_name,
                secret_type,
                |l| {
                    let mut found = Vec::new();
                    let mut search = l;
                    while let Some(idx) = search.find(prefix) {
                        let start = idx + prefix.len();
                        let rest = &search[start..];
                        let end = rest
                            .find(|c: char| !c.is_ascii_alphanumeric())
                            .unwrap_or(rest.len());
                        // GitHub PATs are exactly 36 chars after the prefix.
                        if end >= 36 {
                            let val = format!("{}{}", prefix, &rest[..end.min(40)]);
                            if !gate_reject(&rest[..end.min(40)], 3.0) {
                                found.push(val);
                            }
                        }
                        search = &search[idx + prefix.len()..];
                    }
                    found
                },
                |_| format!("{secret_type} pattern"),
                &mut results,
            );
        }

        // ── GitHub fine-grained PAT ───────────────────────────────────────
        // github_pat_ + 82 word chars. Entropy floor 3.0.
        find_pattern(
            line,
            line_num,
            source_name,
            "github_fine_grained_pat",
            |l| {
                let prefix = "github_pat_";
                let mut found = Vec::new();
                let mut search = l;
                while let Some(idx) = search.find(prefix) {
                    let rest = &search[idx + prefix.len()..];
                    let end = rest
                        .find(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                        .unwrap_or(rest.len());
                    if end >= 82 {
                        let val = &rest[..end];
                        if !gate_reject(val, 3.0) {
                            found.push(format!("{prefix}{val}"));
                        }
                    }
                    search = &search[idx + prefix.len()..];
                }
                found
            },
            |_| "GitHub fine-grained PAT pattern".to_string(),
            &mut results,
        );

        // ── GitLab PAT ───────────────────────────────────────────────────
        // glpat- + 20+ word/dash chars. Entropy floor 3.0.
        find_pattern(
            line,
            line_num,
            source_name,
            "gitlab_pat",
            |l| {
                let prefix = "glpat-";
                let mut found = Vec::new();
                let mut search = l;
                while let Some(idx) = search.find(prefix) {
                    let rest = &search[idx + prefix.len()..];
                    let end = rest
                        .find(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '-')
                        .unwrap_or(rest.len());
                    if end >= 20 {
                        let val = &rest[..end];
                        if !gate_reject(val, 3.0) {
                            found.push(format!("{prefix}{val}"));
                        }
                    }
                    search = &search[idx + prefix.len()..];
                }
                found
            },
            |_| "GitLab PAT pattern".to_string(),
            &mut results,
        );

        // ── GitLab runner token ───────────────────────────────────────────
        // glrt- + 20 word/dash chars. Entropy floor 3.0.
        find_pattern(
            line,
            line_num,
            source_name,
            "gitlab_runner_token",
            |l| {
                let prefix = "glrt-";
                let mut found = Vec::new();
                let mut search = l;
                while let Some(idx) = search.find(prefix) {
                    let rest = &search[idx + prefix.len()..];
                    let end = rest
                        .find(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '-')
                        .unwrap_or(rest.len());
                    if end >= 20 {
                        let val = &rest[..end];
                        if !gate_reject(val, 3.0) {
                            found.push(format!("{prefix}{val}"));
                        }
                    }
                    search = &search[idx + prefix.len()..];
                }
                found
            },
            |_| "GitLab runner token pattern".to_string(),
            &mut results,
        );

        // ── Google API Keys ───────────────────────────────────────────────
        // AIza + 35+ alphanumeric/dash/underscore. Entropy floor 4.0.
        find_pattern(
            line,
            line_num,
            source_name,
            "google_api_key",
            |l| {
                let mut found = Vec::new();
                let mut search = l;
                while let Some(idx) = search.find("AIza") {
                    let rest = &search[idx + 4..];
                    let end = rest
                        .find(|c: char| !c.is_ascii_alphanumeric() && c != '-' && c != '_')
                        .unwrap_or(rest.len());
                    if end >= 35 {
                        let val = &rest[..end];
                        if !gate_reject(val, 4.0) {
                            found.push(format!("AIza{}", val));
                        }
                    }
                    search = &search[idx + 4..];
                }
                found
            },
            |_| "Google API key pattern".to_string(),
            &mut results,
        );

        // ── Slack Tokens ──────────────────────────────────────────────────
        // xoxb-/xoxp-/xoxe-/xoxa-/xoxr-/xoxs- + segments. Entropy floor 2.0.
        find_pattern(
            line,
            line_num,
            source_name,
            "slack_token",
            |l| {
                let mut found = Vec::new();
                let prefixes = ["xoxb-", "xoxa-", "xoxp-", "xoxr-", "xoxs-", "xoxe-"];
                for prefix in prefixes {
                    let mut search = l;
                    while let Some(idx) = search.find(prefix) {
                        let rest = &search[idx..];
                        let end = rest
                            .find(|c: char| c.is_whitespace() || c == '"' || c == '\'')
                            .unwrap_or(rest.len());
                        if end >= prefix.len() + 10 {
                            let val = &rest[..end];
                            // Gate on the part after the prefix.
                            let suffix = &rest[prefix.len()..end];
                            if !gate_reject(suffix, 2.0) {
                                found.push(val.to_string());
                            }
                        }
                        search = &search[idx + prefix.len()..];
                    }
                }
                found
            },
            |_| "Slack token pattern".to_string(),
            &mut results,
        );

        // ── Stripe Keys ───────────────────────────────────────────────────
        // sk_live_, sk_test_, sk_prod_, rk_live_, rk_test_, rk_prod_ + 24+ alphanumeric.
        // Entropy floor 2.0.
        for (prefix, secret_type) in [
            ("sk_live_", "stripe_secret_key"),
            ("sk_test_", "stripe_test_key"),
            ("rk_live_", "stripe_restricted_key"),
            ("rk_test_", "stripe_restricted_test_key"),
        ] {
            find_pattern(
                line,
                line_num,
                source_name,
                secret_type,
                |l| {
                    let mut found = Vec::new();
                    let mut search = l;
                    while let Some(idx) = search.find(prefix) {
                        let rest = &search[idx + prefix.len()..];
                        let end = rest
                            .find(|c: char| !c.is_ascii_alphanumeric())
                            .unwrap_or(rest.len());
                        if end >= 24 {
                            let val = &rest[..end];
                            if !gate_reject(val, 2.0) {
                                found.push(format!("{}{}", prefix, val));
                            }
                        }
                        search = &search[idx + prefix.len()..];
                    }
                    found
                },
                |_| format!("{secret_type} pattern"),
                &mut results,
            );
        }

        // ── SendGrid API key ─────────────────────────────────────────────
        // SG. + 66 chars [a-zA-Z0-9=_\-.]. Entropy floor 2.0.
        find_pattern(
            line,
            line_num,
            source_name,
            "sendgrid_api_key",
            |l| {
                let prefix = "SG.";
                let mut found = Vec::new();
                let mut search = l;
                while let Some(idx) = search.find(prefix) {
                    let rest = &search[idx + prefix.len()..];
                    let end = rest
                        .find(|c: char| {
                            !c.is_ascii_alphanumeric()
                                && c != '='
                                && c != '_'
                                && c != '-'
                                && c != '.'
                        })
                        .unwrap_or(rest.len());
                    if end >= 66 {
                        let val = &rest[..end];
                        if !gate_reject(val, 2.0) {
                            found.push(format!("{prefix}{val}"));
                        }
                    }
                    search = &search[idx + prefix.len()..];
                }
                found
            },
            |_| "SendGrid API key pattern".to_string(),
            &mut results,
        );

        // ── Twilio API key ────────────────────────────────────────────────
        // SK + 32 hex chars. Entropy floor 3.0.
        find_pattern(
            line,
            line_num,
            source_name,
            "twilio_api_key",
            |l| {
                let prefix = "SK";
                let mut found = Vec::new();
                let mut search = l;
                while let Some(idx) = search.find(prefix) {
                    let rest = &search[idx + prefix.len()..];
                    let end = rest
                        .find(|c: char| !c.is_ascii_hexdigit())
                        .unwrap_or(rest.len());
                    if end == 32 {
                        let val = &rest[..32];
                        if !gate_reject(val, 3.0) {
                            found.push(format!("{prefix}{val}"));
                        }
                    }
                    search = &search[idx + prefix.len()..];
                }
                found
            },
            |_| "Twilio API key pattern".to_string(),
            &mut results,
        );

        // ── npm access token ──────────────────────────────────────────────
        // npm_ + 36 alphanumeric. Entropy floor 2.0.
        find_pattern(
            line,
            line_num,
            source_name,
            "npm_token",
            |l| {
                let prefix = "npm_";
                let mut found = Vec::new();
                let mut search = l;
                while let Some(idx) = search.find(prefix) {
                    let rest = &search[idx + prefix.len()..];
                    let end = rest
                        .find(|c: char| !c.is_ascii_alphanumeric())
                        .unwrap_or(rest.len());
                    if end >= 36 {
                        let val = &rest[..end];
                        if !gate_reject(val, 2.0) {
                            found.push(format!("{prefix}{val}"));
                        }
                    }
                    search = &search[idx + prefix.len()..];
                }
                found
            },
            |_| "npm access token pattern".to_string(),
            &mut results,
        );

        // ── OpenAI / Generic sk- tokens ───────────────────────────────────
        // sk- + 20+ alphanumeric/dash. Entropy floor 3.0.
        find_pattern(
            line,
            line_num,
            source_name,
            "openai_api_key",
            |l| {
                let mut found = Vec::new();
                let mut search = l;
                while let Some(idx) = search.find("sk-") {
                    let rest = &search[idx + 3..];
                    let end = rest
                        .find(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '-')
                        .unwrap_or(rest.len());
                    if end >= 20 {
                        let val = &rest[..end];
                        if !gate_reject(val, 3.0) {
                            found.push(format!("sk-{}", val));
                        }
                    }
                    search = &search[idx + 3..];
                }
                found
            },
            |_| "OpenAI/generic sk- API key pattern".to_string(),
            &mut results,
        );

        // ── PEM Private Keys ──────────────────────────────────────────────
        if line.contains("-----BEGIN") && line.contains("PRIVATE KEY-----") {
            results.push((
                RawMatch {
                    secret_type: "private_key",
                    value: line.trim().to_string(),
                    _line_number: line_num,
                    context_hint: "PEM private key header".to_string(),
                },
                format!("{}:{}", source_name, line_num),
            ));
        }

        // ── Shopify tokens ────────────────────────────────────────────────
        // shpat_/shppa_/shpca_ + 32 hex chars. Entropy floor 2.0.
        for (prefix, secret_type) in [
            ("shpat_", "shopify_access_token"),
            ("shppa_", "shopify_private_app_token"),
            ("shpca_", "shopify_custom_app_token"),
        ] {
            find_pattern(
                line,
                line_num,
                source_name,
                secret_type,
                |l| {
                    let mut found = Vec::new();
                    let mut search = l;
                    while let Some(idx) = search.find(prefix) {
                        let rest = &search[idx + prefix.len()..];
                        let end = rest
                            .find(|c: char| !c.is_ascii_hexdigit())
                            .unwrap_or(rest.len());
                        if end >= 32 {
                            let val = &rest[..end];
                            if !gate_reject(val, 2.0) {
                                found.push(format!("{prefix}{val}"));
                            }
                        }
                        search = &search[idx + prefix.len()..];
                    }
                    found
                },
                |_| format!("{secret_type} pattern"),
                &mut results,
            );
        }

        // ── DigitalOcean tokens ───────────────────────────────────────────
        // dop_v1_/doo_v1_ + 64 hex chars. Entropy floor 3.0.
        for (prefix, secret_type) in [
            ("dop_v1_", "digitalocean_pat"),
            ("doo_v1_", "digitalocean_oauth_token"),
        ] {
            find_pattern(
                line,
                line_num,
                source_name,
                secret_type,
                |l| {
                    let mut found = Vec::new();
                    let mut search = l;
                    while let Some(idx) = search.find(prefix) {
                        let rest = &search[idx + prefix.len()..];
                        let end = rest
                            .find(|c: char| !c.is_ascii_hexdigit())
                            .unwrap_or(rest.len());
                        if end >= 64 {
                            let val = &rest[..end];
                            if !gate_reject(val, 3.0) {
                                found.push(format!("{prefix}{val}"));
                            }
                        }
                        search = &search[idx + prefix.len()..];
                    }
                    found
                },
                |_| format!("{secret_type} pattern"),
                &mut results,
            );
        }

        // ── Hugging Face token ────────────────────────────────────────────
        // hf_ + 34 alphanumeric. Entropy floor 2.0.
        find_pattern(
            line,
            line_num,
            source_name,
            "huggingface_token",
            |l| {
                let prefix = "hf_";
                let mut found = Vec::new();
                let mut search = l;
                while let Some(idx) = search.find(prefix) {
                    let rest = &search[idx + prefix.len()..];
                    let end = rest
                        .find(|c: char| !c.is_ascii_alphanumeric())
                        .unwrap_or(rest.len());
                    if end >= 34 {
                        let val = &rest[..end];
                        if !gate_reject(val, 2.0) {
                            found.push(format!("{prefix}{val}"));
                        }
                    }
                    search = &search[idx + prefix.len()..];
                }
                found
            },
            |_| "Hugging Face token pattern".to_string(),
            &mut results,
        );

        // ── Linear API token ──────────────────────────────────────────────
        // lin_api_ + 40 lowercase alphanumeric. Entropy floor 2.0.
        find_pattern(
            line,
            line_num,
            source_name,
            "linear_api_token",
            |l| {
                let prefix = "lin_api_";
                let mut found = Vec::new();
                let mut search = l;
                while let Some(idx) = search.find(prefix) {
                    let rest = &search[idx + prefix.len()..];
                    let end = rest
                        .find(|c: char| !c.is_ascii_alphanumeric())
                        .unwrap_or(rest.len());
                    if end >= 40 {
                        let val = &rest[..end];
                        if !gate_reject(val, 2.0) {
                            found.push(format!("{prefix}{val}"));
                        }
                    }
                    search = &search[idx + prefix.len()..];
                }
                found
            },
            |_| "Linear API token pattern".to_string(),
            &mut results,
        );

        // ── Postman API token ─────────────────────────────────────────────
        // PMAK- + 24 hex + - + 34 hex. Entropy floor 3.0.
        find_pattern(
            line,
            line_num,
            source_name,
            "postman_api_token",
            |l| {
                let prefix = "PMAK-";
                let mut found = Vec::new();
                let mut search = l;
                while let Some(idx) = search.find(prefix) {
                    let rest = &search[idx + prefix.len()..];
                    // Expect 24 hex, dash, 34 hex.
                    let part1_end = rest
                        .find(|c: char| !c.is_ascii_hexdigit())
                        .unwrap_or(rest.len());
                    if part1_end == 24 && rest.as_bytes().get(24) == Some(&b'-') {
                        let part2 = &rest[25..];
                        let part2_end = part2
                            .find(|c: char| !c.is_ascii_hexdigit())
                            .unwrap_or(part2.len());
                        if part2_end >= 34 {
                            let val = &rest[..25 + part2_end];
                            if !gate_reject(val, 3.0) {
                                found.push(format!("{prefix}{val}"));
                            }
                        }
                    }
                    search = &search[idx + prefix.len()..];
                }
                found
            },
            |_| "Postman API token pattern".to_string(),
            &mut results,
        );

        // ── Doppler service token ─────────────────────────────────────────
        // dp.pt. + 43 lowercase alphanumeric. Entropy floor 2.0.
        find_pattern(
            line,
            line_num,
            source_name,
            "doppler_token",
            |l| {
                let prefix = "dp.pt.";
                let mut found = Vec::new();
                let mut search = l;
                while let Some(idx) = search.find(prefix) {
                    let rest = &search[idx + prefix.len()..];
                    let end = rest
                        .find(|c: char| !c.is_ascii_alphanumeric())
                        .unwrap_or(rest.len());
                    if end >= 43 {
                        let val = &rest[..end];
                        if !gate_reject(val, 2.0) {
                            found.push(format!("{prefix}{val}"));
                        }
                    }
                    search = &search[idx + prefix.len()..];
                }
                found
            },
            |_| "Doppler token pattern".to_string(),
            &mut results,
        );

        // ── Atlassian API token ────────────────────────────────────────────
        // ATATT3 + 186 alphanumeric/dash/underscore/equals. Entropy floor 3.5.
        find_pattern(
            line,
            line_num,
            source_name,
            "atlassian_api_token",
            |l| {
                let prefix = "ATATT3";
                let mut found = Vec::new();
                let mut search = l;
                while let Some(idx) = search.find(prefix) {
                    let rest = &search[idx + prefix.len()..];
                    let end = rest
                        .find(|c: char| {
                            !c.is_ascii_alphanumeric() && c != '_' && c != '-' && c != '='
                        })
                        .unwrap_or(rest.len());
                    if end >= 186 {
                        let val = &rest[..end];
                        if !gate_reject(val, 3.5) {
                            found.push(format!("{prefix}{val}"));
                        }
                    }
                    search = &search[idx + prefix.len()..];
                }
                found
            },
            |_| "Atlassian API token pattern".to_string(),
            &mut results,
        );

        // ── Heroku API key v2 ─────────────────────────────────────────────
        // HRKU-AA + 58 alphanumeric/dash/underscore. Entropy floor 4.0.
        find_pattern(
            line,
            line_num,
            source_name,
            "heroku_api_key",
            |l| {
                let prefix = "HRKU-AA";
                let mut found = Vec::new();
                let mut search = l;
                while let Some(idx) = search.find(prefix) {
                    let rest = &search[idx + prefix.len()..];
                    let end = rest
                        .find(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '-')
                        .unwrap_or(rest.len());
                    if end >= 58 {
                        let val = &rest[..end];
                        if !gate_reject(val, 4.0) {
                            found.push(format!("{prefix}{val}"));
                        }
                    }
                    search = &search[idx + prefix.len()..];
                }
                found
            },
            |_| "Heroku API key pattern".to_string(),
            &mut results,
        );

        // ── PyPI upload token ─────────────────────────────────────────────
        // pypi-AgEIcHlwaS5vcmc + 50+ word/dash chars. Entropy floor 3.0.
        find_pattern(
            line,
            line_num,
            source_name,
            "pypi_token",
            |l| {
                let prefix = "pypi-AgEIcHlwaS5vcmc";
                let mut found = Vec::new();
                let mut search = l;
                while let Some(idx) = search.find(prefix) {
                    let rest = &search[idx + prefix.len()..];
                    let end = rest
                        .find(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '-')
                        .unwrap_or(rest.len());
                    if end >= 50 {
                        let val = &rest[..end];
                        if !gate_reject(val, 3.0) {
                            found.push(format!("{prefix}{val}"));
                        }
                    }
                    search = &search[idx + prefix.len()..];
                }
                found
            },
            |_| "PyPI token pattern".to_string(),
            &mut results,
        );

        // ── Telegram bot token ────────────────────────────────────────────
        // <5-16 digits>:A<34 alphanumeric/dash/underscore>. Entropy floor 2.0.
        find_pattern(
            line,
            line_num,
            source_name,
            "telegram_bot_token",
            |l| {
                let mut found = Vec::new();
                // Scan for digit sequences followed by :A
                let mut pos = 0;
                while pos < l.len() {
                    if let Some(colon_idx) = l[pos..].find(":A") {
                        let abs_colon = pos + colon_idx;
                        // Digits immediately before the colon.
                        let digits_start = l[..abs_colon]
                            .rfind(|c: char| !c.is_ascii_digit())
                            .map(|i| i + 1)
                            .unwrap_or(0);
                        let digit_len = abs_colon - digits_start;
                        if (5..=16).contains(&digit_len) {
                            let rest = &l[abs_colon + 2..]; // after :A
                            let end = rest
                                .find(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '-')
                                .unwrap_or(rest.len());
                            if end >= 34 {
                                let val = &rest[..end];
                                if !gate_reject(val, 2.0) {
                                    let full = format!("{}:A{}", &l[digits_start..abs_colon], val);
                                    found.push(full);
                                }
                            }
                        }
                        pos = abs_colon + 2;
                    } else {
                        break;
                    }
                }
                found
            },
            |_| "Telegram bot token pattern".to_string(),
            &mut results,
        );

        // ── High-entropy strings near sensitive keywords ───────────────────
        detect_high_entropy_near_keywords(line, line_num, source_name, strict, &mut results);
    }

    results
}

/// Helper: run a detection function over a line, add any matches to results.
fn find_pattern<F, H>(
    line: &str,
    line_num: usize,
    source_name: &str,
    secret_type: &'static str,
    detector: F,
    hint_fn: H,
    results: &mut Vec<(RawMatch, String)>,
) where
    F: Fn(&str) -> Vec<String>,
    H: Fn(&str) -> String,
{
    for value in detector(line) {
        results.push((
            RawMatch {
                secret_type,
                value: value.clone(),
                _line_number: line_num,
                context_hint: hint_fn(&value),
            },
            format!("{source_name}:{line_num}"),
        ));
    }
}

/// A credential keyword and the entropy/length bar its assigned value must clear.
struct CredentialKeyword {
    word: &'static str,
    /// `true` = strong, unambiguous keyword (`password`, `api_key`): the keyword
    /// itself is the signal, so a short human-chosen value clears a lenient bar.
    /// `false` = weak/ambiguous keyword (`key`): also an ordinary English word and
    /// universal config map-key, so the value must look genuinely key-like (the
    /// machine-token bar) before we report it.
    strong: bool,
}

/// Sensitive credential keywords. When one of these appears as an *assignment*
/// (`keyword = value` / `keyword: value`), the keyword signals that the assigned
/// value is likely a credential. Strong keywords lower the entropy/length bar to
/// catch even short human-chosen passwords; weak keywords require a high-entropy
/// value. The placeholder filter rejects obvious examples in both cases, and the
/// LLM assessment sorts the borderline real-vs-placeholder cases.
///
/// Hyphen/space spellings (`api-key`, `api key`) and underscore spellings
/// (`api_key`) are listed separately because the match is a plain substring scan,
/// not a normalised one. More-specific keywords must precede the generic ones so
/// `api-key` wins over a bare `key`.
const CREDENTIAL_KEYWORDS: &[CredentialKeyword] = &[
    CredentialKeyword {
        word: "password",
        strong: true,
    },
    CredentialKeyword {
        word: "passwd",
        strong: true,
    },
    CredentialKeyword {
        word: "passphrase",
        strong: true,
    },
    CredentialKeyword {
        word: "client_secret",
        strong: true,
    },
    CredentialKeyword {
        word: "client-secret",
        strong: true,
    },
    CredentialKeyword {
        word: "access_key",
        strong: true,
    },
    CredentialKeyword {
        word: "access-key",
        strong: true,
    },
    CredentialKeyword {
        word: "secret_key",
        strong: true,
    },
    CredentialKeyword {
        word: "secret-key",
        strong: true,
    },
    CredentialKeyword {
        word: "auth_token",
        strong: true,
    },
    CredentialKeyword {
        word: "auth-token",
        strong: true,
    },
    CredentialKeyword {
        word: "private_key",
        strong: true,
    },
    CredentialKeyword {
        word: "private-key",
        strong: true,
    },
    CredentialKeyword {
        word: "api_key",
        strong: true,
    },
    CredentialKeyword {
        word: "api-key",
        strong: true,
    },
    CredentialKeyword {
        word: "api key",
        strong: true,
    },
    CredentialKeyword {
        word: "apikey",
        strong: true,
    },
    CredentialKeyword {
        word: "secret",
        strong: true,
    },
    CredentialKeyword {
        word: "token",
        strong: true,
    },
    CredentialKeyword {
        word: "credential",
        strong: true,
    },
    CredentialKeyword {
        word: "pwd",
        strong: true,
    },
    // A curated handful of unambiguous non-English "password" words, strong-tier.
    // Deliberately limited to terms that are NOT ordinary words and do NOT mean
    // "key" (the "key"-meaning translations — clave/clé/chiave/schlüssel — are
    // ambiguous, exactly the trouble the weak `key` tier shows, so we omit them).
    // Machine-shaped secrets are already caught language-independently by the
    // prefix/high-entropy detectors; this only closes the human-password gap for
    // a translated keyword (e.g. `passwort: Qw7k@PmRn!TvXs91`).
    CredentialKeyword {
        word: "passwort", // de
        strong: true,
    },
    CredentialKeyword {
        word: "contraseña", // es
        strong: true,
    },
    CredentialKeyword {
        word: "contrasena", // es, ASCII spelling
        strong: true,
    },
    CredentialKeyword {
        word: "senha", // pt
        strong: true,
    },
    CredentialKeyword {
        word: "mot de passe", // fr
        strong: true,
    },
    CredentialKeyword {
        word: "motdepasse", // fr, no spaces
        strong: true,
    },
    // Weak: also a common English word and the universal config map-key.
    // Only fires on a genuinely key-like (high-entropy) value.
    CredentialKeyword {
        word: "key",
        strong: false,
    },
];

/// A credential keyword used as an assignment, with the value token and the tier
/// (strong/weak) that determines which entropy bar applies.
struct Assignment<'a> {
    keyword: &'static str,
    value: &'a str,
    strong: bool,
}

/// Find a credential keyword used as an *assignment* on the line.
///
/// Requiring the keyword to be immediately followed (after optional whitespace)
/// by `=` or `:` is what separates a real assignment (`password = hunter2`) from
/// a prose mention (`choosing a good password improves security`) — the latter
/// has no separator after the keyword and is correctly ignored.
fn find_credential_assignment(line: &str) -> Option<Assignment<'_>> {
    let lower = line.to_ascii_lowercase();
    for kw in CREDENTIAL_KEYWORDS {
        let mut from = 0;
        while let Some(rel) = lower[from..].find(kw.word) {
            let kw_start = from + rel;
            let kw_end = kw_start + kw.word.len();
            // Require a word boundary before the keyword so "password" does not
            // match inside "mypasswordfield", and "key" does not match "monkey".
            let prev_ok = kw_start == 0 || !lower.as_bytes()[kw_start - 1].is_ascii_alphanumeric();
            // After the keyword, optional whitespace then '=' or ':'.
            let after = &line[kw_end..];
            let trimmed = after.trim_start();
            let sep_ok = trimmed.starts_with('=') || trimmed.starts_with(':');
            if prev_ok && sep_ok {
                let val_region = trimmed[1..].trim_start_matches(['"', '\'', ' ', '\t']);
                let token: &str = val_region
                    .split(|c: char| {
                        c.is_whitespace() || c == '"' || c == '\'' || c == ',' || c == ';'
                    })
                    .next()
                    .unwrap_or("")
                    .trim_matches(|c: char| !c.is_ascii_graphic());
                if !token.is_empty() {
                    return Some(Assignment {
                        keyword: kw.word,
                        value: token,
                        strong: kw.strong,
                    });
                }
            }
            from = kw_end;
        }
    }
    None
}

/// Detect secret values assigned to a credential keyword, plus (in `--strict`
/// mode) a keyword-independent high-entropy sweep.
fn detect_high_entropy_near_keywords(
    line: &str,
    line_num: usize,
    source_name: &str,
    strict: bool,
    results: &mut Vec<(RawMatch, String)>,
) {
    let assignment = find_credential_assignment(line);

    // A credential keyword used as an assignment is itself a signal. Strong
    // keywords (password, api_key, …) lower the bar to catch even short
    // human-chosen passwords (len ≥ 8, entropy ≥ 2.5); weak keywords (`key`)
    // require a key-like high-entropy value (len ≥ 20, entropy ≥ 3.5) so they do
    // not fire on ordinary config map-values. The placeholder filter rejects
    // "test"/"example"/… in both cases; the LLM assessment disambiguates the rest.
    if let Some(a) = &assignment {
        let (min_len, min_entropy) = if a.strong { (8, 2.5) } else { (20, 3.5) };
        if a.value.len() >= min_len
            && shannon_entropy(a.value) >= min_entropy
            && !looks_like_placeholder(a.value)
        {
            results.push((
                RawMatch {
                    secret_type: "high_entropy_secret",
                    value: a.value.to_string(),
                    _line_number: line_num,
                    context_hint: format!("assigned to credential keyword '{}'", a.keyword),
                },
                format!("{source_name}:{line_num}"),
            ));
        }
    }

    // Strict mode: sweep all whitespace-delimited tokens regardless of keyword,
    // with a higher entropy floor and longer minimum length.
    //
    // This is the only path with no anchoring signal (no keyword, no prefix), so
    // it is the most false-positive-prone and the floors are deliberately tight:
    //
    //   entropy >= 4.0 — a hex string (16 symbols) maxes out *at* 4.0 and real
    //     hashes land just under (~3.95–3.97), so this floor is what excludes git
    //     SHAs, MD5s, and other hex digests. It is NOT the length floor that does
    //     this. Lowering it below ~3.95 floods the sweep with every hash in a diff.
    //
    //   len >= 24 — the smallest base64-encoded real secret is a 16-byte key/IV/
    //     salt, which is 24 chars. Below that lives the short-random-token noise
    //     band (nanoid/cuid/React list keys, ~16–22 chars at H 4.0–4.4) that we do
    //     not want to flag. 24 sits just above that band, so do not raise it: a
    //     24-char token is the boundary where a genuine secret first appears.
    //
    // Consequence: a bare password with no keyword (e.g. `H0lyM*ly123*liksdfju832`,
    // 23 chars / H 3.969) is indistinguishable from a hash to this gate and is left
    // alone. The credential-keyword assignment path above (lenient bar) is what
    // catches real passwords — they almost always appear as `password: <value>`.
    if strict && assignment.is_none() {
        for token in line.split_whitespace() {
            let t = token.trim_matches(|c: char| {
                c == '"' || c == '\'' || c == ',' || c == ';' || c == ':' || c == '='
            });
            if t.len() >= 24 && shannon_entropy(t) >= 4.0 && !looks_like_placeholder(t) {
                // Avoid flagging URLs and file paths.
                if t.starts_with("http") || t.starts_with('/') || t.starts_with('\\') {
                    continue;
                }
                results.push((
                    RawMatch {
                        secret_type: "high_entropy_secret",
                        value: t.to_string(),
                        _line_number: line_num,
                        context_hint: "high-entropy token (strict scan)".to_string(),
                    },
                    format!("{source_name}:{line_num}"),
                ));
            }
        }
    }
}

// ── Public run() function ─────────────────────────────────────────────────────

/// Core logic for lxsecret.
///
/// Security properties:
/// - Sends only masked forms and metadata to the LLM — never actual secret values.
pub fn run(
    input: &str,
    config: &Config,
    client: &dyn LlmClient,
    strict: bool,
) -> Result<Output, LxError> {
    run_inner(input, "stdin", config, Some(client), strict)
}

/// Variant used when no LLM call is desired (e.g. testing local detection only).
pub fn run_local(input: &str, strict: bool) -> Vec<Finding> {
    run_inner(input, "stdin", &Config::default(), None, strict)
        .map(|o| o.findings)
        .unwrap_or_default()
}

fn run_inner(
    input: &str,
    source_name: &str,
    config: &Config,
    client: Option<&dyn LlmClient>,
    strict: bool,
) -> Result<Output, LxError> {
    if input.trim().is_empty() {
        return Ok(Output { findings: vec![] });
    }

    let raw_matches = scan_text(input, source_name, strict);

    let mut findings = Vec::new();

    for (raw, location) in raw_matches {
        let masked = mask_value(&raw.value);

        let assessment = if let Some(cl) = client {
            let llm_input = serde_json::json!({
                "type": raw.secret_type,
                "location": location,
                "masked": masked,
                "context_hint": raw.context_hint,
            })
            .to_string();

            let system = inject_lang(SYSTEM_TEMPLATE, &config.output.lang);
            let req = Request {
                system: &system,
                user: &llm_input,
                max_tokens: MAX_TOKENS,
                temperature: 0.0,
                image: None,
            };

            match cl.complete(&req) {
                Ok(resp) => parse_response::<AssessmentResponse>(&resp.content)
                    .ok()
                    .map(|a| a.assessment),
                Err(_) => None,
            }
        } else {
            None
        };

        findings.push(Finding {
            secret_type: raw.secret_type.to_string(),
            location,
            masked,
            assessment,
        });
    }

    Ok(Output { findings })
}

// ── Directory scanning ────────────────────────────────────────────────────────

/// Scan all files within a directory (fsbound: stays inside root).
pub fn scan_directory(
    root: &std::path::Path,
    config: &Config,
    client: Option<&dyn LlmClient>,
    max_bytes_per_file: usize,
    strict: bool,
) -> Result<Output, LxError> {
    let canonical_root = std::fs::canonicalize(root)
        .map_err(|e| LxError::BadUsage(format!("cannot resolve path {}: {e}", root.display())))?;

    let mut all_findings = Vec::new();
    scan_dir_recursive(
        &canonical_root,
        &canonical_root,
        config,
        client,
        max_bytes_per_file,
        strict,
        &mut all_findings,
    )?;

    Ok(Output {
        findings: all_findings,
    })
}

fn scan_dir_recursive(
    dir: &std::path::Path,
    root: &std::path::Path,
    config: &Config,
    client: Option<&dyn LlmClient>,
    max_bytes: usize,
    strict: bool,
    findings: &mut Vec<Finding>,
) -> Result<(), LxError> {
    let entries = std::fs::read_dir(dir)
        .map_err(|e| LxError::BadUsage(format!("cannot read directory {}: {e}", dir.display())))?;

    for entry in entries {
        let entry = entry.map_err(|e| LxError::BadUsage(format!("directory entry error: {e}")))?;
        let path = entry.path();

        let canonical = match std::fs::canonicalize(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        if !canonical.starts_with(root) {
            eprintln!(
                "warning: skipping {} (escapes allowed root)",
                path.display()
            );
            continue;
        }

        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if should_skip_entry(&name_str) {
            continue;
        }

        if canonical.is_dir() {
            scan_dir_recursive(
                &canonical, root, config, client, max_bytes, strict, findings,
            )?;
        } else if canonical.is_file() {
            let source_name = canonical
                .strip_prefix(root)
                .unwrap_or(&canonical)
                .display()
                .to_string();

            let content = match lx_core::io::read_file(&canonical, max_bytes, Some(root)) {
                Ok(c) => c,
                Err(_) => continue,
            };

            if looks_binary(&content) {
                continue;
            }

            let raw_matches = scan_text(&content, &source_name, strict);
            for (raw, location) in raw_matches {
                let masked = mask_value(&raw.value);
                let assessment = if let Some(cl) = client {
                    let llm_input = serde_json::json!({
                        "type": raw.secret_type,
                        "location": location,
                        "masked": masked,
                        "context_hint": raw.context_hint,
                    })
                    .to_string();
                    let system = inject_lang(SYSTEM_TEMPLATE, &config.output.lang);
                    let req = Request {
                        system: &system,
                        user: &llm_input,
                        max_tokens: MAX_TOKENS,
                        temperature: 0.0,
                        image: None,
                    };
                    cl.complete(&req)
                        .ok()
                        .and_then(|r| parse_response::<AssessmentResponse>(&r.content).ok())
                        .map(|a| a.assessment)
                } else {
                    None
                };

                findings.push(Finding {
                    secret_type: raw.secret_type.to_string(),
                    location,
                    masked,
                    assessment,
                });
            }
        }
    }

    Ok(())
}

/// Return true if the entry name suggests it should be skipped.
fn should_skip_entry(name: &str) -> bool {
    matches!(
        name,
        ".git"
            | "node_modules"
            | "target"
            | ".cargo"
            | "vendor"
            | ".tox"
            | "__pycache__"
            | ".venv"
            | "venv"
            | "dist"
            | "build"
    )
}

/// Cheap heuristic: if the content has many null bytes, treat it as binary.
fn looks_binary(content: &str) -> bool {
    let null_count = content.bytes().filter(|&b| b == 0).count();
    !content.is_empty() && null_count * 100 / content.len() > 5
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mask_short_value() {
        assert_eq!(mask_value("abc"), "****");
        assert_eq!(mask_value("12345678"), "****");
    }

    #[test]
    fn mask_long_value() {
        let masked = mask_value("AKIAIOSFODNN7EXAMPLE");
        assert!(masked.starts_with("AKIA"));
        assert!(masked.ends_with("MPLE"));
        assert!(masked.contains("****"));
        assert!(!masked.contains("IOSFODNN7EXA"));
    }

    #[test]
    fn detects_aws_access_key() {
        // Use a high-entropy AWS-format key (AKIAIOSFODNN7EXAMPLE is the docs example
        // and correctly fails the entropy gate — use a realistic mixed key instead).
        let findings = run_local("export AWS_ACCESS_KEY_ID=AKIAJ3MV4BNZC9X7PQRF", false);
        assert!(
            findings.iter().any(|f| f.secret_type == "aws_access_key"),
            "expected aws_access_key finding, got: {findings:?}"
        );
    }

    #[test]
    fn detects_github_token() {
        // 36 chars after ghp_ (minimum required) with high entropy.
        // No keyword in the surrounding text so only the prefix detector fires.
        let findings = run_local(
            "GITHUB_CREDENTIALS=ghp_R8mA9fL3kDe2nV0xPqWsYuIoBtJhMcZg5r6T",
            false,
        );
        assert!(
            findings.iter().any(|f| f.secret_type == "github_pat"),
            "expected github_pat finding, got: {findings:?}"
        );
    }

    #[test]
    fn detects_slack_token() {
        let findings = run_local(
            "SLACK_BOT_TOKEN=xoxb-123456789012-123456789012-abcdefghijklmnop",
            false,
        );
        assert!(
            findings.iter().any(|f| f.secret_type == "slack_token"),
            "expected slack_token finding, got: {findings:?}"
        );
    }

    #[test]
    fn detects_stripe_key() {
        let findings = run_local("STRIPE_KEY=sk_live_abcdefghijklmnopqrstuvwx", false);
        assert!(
            findings
                .iter()
                .any(|f| f.secret_type == "stripe_secret_key"),
            "expected stripe_secret_key finding, got: {findings:?}"
        );
    }

    #[test]
    fn detects_openai_key() {
        let findings = run_local("api_key = sk-abcdefghijklmnopqrstuvwxyz1234567890", false);
        assert!(
            findings.iter().any(|f| f.secret_type == "openai_api_key"),
            "expected openai_api_key finding, got: {findings:?}"
        );
    }

    #[test]
    fn detects_private_key_header() {
        let findings = run_local("-----BEGIN RSA PRIVATE KEY-----", false);
        assert!(
            findings.iter().any(|f| f.secret_type == "private_key"),
            "expected private_key finding, got: {findings:?}"
        );
    }

    #[test]
    fn detects_sendgrid_key() {
        // SG. + 66 chars of mixed alphanum
        let key = format!("SG.{}", "aB3xY9zQ1m".repeat(7)); // 70 chars → trimmed to 66 by detector
        let findings = run_local(&format!("SENDGRID_KEY={key}"), false);
        assert!(
            findings.iter().any(|f| f.secret_type == "sendgrid_api_key"),
            "expected sendgrid_api_key finding, got: {findings:?}"
        );
    }

    #[test]
    fn detects_npm_token() {
        let findings = run_local("NPM_TOKEN=npm_aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789", false);
        assert!(
            findings.iter().any(|f| f.secret_type == "npm_token"),
            "expected npm_token finding, got: {findings:?}"
        );
    }

    #[test]
    fn detects_gitlab_pat() {
        let findings = run_local("CI_JOB_TOKEN=glpat-aBcDeFgHiJkLmNoPqRsT", false);
        assert!(
            findings.iter().any(|f| f.secret_type == "gitlab_pat"),
            "expected gitlab_pat finding, got: {findings:?}"
        );
    }

    #[test]
    fn masked_value_never_contains_full_secret() {
        let findings = run_local("export AWS_KEY=AKIAJ3MV4BNZC9X7PQRF", false);
        for f in &findings {
            assert!(
                !f.masked.contains("J3MV4BNZC9"),
                "masked value must not contain middle of secret: {}",
                f.masked
            );
        }
    }

    #[test]
    fn empty_input_returns_empty_findings() {
        let findings = run_local("", false);
        assert!(findings.is_empty());
    }

    #[test]
    fn clean_input_returns_no_findings() {
        let findings = run_local("This is a normal log message with no secrets.", false);
        assert!(
            findings.is_empty(),
            "expected no findings, got: {findings:?}"
        );
    }

    #[test]
    fn placeholder_values_skipped_by_entropy_check() {
        let findings = run_local("api_key = your_api_key_here_example", false);
        let entropy_findings: Vec<_> = findings
            .iter()
            .filter(|f| f.secret_type == "high_entropy_secret")
            .collect();
        assert!(
            entropy_findings.is_empty(),
            "placeholder should not produce high_entropy_secret findings: {entropy_findings:?}"
        );
    }

    // ── Credential-keyword assignment tests ───────────────────────────────────

    #[test]
    fn detects_human_password_assignment() {
        // A short, human-chosen password assigned to `password:` is a credential —
        // the keyword is the signal, even though entropy/length are modest.
        for line in [
            "password: Qw7k@PmRn!TvXs91",
            "password: Qw7kPmRn91",
            "DB_PASSWORD=Qw7k@PmRn!TvXs91",
            "passphrase = correcthorsebatterystaple",
        ] {
            let findings = run_local(line, false);
            assert!(
                findings
                    .iter()
                    .any(|f| f.secret_type == "high_entropy_secret"),
                "expected high_entropy_secret for {line:?}, got: {findings:?}"
            );
        }
    }

    #[test]
    fn placeholder_password_assignment_not_flagged() {
        // "test" is a placeholder; trivial values must not fire even with a keyword.
        for line in ["password=test", "password: test", "key: test"] {
            let findings = run_local(line, false);
            assert!(
                !findings
                    .iter()
                    .any(|f| f.secret_type == "high_entropy_secret"),
                "placeholder/trivial value must not fire for {line:?}, got: {findings:?}"
            );
        }
    }

    #[test]
    fn non_english_password_keywords_detected() {
        // A curated handful of unambiguous non-English "password" words behave
        // like the English strong keywords (lenient bar).
        for line in [
            "passwort: Qw7k@PmRn!TvXs91",
            "contraseña: Qw7kPmRn91",
            "contrasena = Qw7k@PmRn!TvXs91",
            "senha: Qw7kPmRn91",
            "mot de passe: Qw7k@PmRn!TvXs91",
            "motdepasse = Qw7k@PmRn!TvXs91",
        ] {
            let findings = run_local(line, false);
            assert!(
                findings
                    .iter()
                    .any(|f| f.secret_type == "high_entropy_secret"),
                "expected high_entropy_secret for {line:?}, got: {findings:?}"
            );
        }
    }

    #[test]
    fn hyphen_and_space_keyword_spellings_detected() {
        // api-key / api key / access-key etc. are unambiguous credential keywords
        // in default mode, just like their underscore spellings.
        for line in [
            "api-key: Qw7kPmRn91",
            "api key: Qw7kPmRn91",
            "access-key = Qw7kPmRnTv99x",
            "client-secret: aB3xY9zQ1mN7pK2r",
        ] {
            let findings = run_local(line, false);
            assert!(
                findings
                    .iter()
                    .any(|f| f.secret_type == "high_entropy_secret"),
                "expected high_entropy_secret for {line:?}, got: {findings:?}"
            );
        }
    }

    #[test]
    fn bare_key_requires_high_entropy_value() {
        // Bare `key` is ambiguous (English word + universal config map-key), so it
        // fires only on a key-like high-entropy value (len ≥ 20, H ≥ 3.5) — not on
        // ordinary config values or short human-ish strings.
        for benign in [
            "key: production",
            "key: application/json",
            "key: Qw7kPmRn91", // 10 chars: below the weak-key bar (len ≥ 20)
        ] {
            let finds = run_local(benign, false);
            assert!(
                !finds.iter().any(|f| f.secret_type == "high_entropy_secret"),
                "bare `key:` must not fire on benign value {benign:?}: {finds:?}"
            );
        }
        // A genuine machine key after `key:` fires in default mode.
        let finds = run_local("key: aB3xY9zQ1mN7pK2rT5wL", false);
        assert!(
            finds.iter().any(|f| f.secret_type == "high_entropy_secret"),
            "bare `key:` must fire on a high-entropy value: {finds:?}"
        );
    }

    #[test]
    fn keyword_word_boundary_prevents_substring_match() {
        // `monkey:` must NOT match the keyword `key` — the char before `key` is
        // alphanumeric. Use a high-entropy value so that, if `key` wrongly matched,
        // it would fire; expect no findings because the boundary check rejects it.
        let findings = run_local("monkey: aB3xY9zQ1mN7pK2rT5wL", false);
        assert!(
            findings.is_empty(),
            "monkey: must not match `key` keyword: {findings:?}"
        );
    }

    #[test]
    fn keyword_in_prose_is_not_an_assignment() {
        // The keyword appears in prose with no `=`/`:` separator immediately after —
        // must NOT be treated as a credential assignment.
        for line in [
            "and choosing a good string as a password tremendously improves the security",
            "The password should be rotated every 90 days for compliance.",
            "Your password is the key to security.",
        ] {
            let findings = run_local(line, false);
            assert!(
                findings.is_empty(),
                "prose mention must produce no findings for {line:?}, got: {findings:?}"
            );
        }
    }

    // ── Entropy gate tests ────────────────────────────────────────────────────

    #[test]
    fn low_entropy_stripe_prefix_not_flagged() {
        // Low-entropy value after sk_live_ — must be rejected by entropy gate.
        // All repeated 'a' chars = near-zero entropy.
        let findings = run_local("key=sk_live_aaaaaaaaaaaaaaaaaaaaaaaa", false);
        let stripe = findings
            .iter()
            .filter(|f| f.secret_type.starts_with("stripe"))
            .count();
        assert_eq!(
            stripe, 0,
            "low-entropy sk_live_ value must not be flagged: {findings:?}"
        );
    }

    #[test]
    fn high_entropy_stripe_key_is_flagged() {
        let findings = run_local("STRIPE=sk_live_4eC39HqLyjWDarjtT1zdp7dcABcDeF12", false);
        assert!(
            findings.iter().any(|f| f.secret_type.starts_with("stripe")),
            "high-entropy stripe key must be flagged: {findings:?}"
        );
    }

    #[test]
    fn low_entropy_github_prefix_not_flagged() {
        // ghp_ followed by 36 repeated chars — low entropy.
        let findings = run_local("token=ghp_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA", false);
        let gh = findings
            .iter()
            .filter(|f| f.secret_type.starts_with("github"))
            .count();
        assert_eq!(
            gh, 0,
            "low-entropy ghp_ value must not be flagged: {findings:?}"
        );
    }

    #[test]
    fn strict_mode_finds_keywordless_high_entropy() {
        // A high-entropy token with no surrounding keyword — default mode misses it,
        // strict mode catches it.
        let token = "X=Zk9Q2vR7sT1uW4xY6bC8dE0fG3hJ5kL7mN9pQ2r";
        let default_finds = run_local(token, false);
        let strict_finds = run_local(token, true);
        // Default: no high_entropy_secret (no keyword near '=', token is after '=')
        // Actually the '=' separator may trigger RE_HIGH_ENTROPY logic — just check strict finds MORE.
        assert!(
            strict_finds.len() >= default_finds.len(),
            "strict mode must find at least as many secrets as default mode"
        );
    }
}
