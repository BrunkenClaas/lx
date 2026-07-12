#![forbid(unsafe_code)]

/// Shannon entropy in bits per byte.
pub fn shannon_entropy(s: &str) -> f64 {
    if s.is_empty() {
        return 0.0;
    }
    let mut freq = [0usize; 256];
    for b in s.bytes() {
        freq[b as usize] += 1;
    }
    let len = s.len() as f64;
    freq.iter()
        .filter(|&&c| c > 0)
        .map(|&c| {
            let p = c as f64 / len;
            -p * p.log2()
        })
        .sum()
}

/// Heuristic: is this value a typical placeholder / documentation example string?
///
/// Returns `true` when the value contains words that strongly suggest it is not
/// a real credential (e.g. "example", "your_token", "changeme").
pub fn looks_like_placeholder(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    // Ordered roughly by how often they appear in the wild; short-circuit on first hit.
    [
        "example",
        "placeholder",
        "your_",
        "your-",
        "yourtoken",
        "changeme",
        "replace",
        "todo",
        "fixme",
        "xxxxxxx",
        "0000000",
        "1111111",
        "aaaaaa",
        "test",
        "fake",
        "dummy",
        "sample",
        "demo",
        "insert_",
        "put_your",
        "_here",
        "here_",
    ]
    .iter()
    .any(|p| lower.contains(p))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entropy_random_string() {
        let h = shannon_entropy("aBcDeFgHiJkLmNoPqRsT");
        assert!(h > 3.0, "expected entropy > 3.0, got {h}");
    }

    #[test]
    fn entropy_repeated_chars() {
        let h = shannon_entropy("aaaaaaaaaaaaaaaaaaa");
        assert!(h < 0.1, "expected entropy near 0, got {h}");
    }

    #[test]
    fn entropy_empty() {
        assert_eq!(shannon_entropy(""), 0.0);
    }

    #[test]
    fn placeholder_detected() {
        assert!(looks_like_placeholder("your_api_key_here"));
        assert!(looks_like_placeholder(
            "EXAMPLE_TOKEN_1234567890123456789012"
        ));
        assert!(looks_like_placeholder("changeme"));
    }

    #[test]
    fn real_looking_value_not_placeholder() {
        assert!(!looks_like_placeholder("AKIAIOSFODNN7REAL1234"));
        assert!(!looks_like_placeholder(
            "ghp_aBcDeFgHiJkLmNoPqRsTuVwXyZ1234567890"
        ));
    }
}
