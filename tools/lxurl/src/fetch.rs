#![forbid(unsafe_code)]

use lx_core::exit::LxError;
use once_cell::sync::Lazy;
use regex::Regex;
use std::time::Duration;

pub const DEFAULT_MAX_URL_BYTES: usize = 512 * 1024;

/// Fetch `url` and extract plain text. Returns `(text, truncated)`.
///
/// Rejects SSRF targets (loopback, RFC-1918, link-local).
/// Strips `<script>`, `<style>`, `<nav>`, `<header>`, `<footer>` and all
/// remaining HTML tags. Normalises whitespace. Truncates at `max_bytes`.
pub fn fetch_and_extract(
    url: &str,
    max_bytes: usize,
    timeout_secs: u64,
) -> Result<(String, bool), LxError> {
    validate_url(url)?;

    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(timeout_secs))
        .build();

    let resp = agent
        .get(url)
        .call()
        .map_err(|e| LxError::NetworkLlm(format!("fetch failed: {e}")))?;

    // Read up to max_bytes of body.
    let body = {
        use std::io::Read;
        let mut buf = Vec::with_capacity(max_bytes.min(65_536));
        let mut reader = resp.into_reader();
        let mut chunk = [0u8; 8_192];
        let mut total = 0usize;
        let mut truncated = false;
        loop {
            match reader.read(&mut chunk) {
                Ok(0) => break,
                Ok(n) => {
                    let remaining = max_bytes.saturating_sub(total);
                    if n >= remaining {
                        buf.extend_from_slice(&chunk[..remaining]);
                        truncated = true;
                        break;
                    }
                    buf.extend_from_slice(&chunk[..n]);
                    total += n;
                }
                Err(e) => return Err(LxError::NetworkLlm(format!("read error: {e}"))),
            }
        }
        (String::from_utf8_lossy(&buf).into_owned(), truncated)
    };

    let text = strip_html(&body.0);
    Ok((text, body.1))
}

/// Reject loopback, RFC-1918, and link-local targets (SSRF protection).
pub fn validate_url(url: &str) -> Result<(), LxError> {
    let url_lower = url.to_lowercase();

    // Only allow http/https.
    if !url_lower.starts_with("http://") && !url_lower.starts_with("https://") {
        return Err(LxError::BadUsage(format!(
            "only http/https URLs are supported; got: {url}"
        )));
    }

    // Extract host (between :// and first / or end).
    let without_scheme = url
        .trim_start_matches("https://")
        .trim_start_matches("http://");
    let host = without_scheme
        .split('/')
        .next()
        .unwrap_or("")
        .split('@')
        .next_back()
        .unwrap_or("")
        .split(':')
        .next()
        .unwrap_or("");

    if is_private_host(host) {
        return Err(LxError::BadUsage(format!(
            "URL '{url}' resolves to a private/loopback address; \
             lxurl only fetches public URLs"
        )));
    }

    Ok(())
}

fn is_private_host(host: &str) -> bool {
    if host == "localhost" {
        return true;
    }
    // Check numeric IPv4 patterns.
    let parts: Vec<&str> = host.split('.').collect();
    if parts.len() == 4 {
        let octets: Option<Vec<u8>> = parts.iter().map(|p| p.parse::<u8>().ok()).collect();
        if let Some(octs) = octets {
            return is_private_ipv4(octs[0], octs[1], octs[2], octs[3]);
        }
    }
    false
}

fn is_private_ipv4(a: u8, b: u8, _c: u8, _d: u8) -> bool {
    a == 127                                         // loopback
    || a == 10                                       // RFC-1918 10.x.x.x
    || (a == 172 && (16..=31).contains(&b))          // RFC-1918 172.16-31.x.x
    || (a == 192 && b == 168)                        // RFC-1918 192.168.x.x
    || (a == 169 && b == 254)                        // link-local
    || a == 0 // "this" network
}

/// Strip HTML and normalise whitespace.
///
/// The `regex` crate does not support backreferences, so we strip each
/// known block tag individually using separate patterns.
pub fn strip_html(html: &str) -> String {
    // Strip block elements with their content (individually, no backrefs).
    static RE_SCRIPT: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"(?is)<script[^>]*>.*?</script\s*>").unwrap());
    static RE_STYLE: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"(?is)<style[^>]*>.*?</style\s*>").unwrap());
    static RE_NAV: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?is)<nav[^>]*>.*?</nav\s*>").unwrap());
    static RE_HEADER: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"(?is)<header[^>]*>.*?</header\s*>").unwrap());
    static RE_FOOTER: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"(?is)<footer[^>]*>.*?</footer\s*>").unwrap());
    static RE_TAGS: Lazy<Regex> = Lazy::new(|| Regex::new(r"<[^>]+>").unwrap());
    static RE_WS: Lazy<Regex> = Lazy::new(|| Regex::new(r"[ \t]{2,}").unwrap());
    static RE_NEWLINE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\n{3,}").unwrap());

    let s = RE_SCRIPT.replace_all(html, " ");
    let s = RE_STYLE.replace_all(&s, " ");
    let s = RE_NAV.replace_all(&s, " ");
    let s = RE_HEADER.replace_all(&s, " ");
    let s = RE_FOOTER.replace_all(&s, " ");
    let s = RE_TAGS.replace_all(&s, " ");
    // Decode a few common HTML entities.
    let s = s
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ");
    let s = RE_WS.replace_all(&s, " ");
    let s = RE_NEWLINE.replace_all(s.trim(), "\n\n");
    s.into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_removes_script_blocks() {
        let html = "<p>hello</p><script>alert(1)</script><p>world</p>";
        let text = strip_html(html);
        assert!(!text.contains("alert"), "script content should be removed");
        assert!(text.contains("hello"));
        assert!(text.contains("world"));
    }

    #[test]
    fn strip_removes_style_blocks() {
        let html = "<p>text</p><style>.a{color:red}</style>";
        let text = strip_html(html);
        assert!(!text.contains("color:red"));
        assert!(text.contains("text"));
    }

    #[test]
    fn strip_removes_nav_and_header() {
        let html = "<header>logo</header><main>content</main><nav>links</nav>";
        let text = strip_html(html);
        assert!(!text.contains("logo"));
        assert!(!text.contains("links"));
        assert!(text.contains("content"));
    }

    #[test]
    fn validate_url_rejects_localhost() {
        assert!(validate_url("http://localhost/path").is_err());
    }

    #[test]
    fn validate_url_rejects_loopback_ip() {
        assert!(validate_url("http://127.0.0.1/").is_err());
    }

    #[test]
    fn validate_url_rejects_rfc1918_10x() {
        assert!(validate_url("http://10.0.0.1/").is_err());
    }

    #[test]
    fn validate_url_rejects_rfc1918_192168() {
        assert!(validate_url("http://192.168.1.1/").is_err());
    }

    #[test]
    fn validate_url_rejects_rfc1918_172_16_31() {
        assert!(validate_url("http://172.16.0.1/").is_err());
        assert!(validate_url("http://172.31.255.254/").is_err());
        assert!(validate_url("http://172.32.0.1/").is_ok());
    }

    #[test]
    fn validate_url_rejects_link_local() {
        assert!(validate_url("http://169.254.1.1/").is_err());
    }

    #[test]
    fn validate_url_rejects_non_http_scheme() {
        assert!(validate_url("ftp://example.com/").is_err());
        assert!(validate_url("file:///etc/passwd").is_err());
    }

    #[test]
    fn validate_url_accepts_public_https() {
        assert!(validate_url("https://example.com/page").is_ok());
        assert!(validate_url("https://172.32.0.1/").is_ok());
    }

    #[test]
    fn strip_decodes_html_entities() {
        let html = "<p>a &amp; b &lt;3&gt;</p>";
        let text = strip_html(html);
        assert!(text.contains("a & b"), "got: {text}");
    }
}
