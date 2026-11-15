//! The CDP proxy's token: 32 bytes from the OS's random source as hex, new at every start and on
//! `cdp.rotate_token`. A client proves it as `Authorization: Bearer <token>` (Playwright's
//! `connectOverCDP(url, { headers })`) or as the first path segments `/t/<token>/...` (clients
//! without headers). Every comparison is constant-time; `Debug` shows a placeholder and there
//! is no `Display`, so the token reaches no log by accident.

use std::fmt;

use crate::secret::{constant_time_eq, random_hex};

/// The token's length in bytes.
pub const TOKEN_BYTES: usize = 32;
/// The path segment before a token in the path.
pub const PATH_SEGMENT: &str = "/t/";

/// The proxy's secret.
#[derive(Clone, PartialEq, Eq)]
pub struct ProxyToken(String);

impl ProxyToken {
    /// A fresh token.
    ///
    /// # Errors
    ///
    /// The OS gives no randomness.
    pub fn generate() -> std::io::Result<Self> {
        random_hex(TOKEN_BYTES).map(Self)
    }

    /// A token with the text `text` (tests, a token read back from the app's own state).
    #[must_use]
    pub fn from_text(text: String) -> Self {
        Self(text)
    }

    /// The token's text: only for an authenticated MCP client, the shell's copy command and
    /// the log's mask.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }

    /// The value of the `Authorization` header that proves it.
    #[must_use]
    pub fn bearer(&self) -> String {
        format!("Bearer {}", self.0)
    }

    /// `/t/<token>`: the path prefix that proves it.
    #[must_use]
    pub fn path_prefix(&self) -> String {
        format!("{PATH_SEGMENT}{}", self.0)
    }

    /// Whether `offered` is this token, in time independent of where they differ.
    #[must_use]
    pub fn matches(&self, offered: &str) -> bool {
        constant_time_eq(self.0.as_bytes(), offered.as_bytes())
    }
}

impl fmt::Debug for ProxyToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ProxyToken(<redacted>)")
    }
}

/// How a request proved the token.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Proof {
    /// `Authorization: Bearer <token>`.
    Header,
    /// `/t/<token>/...`; the answers' URLs carry the prefix again.
    Path,
}

/// An authorized request: its path without the token, and how it proved it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Authorized {
    /// The path the request asks for (`/json/version`), the token prefix removed.
    pub path: String,
    /// The proof.
    pub proof: Proof,
}

/// Whether a request for `path` with the `Authorization` header `authorization` proves
/// `token`: at least one proof, and every proof offered matches. `None` refuses (401).
#[must_use]
pub fn authorize(
    token: &ProxyToken,
    path: &str,
    authorization: Option<&str>,
) -> Option<Authorized> {
    let header = authorization.map(|value| bearer_value(value).is_some_and(|t| token.matches(t)));
    let in_path = path.strip_prefix(PATH_SEGMENT).map(|rest| {
        let (offered, rest) = rest.split_at(rest.find('/').unwrap_or(rest.len()));
        (
            token.matches(offered),
            if rest.is_empty() { "/" } else { rest },
        )
    });
    // Both proofs are compared before deciding, so the time tells nothing about either.
    match (header, in_path) {
        (None, None) | (Some(false), _) | (_, Some((false, _))) => None,
        (_, Some((true, rest))) => Some(Authorized {
            path: rest.to_owned(),
            proof: Proof::Path,
        }),
        (Some(true), None) => Some(Authorized {
            path: path.to_owned(),
            proof: Proof::Header,
        }),
    }
}

/// The credentials of `Bearer <credentials>`, the scheme in any case.
fn bearer_value(header: &str) -> Option<&str> {
    let (scheme, rest) = header.trim().split_once(' ')?;
    scheme
        .eq_ignore_ascii_case("bearer")
        .then(|| rest.trim())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
#[path = "token_tests.rs"]
mod tests;
