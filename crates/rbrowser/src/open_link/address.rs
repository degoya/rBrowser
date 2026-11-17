//! Which addresses `app.open_link` opens: a web address and nothing else - `http:` or `https:`,
//! a host, no user name or password (a link that hides its real host behind `user@`), no
//! control or white space, at most [`MAX_URL_CHARS`]. The address that opens is the parsed,
//! normalised one (`url` crate), never the text as given.

/// The longest address that opens.
pub const MAX_URL_CHARS: usize = 2048;

/// Why an address does not open; each is an i18n key the status bar shows as a sentence.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum Refusal {
    /// Not `http:` or `https:` (`file:`, `javascript:`, `rbrowser:`, a program's own scheme).
    #[error("links.error.scheme")]
    Scheme,
    /// No address at all, too long, with credentials or characters no address has.
    #[error("links.error.invalid")]
    Invalid,
}

impl Refusal {
    /// The i18n key of the sentence.
    pub fn key(self) -> &'static str {
        match self {
            Self::Scheme => "links.error.scheme",
            Self::Invalid => "links.error.invalid",
        }
    }
}

/// `text` as the address to open, or why it does not.
pub fn checked(text: &str) -> Result<url::Url, Refusal> {
    if text.is_empty()
        || text.chars().count() > MAX_URL_CHARS
        || text.chars().any(|c| c.is_control() || c.is_whitespace())
    {
        return Err(Refusal::Invalid);
    }
    let parsed = url::Url::parse(text).map_err(|_| Refusal::Invalid)?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(Refusal::Scheme);
    }
    if parsed.host_str().is_none_or(str::is_empty)
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.as_str().chars().count() > MAX_URL_CHARS
    {
        return Err(Refusal::Invalid);
    }
    Ok(parsed)
}

#[cfg(test)]
#[path = "address_tests.rs"]
mod tests;
