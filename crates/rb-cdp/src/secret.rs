//! The secrets of rBrowser's own endpoints - the CDP proxy's token (RB-030-06), the one-time
//! DevTools token (RB-010-06) and the instance nonce (RB-010-11): bytes from the OS's random
//! source as lower-case hex, compared in constant time. The one place for them, so all use one
//! random source and one comparison.

/// `bytes` random bytes from the OS's source as `2 * bytes` lower-case hex digits.
///
/// # Errors
///
/// The OS gives no randomness.
pub fn random_hex(bytes: usize) -> std::io::Result<String> {
    let mut buffer = vec![0u8; bytes];
    getrandom::fill(&mut buffer).map_err(|error| std::io::Error::other(error.to_string()))?;
    Ok(hex(&buffer))
}

/// Lower-case hex, two digits per byte (`rb_util::hex`), under the path the secrets always had.
pub use rb_util::hex;

/// Compares `a` and `b` without an early exit; only the length is learned from the time.
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |diff, (x, y)| diff | (x ^ y)) == 0
}

#[cfg(test)]
#[path = "secret_tests.rs"]
mod tests;
