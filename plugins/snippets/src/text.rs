//! Small text helpers.

/// `text` cut to at most `max` characters.
#[must_use]
pub fn cut(text: &str, max: usize) -> String {
    text.chars().take(max).collect()
}
