//! Text cut to a length in characters (RB-094-10), never inside a character: [`cut`] without a
//! marker, [`cut_ellipsis`] with `...` where the text was cut - both never longer than `max`.

/// The marker [`cut_ellipsis`] puts where it cut.
const ELLIPSIS: &str = "...";

/// `text`'s first `max` characters.
#[must_use]
pub fn cut(text: &str, max: usize) -> String {
    match text.char_indices().nth(max) {
        Some((end, _)) => text[..end].to_owned(),
        None => text.to_owned(),
    }
}

/// `text` when it has at most `max` characters, else its start and `...`, `max` characters
/// together (below three, the start alone).
#[must_use]
pub fn cut_ellipsis(text: &str, max: usize) -> String {
    if text.char_indices().nth(max).is_none() {
        return text.to_owned();
    }
    let marker = ELLIPSIS.len();
    if max < marker {
        return cut(text, max);
    }
    let mut out = cut(text, max - marker);
    out.push_str(ELLIPSIS);
    out
}

#[cfg(test)]
#[path = "text_tests.rs"]
mod tests;
