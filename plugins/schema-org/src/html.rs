//! The selector and text helpers the readers share, as `plugins/seo/src/page.rs` has them: a
//! selector of the crate's own, and text with its white space collapsed and cut.

use scraper::Selector;

/// The longest text kept of one value.
pub const MAX_TEXT: usize = 1024;

/// A selector of this crate's own; each is a constant, so a parse failure is a bug.
#[must_use]
pub fn sel(text: &str) -> Selector {
    Selector::parse(text).expect("a selector of the crate's own parses")
}

/// White space collapsed and the text cut to [`MAX_TEXT`] characters.
#[must_use]
pub fn squash(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(MAX_TEXT)
        .collect()
}

#[cfg(test)]
#[path = "html_tests.rs"]
mod tests;
