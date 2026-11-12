use rand::RngCore;
use subtle::ConstantTimeEq;

/// 32 random bytes as 64 hex chars.
pub fn new_token() -> String {
    let mut b = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut b);
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// Constant-time compare; differing lengths return false immediately (length is not secret).
/// An empty `expected` never matches, so a blank configured token cannot fail open.
pub fn verify(expected: &str, given: &str) -> bool {
    !expected.is_empty() && expected.as_bytes().ct_eq(given.as_bytes()).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_is_64_hex_and_unique() {
        let a = new_token();
        assert_eq!(a.len(), 64);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, new_token());
    }

    #[test]
    fn verify_matches_only_equal_tokens() {
        assert!(verify("abc", "abc"));
        assert!(!verify("abc", "abd"));
        assert!(!verify("abc", "ab"));
        assert!(!verify("abc", ""));
        assert!(!verify("", ""));
    }
}
