//! The generator behind every value: SplitMix64, seeded per run and keyed per field, so a field
//! gets the same value in every view of a run whatever the other fields are, and a seed repeats a
//! run exactly. Not for secrets: the values are test data.

/// FNV-1a, 64 bit.
#[must_use]
pub fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

/// A SplitMix64 generator.
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    /// The generator of `seed` keyed by `parts` (each part length-prefixed, so `["ab", "c"]` and
    /// `["a", "bc"]` differ).
    #[must_use]
    pub fn keyed(seed: u64, parts: &[&str]) -> Self {
        let mut bytes = Vec::new();
        for part in parts {
            bytes.extend_from_slice(&(part.len() as u64).to_le_bytes());
            bytes.extend_from_slice(part.as_bytes());
        }
        Self(seed ^ fnv1a(&bytes))
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// A number in `0..n` (0 for `n == 0`).
    pub fn below(&mut self, n: u64) -> u64 {
        if n == 0 { 0 } else { self.next_u64() % n }
    }

    /// A number in `lo..=hi`; `lo` when the range is empty.
    pub fn between(&mut self, lo: i64, hi: i64) -> i64 {
        if hi <= lo {
            return lo;
        }
        let span = hi.abs_diff(lo).saturating_add(1);
        lo.wrapping_add_unsigned(self.below(span))
    }

    /// One of `items`; `items` must not be empty.
    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        let at = usize::try_from(self.below(items.len() as u64)).unwrap_or(0);
        &items[at.min(items.len().saturating_sub(1))]
    }

    /// `count` decimal digits.
    pub fn digits(&mut self, count: usize) -> String {
        (0..count)
            .map(|_| char::from(b'0' + u8::try_from(self.below(10)).unwrap_or(0)))
            .collect()
    }
}

#[cfg(test)]
#[path = "rng_tests.rs"]
mod tests;
