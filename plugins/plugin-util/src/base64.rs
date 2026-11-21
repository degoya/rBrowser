//! Standard base64 with padding (RFC 4648), for the images a panel shows as `data:` URIs - the
//! panel's frame has no network (the PWA check's icons, the SEO check's share image). Once the
//! copies of `plugins/pwa-check` and `plugins/seo` (RB-094-11).

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// `bytes` in base64.
#[must_use]
pub fn encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let triple = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let bits =
            (u32::from(triple[0]) << 16) | (u32::from(triple[1]) << 8) | u32::from(triple[2]);
        for index in 0..4 {
            if index <= chunk.len() {
                let six = (bits >> (18 - 6 * index)) & 0x3F;
                out.push(char::from(ALPHABET[six as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[cfg(test)]
#[path = "base64_tests.rs"]
mod tests;
