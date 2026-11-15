//! The one-time token of the internal DevTools endpoint: 32 bytes from the OS's random source
//! as hex, compared in constant time ([`crate::secret`]), and never printed (`Debug` shows a
//! placeholder).

use std::fmt;

/// The token's length in bytes.
const TOKEN_BYTES: usize = 32;

/// A secret that admits one connection.
#[derive(Clone, PartialEq, Eq)]
pub struct Token(String);

impl Token {
    /// A fresh token; `None` when the OS gives no randomness.
    pub fn generate() -> Option<Self> {
        crate::secret::random_hex(TOKEN_BYTES).ok().map(Self)
    }

    /// The token's text, for the frontend URL only.
    pub fn expose(&self) -> &str {
        &self.0
    }

    /// Whether `offered` is this token, in time independent of where they differ.
    pub fn matches(&self, offered: &str) -> bool {
        crate::secret::constant_time_eq(self.0.as_bytes(), offered.as_bytes())
    }
}

impl fmt::Debug for Token {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Token(<redacted>)")
    }
}

#[cfg(test)]
#[path = "token_tests.rs"]
mod tests;
