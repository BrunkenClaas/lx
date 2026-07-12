#![forbid(unsafe_code)]

use lx_config::Config;
use lx_core::error::LxError;
use lx_llm::{inject_lang, parse_response, LlmClient, Request};
use serde::{Deserialize, Serialize};

pub const SYSTEM_TEMPLATE: &str = include_str!("../prompts/system.txt");
/// Max tokens: each item explanation can be ~200 tokens, allow up to 20 items.
const MAX_TOKENS: u32 = 2048;

/// A single permission item with its risk assessment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermItem {
    /// The permission string, e.g. "-rwxr-xr-x".
    pub perm: String,
    /// The filename or path.
    pub file: String,
    /// Short risk category: "world-writable", "suid", "world-executable", "standard", etc.
    pub risk: String,
    /// Human-readable explanation of what the permission means and its risks.
    pub explanation: String,
}

/// Output of `lxperm`.
#[derive(Debug, Serialize, Deserialize)]
pub struct Output {
    pub items: Vec<PermItem>,
}

impl Output {
    /// Render as human-readable plain text for stdout.
    /// The explanation IS the result for this tool, so it goes to stdout.
    pub fn to_plain(&self) -> String {
        let mut out = String::new();
        for item in &self.items {
            out.push_str(&format!(
                "{}  [{}]  risk: {}\n",
                item.file, item.perm, item.risk
            ));
            // Indent explanation lines for readability.
            for line in item.explanation.lines() {
                out.push_str(&format!("  {line}\n"));
            }
            out.push('\n');
        }
        out
    }
}

/// Parse ls -l output lines and extract (perm_string, filename) pairs.
///
/// Handles the standard ls -l format:
///   -rwxr-xr-x  1 user group  1234 Jan  1 12:00 filename
///   drwxr-xr-x  2 user group  4096 Jan  1 12:00 dirname
///   lrwxrwxrwx  1 user group     7 Jan  1 12:00 link -> target
///
/// Fields (whitespace-separated): perm links owner group size month day time name
#[allow(dead_code)]
pub fn parse_ls_output(input: &str) -> Vec<(String, String)> {
    let mut items = Vec::new();
    for line in input.lines() {
        let trimmed = line.trim();
        // Skip blank lines and "total N" header lines.
        if trimmed.is_empty() || trimmed.starts_with("total ") {
            continue;
        }
        // Tokenise by whitespace.
        let tokens: Vec<&str> = trimmed.split_ascii_whitespace().collect();
        // Need at least 9 fields: perm links owner group size month day time name
        if tokens.len() < 9 {
            continue;
        }
        let perm = tokens[0];
        // Permission string must be exactly 10 chars.
        if perm.len() != 10 {
            continue;
        }
        // Validate first char is a known file-type indicator.
        match perm.chars().next() {
            Some('-' | 'd' | 'l' | 'c' | 'b' | 'p' | 's') => {}
            _ => continue,
        }
        // Name is the 9th token (index 8); join remaining tokens with space
        // to reconstruct filenames that may have spaces (uncommon in ls -l
        // but robust). For symlinks, strip " -> target".
        let name_raw = tokens[8..].join(" ");
        let name = if let Some(idx) = name_raw.find(" -> ") {
            name_raw[..idx].to_string()
        } else {
            name_raw
        };
        items.push((perm.to_string(), name));
    }
    items
}

/// Core logic for lxperm.
///
/// Pure function: no I/O, no process::exit. Testable with MockLlmClient.
pub fn run(input: &str, config: &Config, client: &dyn LlmClient) -> Result<Output, LxError> {
    if input.trim().is_empty() {
        return Err(LxError::BadUsage("no input provided".to_string()));
    }

    let system = inject_lang(SYSTEM_TEMPLATE, &config.output.lang);

    let req = Request {
        system: &system,
        user: input.trim(),
        max_tokens: MAX_TOKENS,
        temperature: 0.0,
        image: None,
    };

    let resp = client
        .complete(&req)
        .map_err(lx_core::error::LxError::from)?;

    parse_response::<Output>(&resp.content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ls_output_standard_file() {
        let ls = "-rwxr-xr-x  1 user group  1234 Jan  1 12:00 script.sh";
        let items = parse_ls_output(ls);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].0, "-rwxr-xr-x");
        assert_eq!(items[0].1, "script.sh");
    }

    #[test]
    fn parse_ls_output_skips_total_line() {
        let ls = "total 16\n-rw-r--r-- 1 user group 42 Jan 1 12:00 file.txt";
        let items = parse_ls_output(ls);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].1, "file.txt");
    }

    #[test]
    fn parse_ls_output_directory() {
        let ls = "drwxr-xr-x  2 root root 4096 Jan  1 12:00 etc";
        let items = parse_ls_output(ls);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].0, "drwxr-xr-x");
        assert_eq!(items[0].1, "etc");
    }

    #[test]
    fn parse_ls_output_symlink_strips_target() {
        let ls = "lrwxrwxrwx 1 user group 7 Jan 1 12:00 link -> target";
        let items = parse_ls_output(ls);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].1, "link");
    }

    #[test]
    fn parse_ls_output_empty_returns_empty() {
        let items = parse_ls_output("");
        assert!(items.is_empty());
    }

    #[test]
    fn to_plain_formats_correctly() {
        let out = Output {
            items: vec![PermItem {
                perm: "-rwxr-xr-x".to_string(),
                file: "script.sh".to_string(),
                risk: "world-executable".to_string(),
                explanation: "Owner can read/write/execute.".to_string(),
            }],
        };
        let plain = out.to_plain();
        assert!(plain.contains("script.sh"));
        assert!(plain.contains("-rwxr-xr-x"));
        assert!(plain.contains("world-executable"));
        assert!(plain.contains("Owner can read/write/execute."));
    }
}
