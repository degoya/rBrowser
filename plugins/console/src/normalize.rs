//! The dedupe rule: what makes two messages the same message.
//!
//! A message's text is compared without its numbers and ids - the width a view logs, a time
//! stamp, a request counter, a UUID or a hash differ from view to view and run to run - and its
//! source without query and fragment (a cache buster). [`text`] keeps everything else, so
//! `Failed at 375` and `Failed at 1280` are one message, `Failed at 375` and `Failed in 375` two.

/// The stand-in for a run of digits.
pub const NUMBER: &str = "#";
/// The stand-in for an id: a UUID or a run of at least [`MIN_ID_CHARS`] hex digits with a digit.
pub const ID: &str = "<id>";
/// The shortest hex run that counts as an id.
pub const MIN_ID_CHARS: usize = 8;

/// `text` without numbers and ids, whitespace collapsed: the dedupe key's text.
///
/// The text is split into words of ASCII letters, digits and `-`; a word whose `-`-separated
/// parts are all hex, at least [`MIN_ID_CHARS`] hex digits long together and hold a digit, is an
/// id ([`ID`]); in any other word each run of digits is [`NUMBER`]. Everything between words is
/// kept, whitespace collapsed to one space.
#[must_use]
pub fn text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut word = String::new();
    let mut space = false;
    for c in text.chars() {
        if c.is_ascii_alphanumeric() || c == '-' {
            word.push(c);
            continue;
        }
        flush(&mut out, &mut word, &mut space);
        if c.is_whitespace() {
            space = true;
        } else {
            push_spaced(&mut out, &mut space);
            out.push(c);
        }
    }
    flush(&mut out, &mut word, &mut space);
    out
}

fn push_spaced(out: &mut String, space: &mut bool) {
    if *space && !out.is_empty() {
        out.push(' ');
    }
    *space = false;
}

fn flush(out: &mut String, word: &mut String, space: &mut bool) {
    if word.is_empty() {
        return;
    }
    push_spaced(out, space);
    if is_id(word) {
        out.push_str(ID);
    } else {
        let mut digits = false;
        for c in word.chars() {
            if c.is_ascii_digit() {
                if !digits {
                    out.push_str(NUMBER);
                }
                digits = true;
            } else {
                out.push(c);
                digits = false;
            }
        }
    }
    word.clear();
}

fn is_id(word: &str) -> bool {
    let hex: usize = word.chars().filter(char::is_ascii_hexdigit).count();
    word.split('-')
        .all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_hexdigit()))
        && hex >= MIN_ID_CHARS
        && word.chars().any(|c| c.is_ascii_digit())
}

/// A source URL as the key compares it: without its query and fragment.
#[must_use]
pub fn source_url(url: &str) -> &str {
    let end = url.find(['?', '#']).unwrap_or(url.len());
    &url[..end]
}

/// FNV-1a, 64 bit: the stable id of a group's key (the same in every call and on every machine).
#[must_use]
pub fn fnv1a(text: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in text.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// `text` cut to `max` characters, an ellipsis (`...`) marking the cut.
#[must_use]
pub fn cut(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_owned();
    }
    let mut out: String = text.chars().take(max.saturating_sub(3)).collect();
    out.push_str("...");
    out
}

#[cfg(test)]
#[path = "normalize_tests.rs"]
mod tests;
