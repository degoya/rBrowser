//! The secrets of the MCP endpoint: the random bytes as lower-case hex and the comparison
//! without an early exit are `rb_cdp::secret`'s, the one random source and one comparison of
//! every endpoint (RB-080-28); HMAC-SHA256 for the probe's proof is this crate's own.

pub use rb_cdp::secret::{constant_time_eq, hex, random_hex};
use sha2::{Digest as _, Sha256};

/// The block size of SHA-256 in bytes.
const BLOCK: usize = 64;

/// HMAC-SHA256 of `message` under `key` (RFC 2104).
pub fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
    let mut block = [0u8; BLOCK];
    if key.len() > BLOCK {
        block[..32].copy_from_slice(&Sha256::digest(key));
    } else {
        block[..key.len()].copy_from_slice(key);
    }
    let pad = |byte: u8| block.map(|value| value ^ byte);
    let inner = Sha256::new()
        .chain_update(pad(0x36))
        .chain_update(message)
        .finalize();
    let outer = Sha256::new()
        .chain_update(pad(0x5c))
        .chain_update(inner)
        .finalize();
    let mut mac = [0u8; 32];
    mac.copy_from_slice(&outer);
    mac
}

#[cfg(test)]
#[path = "secret_tests.rs"]
mod tests;
