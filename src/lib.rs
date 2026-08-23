pub mod ai;
pub mod api;

/// Truncate a string to at most `max_bytes` bytes without splitting a multibyte
/// UTF-8 character. Always returns a valid `&str`.
pub fn safe_truncate(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut b = max_bytes;
    while !s.is_char_boundary(b) {
        b -= 1;
    }
    &s[..b]
}
pub mod cli;
pub mod errors;
pub mod features;
pub mod graph;
pub mod inference;
pub mod licensing;
pub mod mcp;
pub mod models;
pub mod parser;
pub mod storage;
pub mod sync;
#[cfg(feature = "webui")]
pub mod web;

#[cfg(test)]
mod tests {
    use super::safe_truncate;

    #[test]
    fn safe_truncate_ascii_within_limit() {
        assert_eq!(safe_truncate("hello", 10), "hello");
    }

    #[test]
    fn safe_truncate_ascii_at_limit() {
        assert_eq!(safe_truncate("hello", 5), "hello");
    }

    #[test]
    fn safe_truncate_ascii_over_limit() {
        assert_eq!(safe_truncate("hello world", 5), "hello");
    }

    #[test]
    fn safe_truncate_em_dash_boundary() {
        // '–' (en dash) is 3 bytes (U+2013, bytes E2 80 93)
        let s = "a".repeat(199) + "–"; // 199 + 3 = 202 bytes
        let result = safe_truncate(&s, 200);
        assert!(result.len() <= 200);
        assert!(result.is_char_boundary(result.len()));
        // Should truncate to 199 bytes (the 'a' chars), not split the dash
        assert_eq!(result.len(), 199);
    }

    #[test]
    fn safe_truncate_multibyte_chars_no_panic() {
        let s = "em\u{2013}dash en\u{2014}dash arrow\u{2192} quotes\u{201C}\u{201D} CJK\u{4E2D}\u{6587} done";
        for max in 0..=s.len() + 5 {
            let result = safe_truncate(s, max);
            assert!(result.len() <= max);
            // Verify valid UTF-8 by iterating chars
            let _ = result.chars().count();
        }
    }

    #[test]
    fn safe_truncate_zero() {
        assert_eq!(safe_truncate("hello", 0), "");
    }

    #[test]
    fn safe_truncate_empty_string() {
        assert_eq!(safe_truncate("", 10), "");
    }

    #[test]
    fn safe_truncate_cjk_boundary() {
        // '中' is 3 bytes (U+4E2D)
        let s = "中中中"; // 9 bytes
        assert_eq!(safe_truncate(s, 9), "中中中");
        assert_eq!(safe_truncate(s, 8), "中中"); // 6 bytes
        assert_eq!(safe_truncate(s, 7), "中中");
        assert_eq!(safe_truncate(s, 6), "中中");
        assert_eq!(safe_truncate(s, 5), "中"); // 3 bytes
        assert_eq!(safe_truncate(s, 3), "中");
        assert_eq!(safe_truncate(s, 2), "");
        assert_eq!(safe_truncate(s, 1), "");
    }
}
