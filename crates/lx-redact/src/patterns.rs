#![forbid(unsafe_code)]

//! Compiled regex patterns for secret and PII detection.
//!
//! All patterns are compiled once at first use via `once_cell::sync::Lazy`.
//!
//! Two pattern types:
//!
//! `RedactPattern` — used for Standard/Strict patterns.  Carries a `replace_whole`
//! flag and an optional `min_entropy` floor.  When `min_entropy > 0.0` the captured
//! secret value must have Shannon entropy ≥ that floor AND must not look like a
//! placeholder, otherwise the match is left untouched.  This prevents false positives
//! where a document prefix (e.g. `sk_live_`, `ghp_`) appears in low-entropy prose or
//! documentation examples.  Entropy floors mirror gitleaks defaults (2.0–4.0 per format).
//!
//! `GatedPattern` — used for Aggressive patterns (whole-match only, always gated).

use once_cell::sync::Lazy;
use regex::Regex;

pub struct RedactPattern {
    pub regex: &'static Lazy<Regex>,
    /// Replacement string for the whole match or for the last capture group.
    pub replacement: &'static str,
    /// When true, replace the full match. When false, replace the last capture group.
    pub replace_whole: bool,
    /// Minimum Shannon entropy of the captured secret value.
    /// 0.0 means no entropy gate (used for PEM blocks, JWTs, conn strings — their
    /// structure is already very specific).
    pub min_entropy: f64,
}

pub struct GatedPattern {
    pub regex: &'static Lazy<Regex>,
    pub replacement: &'static str,
    /// Minimum Shannon entropy of the captured value.
    pub min_entropy: f64,
}

// ── Standard patterns — prefix / format-specific ──────────────────────────────

// Anthropic key: sk-ant- followed by 20+ alphanumeric/dash chars.
// Must run before the generic sk- pattern.
// Entropy floor 3.0 (gitleaks: anthropic-api-key rule).
static RE_ANTHROPIC_KEY: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?:^|[^A-Za-z0-9\-])(sk-ant-[A-Za-z0-9\-]{20,})").unwrap());

// Anthropic new-format keys: sk-ant-api03- and sk-ant-admin01- (93 chars + AA suffix).
// Covered by RE_ANTHROPIC_KEY above since both start with sk-ant-.

// OpenAI key: sk- at a word boundary, followed by 20+ alphanumeric chars.
// Anthropic pattern runs first so sk-ant- keys are already replaced before this.
// Entropy floor 3.0 (gitleaks: openai-api-key rule).
static RE_OPENAI_KEY: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?:^|[^A-Za-z0-9\-])(sk-[A-Za-z0-9\-]{20,})").unwrap());

// OpenAI project/service/admin keys: sk-proj-/sk-svcacct-/sk-admin- + 58–74 chars.
// Their distinctive prefixes make them unambiguous without keyword context.
// Entropy floor 3.0.
static RE_OPENAI_PROJECT_KEY: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b(sk-(?:proj|svcacct|admin)-[A-Za-z0-9_\-]{58,74})\b").unwrap());

// AWS access key ID: AKIA/ASIA/ABIA/ACCA/A3T + 16–20 uppercase alphanumeric chars.
// Use {16,} to handle both 20-char standard keys and longer variants; the \b at
// start prevents matching inside longer identifiers.
// Entropy floor 3.0 (gitleaks: aws-access-key rule).
static RE_AWS_ACCESS_KEY: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b((?:AKIA|ASIA|ABIA|ACCA|A3T)[0-9A-Z]{16,})\b").unwrap());

// GCP API key: AIza followed by 35+ alphanumeric/dash/underscore chars.
// Entropy floor 4.0 (gitleaks: gcp-api-key — highest floor because AIza is common text).
static RE_GCP_KEY: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b(AIza[0-9A-Za-z\-_]{35,})\b").unwrap());

// GitHub tokens: ghp_ (PAT), gho_ (OAuth), ghu_ (user-to-server), ghs_ (server-to-server),
// ghr_ (refresh) — all followed by 36+ alphanumeric chars (gitleaks says exactly 36;
// use {36,} to be lenient with extended tokens from newer GitHub versions).
// Entropy floor 3.0 (gitleaks: github-* rules).
static RE_GITHUB_TOKEN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b(gh[pousr]_[A-Za-z0-9]{36,})\b").unwrap());

// GitHub fine-grained PAT: github_pat_ + 82+ word chars.
// Entropy floor 3.0.
static RE_GITHUB_FINE_GRAINED: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b(github_pat_\w{82,})\b").unwrap());

// GitLab PAT: glpat- + 20+ word/dash chars.
// Entropy floor 3.0 (gitleaks: gitlab-pat rule).
static RE_GITLAB_PAT: Lazy<Regex> = Lazy::new(|| Regex::new(r"\b(glpat-[\w\-]{20,})\b").unwrap());

// Slack bot / user / app tokens: xoxb-/xoxp-/xoxe-/xoxa-/xoxr-/xoxs- + numeric-dash segments.
// Structure: xox<type>-<10-13 digits>-<10-13+ alphanumeric>.
// Entropy floor 2.0 (gitleaks uses 2.0 for xoxp; 3.0 for xoxb).
static RE_SLACK_TOKEN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b(xox[bpears]-[0-9]{10,13}-[0-9A-Za-z\-]{10,})\b").unwrap());

// Slack webhook URL: contains /services/T.../B.../...
// Only the path segment is secret; we match the full URL for context but gate
// on the last path component's entropy.
static RE_SLACK_WEBHOOK: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(https?://hooks\.slack\.com/services/T[A-Za-z0-9_]{8,}/B[A-Za-z0-9_]{8,}/[A-Za-z0-9+/]{24,})").unwrap()
});

// Stripe secret and restricted keys: sk_live_, sk_test_, sk_prod_, rk_live_, rk_test_,
// rk_prod_ + 24+ alphanumeric.
// Entropy floor 2.0 (gitleaks: stripe rules).
static RE_STRIPE_KEY: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b([sr]k_(?:live|test|prod)_[A-Za-z0-9]{24,})\b").unwrap());

// SendGrid API key: SG. + 66 chars (a-z0-9=_\-.).
// Very specific shape; entropy floor 2.0.
static RE_SENDGRID_KEY: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b(SG\.[a-zA-Z0-9=_\-\.]{66})\b").unwrap());

// Twilio API key: SK + 32 hex chars (0-9a-fA-F).
// Entropy floor 3.0 (gitleaks: twilio rule). Note: "SK" alone is too short; the
// 32-hex constraint is tight enough that false positives are very unlikely.
static RE_TWILIO_KEY: Lazy<Regex> = Lazy::new(|| Regex::new(r"\b(SK[0-9a-fA-F]{32})\b").unwrap());

// npm access token: npm_ + 36 lowercase alphanumeric.
// Entropy floor 2.0 (gitleaks: npm rule).
static RE_NPM_TOKEN: Lazy<Regex> = Lazy::new(|| Regex::new(r"\b(npm_[a-zA-Z0-9]{36})\b").unwrap());

// Generic context-keyed secrets: (secret|token|key|password|...) = <value>
// Replaces only the value (last capture group), preserving the key name.
// No additional entropy gate here — the value shape gate inside the regex
// already requires digit-or-length-or-prefix, which provides structural gating.
// Setting min_entropy=0.0 skips the extra gate for this pattern.
static RE_CONTEXT_SECRET: Lazy<Regex> = Lazy::new(|| {
    const VC: &str = r"A-Za-z0-9+/\-_.@!#$%^&*~";
    const KEYWORDS: &str = concat!(
        "secret|token|credential|passphrase|password|passwd|pwd|",
        "secret[_-]?key|private[_-]?key|encryption[_-]?key|signing[_-]?key|",
        "access[_-]?key|public[_-]?key|ssh[_-]?key|api[_-]?key|apikey|",
        "client[_-]?secret|client[_-]?id|consumer[_-]?secret|consumer[_-]?key|",
        "app[_-]?secret|",
        "authorization|auth|bearer|session[_-]?token|refresh[_-]?token|",
        "access[_-]?token|id[_-]?token|oauth|totp|otp|mfa|pat|pin|",
        "aws[_-]?secret|aws[_-]?access|azure[_-]?key|gcp[_-]?key|sas[_-]?token|",
        "connection[_-]?string|conn[_-]?str|",
        "github[_-]?token|gitlab[_-]?token|npm[_-]?token|pypi[_-]?token|",
        "docker[_-]?password|slack[_-]?token|stripe[_-]?key|twilio[_-]?token|",
        "sendgrid[_-]?key|webhook[_-]?secret|",
        "database[_-]?url|db[_-]?pass|dsn|",
        "salt|nonce|cookie|csrf|xsrf|license[_-]?key|activation[_-]?key",
    );
    Regex::new(&format!(
        r#"(?i)(?:{KEYWORDS})[^\n\r]{{0,5}}(?:=|:|\s)\s*['"]?((?:[{VC}]{{16,}}={{0,2}})|(?:[{VC}]*[0-9][{VC}]{{7,}}={{0,2}})|(?:sk-|pk-|ghp_|gho_|ghs_|xox[baprs]-|ey)[{VC}]{{6,}})['"]?"#
    ))
    .unwrap()
});

// Connection string password: postgres://user:PASS@host — replace PASS only.
// No entropy gate: the structural context (://user:…@) is already very specific.
static RE_CONN_STRING: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)(postgres|mysql|mongodb|redis|mssql|mariadb)://[^:]+:([^@\s]+)@").unwrap()
});

// PEM private key blocks. No entropy gate: the delimiter is unambiguous.
static RE_PRIVATE_KEY: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"-----BEGIN[^\n]*PRIVATE KEY-----[^-]*-----END[^\n]*PRIVATE KEY-----").unwrap()
});

// JWT: three base64url segments separated by dots; eyJ prefix is characteristic.
// Replace the payload (second segment) only. No entropy gate: triple-segment structure
// with eyJ prefix is already extremely specific.
static RE_JWT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(eyJ[A-Za-z0-9_-]+)\.(eyJ[A-Za-z0-9_-]+)\.([A-Za-z0-9_-]*)").unwrap()
});

// High-entropy base64 (40+ chars) after an assignment or colon separator,
// with mixed-case/digits. Entropy floor checked via the char-class constraint.
// min_entropy=0.0 — the regex already requires uppercase/digit presence.
static RE_HIGH_ENTROPY_B64: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?:=|:[ \t]*)([A-Za-z0-9+/]*[A-Z0-9+/][A-Za-z0-9+/]{38,}={0,2})").unwrap()
});

// High-entropy hex (40+ lowercase hex chars) after assignment.
static RE_HIGH_ENTROPY_HEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?:=|:[ \t]*)([0-9a-f]{40,})\b").unwrap());

// Email addresses.
static RE_EMAIL: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}\b").unwrap());

// ── Strict-only patterns (PII) ────────────────────────────────────────────────

static RE_IPV4: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\b(?:(?:25[0-5]|2[0-4]\d|[01]?\d\d?)\.){3}(?:25[0-5]|2[0-4]\d|[01]?\d\d?)\b")
        .unwrap()
});

static RE_HOSTNAME: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"\b(?:[A-Za-z0-9](?:[A-Za-z0-9\-]{0,61}[A-Za-z0-9])?\.)+(?:com|net|org|io|dev|internal|corp|local|cloud|app|ai)\b"
    ).unwrap()
});

static RE_LOCAL_PATH: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?:/home/[A-Za-z0-9_][A-Za-z0-9_\-]*|C:\\Users\\[^\\:\n\r]+)").unwrap()
});

// ── Aggressive-only patterns (niche services) ─────────────────────────────────

// Shopify access/private/custom tokens: shpat_/shppa_/shpca_ + 32 hex chars.
static RE_SHOPIFY_TOKEN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b(shp(?:at|pa|ca)_[a-fA-F0-9]{32})\b").unwrap());

// DigitalOcean PAT/OAuth: dop_v1_/doo_v1_ + 64 hex chars.
static RE_DIGITALOCEAN_TOKEN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b(do[po]_v1_[a-f0-9]{64})\b").unwrap());

// Hugging Face token: hf_ + 34 lowercase alphanumeric.
static RE_HUGGINGFACE_TOKEN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b(hf_[a-zA-Z0-9]{34})\b").unwrap());

// Linear API token: lin_api_ + 40 lowercase alphanumeric.
static RE_LINEAR_TOKEN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b(lin_api_[a-z0-9]{40})\b").unwrap());

// Postman API token: PMAK- + 24 hex + - + 34 hex.
static RE_POSTMAN_TOKEN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b(PMAK-[a-f0-9]{24}-[a-f0-9]{34})\b").unwrap());

// Doppler service token: dp.pt. + 43 lowercase alphanumeric.
static RE_DOPPLER_TOKEN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b(dp\.pt\.[a-z0-9]{43})\b").unwrap());

// Atlassian API token: ATATT3 + 186 alphanumeric/dash/underscore/equals.
static RE_ATLASSIAN_TOKEN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b(ATATT3[A-Za-z0-9_\-=]{186})\b").unwrap());

// Heroku API key v2: HRKU-AA + 58 alphanumeric/dash/underscore.
static RE_HEROKU_KEY: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b(HRKU-AA[0-9a-zA-Z_\-]{58})\b").unwrap());

// Cloudflare API token: 40 alphanumeric/dash/underscore (no unambiguous prefix —
// only included in Aggressive; requires keyword context here for safety).
// Wrapped in a keyword context to avoid matching arbitrary 40-char strings.
static RE_CLOUDFLARE_TOKEN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?i)(?:cloudflare[_-]?(?:api[_-]?)?(?:token|key))[^\n\r]{0,5}(?:=|:|\s)\s*['""]?([A-Za-z0-9_\-]{40})['""]?"#).unwrap()
});

// Square access token: sq0atp- / sq0csp- + 22–60 word/dash chars.
static RE_SQUARE_TOKEN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b(sq0(?:atp|csp)-[\w\-]{22,60})\b").unwrap());

// PyPI upload token: pypi-AgEIcHlwaS5vcmc prefix + 50+ word/dash chars.
static RE_PYPI_TOKEN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b(pypi-AgEIcHlwaS5vcmc[\w\-]{50,})\b").unwrap());

// GitLab runner registration token: glrt- + 20 word/dash chars.
static RE_GITLAB_RUNNER: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b(glrt-[0-9A-Za-z_\-]{20})\b").unwrap());

// Discord bot token: base64(user_id) + . + timestamp + . + hmac (64 hex chars total).
// Approximated as: 24+ alnum . 6+ alnum . 27+ alnum — distinctive triple-segment shape.
static RE_DISCORD_TOKEN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\b([A-Za-z0-9]{24,}\.[A-Za-z0-9_\-]{6,}\.[A-Za-z0-9_\-]{27,})\b").unwrap()
});

// Telegram bot token: <bot_id>:A<34 alnum/dash/underscore>.
static RE_TELEGRAM_BOT: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b([0-9]{5,16}:A[a-z0-9_\-]{34})\b").unwrap());

// ── Pattern tables ────────────────────────────────────────────────────────────

pub static STANDARD_PATTERNS: &[RedactPattern] = &[
    // Anthropic first — its sk-ant- prefix is a superset of the generic sk- prefix.
    RedactPattern {
        regex: &RE_ANTHROPIC_KEY,
        replacement: "[REDACTED]",
        replace_whole: false,
        min_entropy: 3.0,
    },
    // OpenAI project/service/admin keys (specific sub-prefixes, higher confidence).
    RedactPattern {
        regex: &RE_OPENAI_PROJECT_KEY,
        replacement: "[REDACTED]",
        replace_whole: false,
        min_entropy: 3.0,
    },
    // Generic sk- OpenAI/API key (runs after Anthropic so sk-ant- is already gone).
    RedactPattern {
        regex: &RE_OPENAI_KEY,
        replacement: "[REDACTED]",
        replace_whole: false,
        min_entropy: 3.0,
    },
    RedactPattern {
        regex: &RE_AWS_ACCESS_KEY,
        replacement: "[REDACTED]",
        replace_whole: false,
        min_entropy: 3.0,
    },
    RedactPattern {
        regex: &RE_GCP_KEY,
        replacement: "[REDACTED]",
        replace_whole: false,
        min_entropy: 4.0,
    },
    RedactPattern {
        regex: &RE_GITHUB_TOKEN,
        replacement: "[REDACTED]",
        replace_whole: false,
        min_entropy: 3.0,
    },
    RedactPattern {
        regex: &RE_GITHUB_FINE_GRAINED,
        replacement: "[REDACTED]",
        replace_whole: false,
        min_entropy: 3.0,
    },
    RedactPattern {
        regex: &RE_GITLAB_PAT,
        replacement: "[REDACTED]",
        replace_whole: false,
        min_entropy: 3.0,
    },
    RedactPattern {
        regex: &RE_SLACK_TOKEN,
        replacement: "[REDACTED]",
        replace_whole: false,
        min_entropy: 2.0,
    },
    RedactPattern {
        regex: &RE_SLACK_WEBHOOK,
        replacement: "[REDACTED]",
        replace_whole: true,
        min_entropy: 0.0, // structural URL context is already very specific
    },
    RedactPattern {
        regex: &RE_STRIPE_KEY,
        replacement: "[REDACTED]",
        replace_whole: false,
        min_entropy: 2.0,
    },
    RedactPattern {
        regex: &RE_SENDGRID_KEY,
        replacement: "[REDACTED]",
        replace_whole: false,
        min_entropy: 2.0,
    },
    RedactPattern {
        regex: &RE_TWILIO_KEY,
        replacement: "[REDACTED]",
        replace_whole: false,
        min_entropy: 3.0,
    },
    RedactPattern {
        regex: &RE_NPM_TOKEN,
        replacement: "[REDACTED]",
        replace_whole: false,
        min_entropy: 2.0,
    },
    RedactPattern {
        regex: &RE_PRIVATE_KEY,
        replacement: "[REDACTED]",
        replace_whole: true,
        min_entropy: 0.0,
    },
    RedactPattern {
        regex: &RE_JWT,
        replacement: "[REDACTED]",
        replace_whole: true,
        min_entropy: 0.0,
    },
    RedactPattern {
        regex: &RE_CONN_STRING,
        replacement: "[REDACTED]",
        replace_whole: false,
        min_entropy: 0.0,
    },
    // Context-keyed secrets: capture group 1 is the value.
    RedactPattern {
        regex: &RE_CONTEXT_SECRET,
        replacement: "[REDACTED]",
        replace_whole: false,
        min_entropy: 0.0, // value shape gate is inside the regex itself
    },
    RedactPattern {
        regex: &RE_HIGH_ENTROPY_B64,
        replacement: "[REDACTED]",
        replace_whole: false,
        min_entropy: 0.0,
    },
    RedactPattern {
        regex: &RE_HIGH_ENTROPY_HEX,
        replacement: "[REDACTED]",
        replace_whole: false,
        min_entropy: 0.0,
    },
    // Email last (lower risk, keep structural context).
    RedactPattern {
        regex: &RE_EMAIL,
        replacement: "[EMAIL]",
        replace_whole: true,
        min_entropy: 0.0,
    },
];

pub static STRICT_PATTERNS: &[RedactPattern] = &[
    RedactPattern {
        regex: &RE_IPV4,
        replacement: "[IP]",
        replace_whole: true,
        min_entropy: 0.0,
    },
    RedactPattern {
        regex: &RE_HOSTNAME,
        replacement: "[HOST]",
        replace_whole: true,
        min_entropy: 0.0,
    },
    RedactPattern {
        regex: &RE_LOCAL_PATH,
        replacement: "[PATH]",
        replace_whole: true,
        min_entropy: 0.0,
    },
];

pub static AGGRESSIVE_SECRET_PATTERNS: &[GatedPattern] = &[
    GatedPattern {
        regex: &RE_SHOPIFY_TOKEN,
        replacement: "[REDACTED]",
        min_entropy: 2.0,
    },
    GatedPattern {
        regex: &RE_DIGITALOCEAN_TOKEN,
        replacement: "[REDACTED]",
        min_entropy: 3.0,
    },
    GatedPattern {
        regex: &RE_HUGGINGFACE_TOKEN,
        replacement: "[REDACTED]",
        min_entropy: 2.0,
    },
    GatedPattern {
        regex: &RE_LINEAR_TOKEN,
        replacement: "[REDACTED]",
        min_entropy: 2.0,
    },
    GatedPattern {
        regex: &RE_POSTMAN_TOKEN,
        replacement: "[REDACTED]",
        min_entropy: 3.0,
    },
    GatedPattern {
        regex: &RE_DOPPLER_TOKEN,
        replacement: "[REDACTED]",
        min_entropy: 2.0,
    },
    GatedPattern {
        regex: &RE_ATLASSIAN_TOKEN,
        replacement: "[REDACTED]",
        min_entropy: 3.5,
    },
    GatedPattern {
        regex: &RE_HEROKU_KEY,
        replacement: "[REDACTED]",
        min_entropy: 4.0,
    },
    GatedPattern {
        regex: &RE_CLOUDFLARE_TOKEN,
        replacement: "[REDACTED]",
        min_entropy: 2.0,
    },
    GatedPattern {
        regex: &RE_SQUARE_TOKEN,
        replacement: "[REDACTED]",
        min_entropy: 2.0,
    },
    GatedPattern {
        regex: &RE_PYPI_TOKEN,
        replacement: "[REDACTED]",
        min_entropy: 3.0,
    },
    GatedPattern {
        regex: &RE_GITLAB_RUNNER,
        replacement: "[REDACTED]",
        min_entropy: 3.0,
    },
    GatedPattern {
        regex: &RE_DISCORD_TOKEN,
        replacement: "[REDACTED]",
        min_entropy: 3.0,
    },
    GatedPattern {
        regex: &RE_TELEGRAM_BOT,
        replacement: "[REDACTED]",
        min_entropy: 2.0,
    },
];
