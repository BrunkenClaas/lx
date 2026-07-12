use lx_config::Config;
use lx_core::error::LxError;
use lx_llm::{inject_lang, parse_response, LlmClient, Request};
use lx_redact::{redact, RedactLevel};
use serde::{Deserialize, Serialize};

pub const SYSTEM_TEMPLATE: &str = include_str!("../prompts/system.txt");
const MAX_TOKENS: u32 = 512;

/// Output of `lxjwt`.
#[derive(Debug, Serialize, Deserialize)]
pub struct Output {
    /// Description of the JWT header fields (algorithm, type).
    pub header: String,
    /// Description of the JWT payload claims (subject, issuer, expiry, etc.).
    pub payload: String,
    /// Observations and notes about the claims.
    pub notes: Vec<String>,
}

impl Output {
    /// Render as human-readable plain text (result field for stdout in plain mode).
    pub fn to_plain(&self) -> String {
        let mut out = format!("Header:  {}\nPayload: {}\n", self.header, self.payload);
        if !self.notes.is_empty() {
            out.push_str("Notes:\n");
            for note in &self.notes {
                out.push_str(&format!("  • {note}\n"));
            }
        }
        out
    }
}

/// Decode the base64url-encoded header or payload section of a JWT.
///
/// JWT uses base64url without padding. We decode it to a JSON string so the
/// LLM sees structured claim names and values, but never the raw token or
/// signature bytes.
fn decode_jwt_part(part: &str) -> Result<String, LxError> {
    // base64url → standard base64 (replace - with +, _ with /)
    let standard = part.replace('-', "+").replace('_', "/");
    // Add padding if needed
    let padded = match standard.len() % 4 {
        2 => format!("{standard}=="),
        3 => format!("{standard}="),
        _ => standard,
    };
    let bytes = base64_decode(&padded)
        .map_err(|e| LxError::LogicalError(format!("base64 decode failed: {e}")))?;
    String::from_utf8(bytes)
        .map_err(|e| LxError::LogicalError(format!("JWT part is not valid UTF-8: {e}")))
}

/// Minimal base64 decoder (standard alphabet, with padding).
/// We implement this manually because `base64` is not on the allowed crate list.
fn base64_decode(input: &str) -> Result<Vec<u8>, String> {
    // Build decode table
    let table: [u8; 128] = {
        let mut t = [255u8; 128];
        for (i, c) in b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"
            .iter()
            .enumerate()
        {
            t[*c as usize] = i as u8;
        }
        t
    };

    let input = input.trim_end_matches('=');
    let mut out = Vec::with_capacity(input.len() * 3 / 4);
    let bytes = input.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        let remaining = bytes.len() - i;
        let b0 = bytes[i] as usize;
        if b0 >= 128 || table[b0] == 255 {
            return Err(format!("invalid base64 char at position {i}"));
        }
        let v0 = table[b0] as u32;

        if remaining == 1 {
            // Shouldn't happen with valid base64 — ignore trailing bits
            break;
        }

        let b1 = bytes[i + 1] as usize;
        if b1 >= 128 || table[b1] == 255 {
            return Err(format!("invalid base64 char at position {}", i + 1));
        }
        let v1 = table[b1] as u32;

        out.push(((v0 << 2) | (v1 >> 4)) as u8);

        if remaining >= 3 {
            let b2 = bytes[i + 2] as usize;
            if b2 >= 128 || table[b2] == 255 {
                return Err(format!("invalid base64 char at position {}", i + 2));
            }
            let v2 = table[b2] as u32;
            out.push(((v1 << 4) | (v2 >> 2)) as u8);

            if remaining >= 4 {
                let b3 = bytes[i + 3] as usize;
                if b3 >= 128 || table[b3] == 255 {
                    return Err(format!("invalid base64 char at position {}", i + 3));
                }
                let v3 = table[b3] as u32;
                out.push(((v2 << 6) | v3) as u8);
            }
        }

        i += 4;
    }

    Ok(out)
}

/// Parse a JWT string into its three parts and return (header_json, payload_json).
///
/// The signature is intentionally discarded — it is never sent to the LLM.
pub fn split_and_decode_jwt(jwt: &str) -> Result<(String, String), LxError> {
    let jwt = jwt.trim();
    let parts: Vec<&str> = jwt.splitn(3, '.').collect();
    if parts.len() != 3 {
        return Err(LxError::BadUsage(
            "input does not look like a JWT (expected header.payload.signature)".to_string(),
        ));
    }
    let header_json = decode_jwt_part(parts[0])
        .map_err(|e| LxError::LogicalError(format!("could not decode JWT header: {e}")))?;
    let payload_json = decode_jwt_part(parts[1])
        .map_err(|e| LxError::LogicalError(format!("could not decode JWT payload: {e}")))?;
    Ok((header_json, payload_json))
}

/// Core logic for lxjwt — with mandatory redaction (redact flag).
///
/// The JWT token is decoded locally in Rust. Only the decoded JSON header and
/// payload are sent to the LLM. The signature and the raw JWT string itself are
/// never transmitted.
pub fn run(input: &str, config: &Config, client: &dyn LlmClient) -> Result<Output, LxError> {
    if input.trim().is_empty() {
        return Err(LxError::BadUsage(
            "no JWT provided; pipe or pass the JWT token as input".to_string(),
        ));
    }

    // Decode the JWT locally — signature is discarded here.
    let (header_json, payload_json) = split_and_decode_jwt(input.trim())?;

    // Build user content: header + payload JSON (no raw JWT, no signature).
    let user_content = format!("Header: {header_json}\nPayload: {payload_json}");

    // MANDATORY: redact before LLM. JWT payloads can contain embedded secrets.
    let level = RedactLevel::parse(&config.redact.level);
    let redacted = redact(&user_content, level)
        .map_err(|e| LxError::SecurityAbort(format!("redaction failed: {e}")))?;

    send_to_llm(&redacted, config, client)
}

/// Variant used when `--no-redact` is passed by the user.
///
/// Sends the decoded header/payload without additional redaction. The caller
/// is responsible for having already warned the user about the risk.
pub fn run_no_redact(
    input: &str,
    config: &Config,
    client: &dyn LlmClient,
) -> Result<Output, LxError> {
    if input.trim().is_empty() {
        return Err(LxError::BadUsage(
            "no JWT provided; pipe or pass the JWT token as input".to_string(),
        ));
    }

    let (header_json, payload_json) = split_and_decode_jwt(input.trim())?;
    let user_content = format!("Header: {header_json}\nPayload: {payload_json}");

    send_to_llm(&user_content, config, client)
}

/// Build and send the LLM request, parse and validate the response.
fn send_to_llm(
    user_content: &str,
    config: &Config,
    client: &dyn LlmClient,
) -> Result<Output, LxError> {
    let system = inject_lang(SYSTEM_TEMPLATE, &config.output.lang);

    let req = Request {
        system: &system,
        user: user_content,
        max_tokens: MAX_TOKENS,
        temperature: 0.0,
        image: None,
    };

    let resp = client
        .complete(&req)
        .map_err(lx_core::error::LxError::from)?;

    let out = parse_response::<Output>(&resp.content)?;

    if out.header.is_empty() {
        return Err(LxError::LogicalError(
            "model returned empty header description".to_string(),
        ));
    }
    if out.payload.is_empty() {
        return Err(LxError::LogicalError(
            "model returned empty payload description".to_string(),
        ));
    }

    Ok(out)
}
