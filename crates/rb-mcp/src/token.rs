//! The bearer token of the HTTP transport: 32 random bytes from the OS, new at every start of
//! the server, so a token read from an earlier run's instance file stops working. Its `Debug`
//! is redacted and it has no `Display`; [`McpToken::expose`] is the one way to the text, for
//! the `Authorization` header and the instance file. The token is registered with the log's
//! mask by the host (`rb_diag::register_secret`).

use std::fmt;

use crate::secret::{constant_time_eq, hex, hmac_sha256, random_hex};

/// The domain of the probe's proof, so the MAC cannot be confused with any other use of the
/// token.
const PROOF_DOMAIN: &[u8] = b"rbrowser mcp health proof v1";

/// The token.
#[derive(Clone, PartialEq, Eq)]
pub struct McpToken(String);

impl fmt::Debug for McpToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("McpToken([redacted])")
    }
}

impl McpToken {
    /// A new random token: 32 bytes as 64 hex digits.
    ///
    /// # Errors
    ///
    /// The OS gives no randomness.
    pub fn generate() -> std::io::Result<Self> {
        random_hex(32).map(Self)
    }

    /// A token from its text (the instance file, a test).
    #[must_use]
    pub fn from_text(text: impl Into<String>) -> Self {
        Self(text.into())
    }

    /// The token's text, for the `Authorization` header and the instance file.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }

    /// Whether `offered` is this token, compared in constant time.
    #[must_use]
    pub fn matches(&self, offered: &str) -> bool {
        constant_time_eq(self.0.as_bytes(), offered.as_bytes())
    }

    /// The answer to the health probe's challenge `nonce` on `port`, as 64 hex digits:
    /// HMAC-SHA256 under the token over a domain tag, the port and the nonce. It proves the
    /// token without sending it, for this port and nonce only, so whoever holds the port after a
    /// crash learns nothing and cannot pass on another server's answer.
    #[must_use]
    pub fn proof(&self, port: u16, nonce: &str) -> String {
        let mut message = Vec::with_capacity(PROOF_DOMAIN.len() + 2 + nonce.len());
        message.extend_from_slice(PROOF_DOMAIN);
        message.extend_from_slice(&port.to_be_bytes());
        message.extend_from_slice(nonce.as_bytes());
        hex(&hmac_sha256(self.0.as_bytes(), &message))
    }

    /// Whether `offered` is this token's proof for `port` and `nonce`, compared in constant
    /// time.
    #[must_use]
    pub fn proof_matches(&self, port: u16, nonce: &str, offered: &str) -> bool {
        constant_time_eq(self.proof(port, nonce).as_bytes(), offered.as_bytes())
    }
}

#[cfg(test)]
#[path = "token_tests.rs"]
mod tests;
