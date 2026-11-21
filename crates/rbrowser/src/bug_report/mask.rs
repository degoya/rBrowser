//! The privacy half of the bug report (RB-093-04, review condition "high"): every text that goes
//! into a report passes [`Masker::text`] - the values known to be secret (the views' cookie
//! values and emulated header values), the value after a credential header's name in any form
//! (`Authorization: ...`, `"cookie":"..."`, `set-cookie=...`), then `rb_diag`'s rules (the
//! registered secrets, `secret=value`, `Bearer` values, a URL's user info, the credential URL
//! parameters) and last every address in the text: each query value, the fragment, the user
//! info and each path segment shaped like a token masked ([`mask_url`]). Request and response headers and bodies never reach a report at
//! all; a page's own text is masked, never trusted. Masking twice equals masking once.

use rb_diag::redact::{PLACEHOLDER, Secrets};

/// Header names whose value is a credential wherever the name stands (lower case).
const CREDENTIAL_NAMES: [&str; 4] = [
    "authorization",
    "proxy-authorization",
    "cookie",
    "set-cookie",
];

/// Characters that end an address inside free text.
const URL_STOPS: &[char] = &[
    '"', '\'', '`', '<', '>', '(', ')', '[', ']', '{', '}', '\\', '|',
];

/// A path segment piece this long or longer may be a token.
const TOKEN_MIN_LEN: usize = 16;

/// A piece of digits alone is an id up to this length, a token from it on.
const NUMBER_TOKEN_LEN: usize = 24;

/// A run of letters alone (no digit) is a token from this length on when its case is mixed like
/// random text: at least a quarter upper case and a quarter lower case (`CamelCaseWords` have
/// far fewer capitals).
const LETTERS_TOKEN_LEN: usize = 20;

/// Masks the texts of one report.
pub struct Masker {
    known: Secrets,
}

impl Masker {
    /// A masker that also hides each of `values` wherever it appears (values shorter than four
    /// characters are too common to hide and are left out, as `rb_diag` does).
    pub fn new<I, S>(values: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let known = Secrets::default();
        for value in values {
            known.add(value.as_ref());
        }
        Self { known }
    }

    /// `text` with every secret this module knows masked.
    pub fn text(&self, text: &str) -> String {
        let known = self.known.redact(text);
        let headers = mask_credential_values(&known);
        let rules = rb_diag::page_text::redact_page_text(&headers);
        mask_urls_in(&rules)
    }
}

/// `url` with each query value, the fragment, the user info and each path segment that looks
/// like a token masked: `?token=***&page=***`, a bare query item `***`, `#***`,
/// `https://***@host`, `/reset/***` ([`looks_like_token`]). The host stays.
pub fn mask_url(url: &str) -> String {
    let (rest, fragment) = match url.split_once('#') {
        Some((rest, fragment)) => (rest, Some(fragment)),
        None => (url, None),
    };
    let (base, query) = match rest.split_once('?') {
        Some((base, query)) => (base, Some(query)),
        None => (rest, None),
    };
    let mut out = mask_path_tokens(&mask_user_info(base));
    if let Some(query) = query {
        out.push('?');
        let items: Vec<String> = query.split('&').map(mask_query_item).collect();
        out.push_str(&items.join("&"));
    }
    if let Some(fragment) = fragment {
        out.push('#');
        if !fragment.is_empty() {
            out.push_str(PLACEHOLDER);
        }
    }
    out
}

fn mask_query_item(item: &str) -> String {
    match item.split_once('=') {
        _ if item.is_empty() => String::new(),
        Some((name, "")) => format!("{name}="),
        Some((name, _)) => format!("{name}={PLACEHOLDER}"),
        None => PLACEHOLDER.to_owned(),
    }
}

/// `base` (an address without query and fragment) with the user info before the host masked.
fn mask_user_info(base: &str) -> String {
    let Some(scheme_end) = base.find("://").map(|at| at + 3) else {
        return base.to_owned();
    };
    let authority_end = base[scheme_end..]
        .find('/')
        .map_or(base.len(), |at| scheme_end + at);
    match base[scheme_end..authority_end].rfind('@') {
        Some(at) if at > 0 => format!(
            "{}{PLACEHOLDER}{}",
            &base[..scheme_end],
            &base[scheme_end + at..]
        ),
        _ => base.to_owned(),
    }
}

/// `base` (an address without query and fragment) with each path segment that looks like a
/// token masked; the scheme and the host stay. Without a scheme all of `base` is the path.
fn mask_path_tokens(base: &str) -> String {
    let path_start = match base.find("://") {
        Some(at) => match base[at + 3..].find('/') {
            Some(slash) => at + 3 + slash,
            None => return base.to_owned(),
        },
        None => 0,
    };
    let segments: Vec<String> = base[path_start..].split('/').map(mask_segment).collect();
    format!("{}{}", &base[..path_start], segments.join("/"))
}

/// `segment` masked when it looks like a token; a stack frame's `:line:column` after it stays.
fn mask_segment(segment: &str) -> String {
    let mut end = segment.len();
    while let Some(colon) = segment[..end].rfind(':') {
        let digits = &segment[colon + 1..end];
        if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
            break;
        }
        end = colon;
    }
    let (core, position) = segment.split_at(end);
    if looks_like_token(core) {
        format!("{PLACEHOLDER}{position}")
    } else {
        segment.to_owned()
    }
}

/// Whether a path segment looks like a token rather than a word, a slug or an id: a UUID, a
/// piece (between characters other than ASCII letters and digits) of [`TOKEN_MIN_LEN`] or more
/// with letters and digits, a number of [`NUMBER_TOKEN_LEN`] digits or more, a run of random-case
/// letters ([`LETTERS_TOKEN_LEN`]), or - `-` and `_` left out, the base64url alphabet - a run of
/// [`LETTERS_TOKEN_LEN`] or more with upper and lower case letters and digits.
fn looks_like_token(core: &str) -> bool {
    if is_uuid(core) {
        return true;
    }
    if core
        .split(|character: char| !character.is_ascii_alphanumeric())
        .any(is_token_piece)
    {
        return true;
    }
    let joined: String = core.chars().filter(|c| *c != '-' && *c != '_').collect();
    joined.len() >= LETTERS_TOKEN_LEN
        && joined.bytes().all(|byte| byte.is_ascii_alphanumeric())
        && joined.bytes().any(|byte| byte.is_ascii_uppercase())
        && joined.bytes().any(|byte| byte.is_ascii_lowercase())
        && joined.bytes().any(|byte| byte.is_ascii_digit())
}

/// `8-4-4-4-12` hexadecimal digits.
fn is_uuid(core: &str) -> bool {
    let groups: Vec<&str> = core.split('-').collect();
    groups.len() == 5
        && groups.iter().zip([8, 4, 4, 4, 12]).all(|(group, len)| {
            group.len() == len && group.bytes().all(|byte| byte.is_ascii_hexdigit())
        })
}

/// One piece of ASCII letters and digits: see [`looks_like_token`].
fn is_token_piece(piece: &str) -> bool {
    let len = piece.len();
    let digits = piece.bytes().filter(u8::is_ascii_digit).count();
    if digits == len {
        return len >= NUMBER_TOKEN_LEN;
    }
    if len < TOKEN_MIN_LEN {
        return false;
    }
    if digits > 0 {
        return true;
    }
    let upper = piece.bytes().filter(u8::is_ascii_uppercase).count();
    len >= LETTERS_TOKEN_LEN && upper * 4 >= len && (len - upper) * 4 >= len
}

/// Every address of `text` (a run between blanks and quotes that holds `://`, `?` or `/`) through
/// [`mask_url`]; the rest as it is. Splits at ASCII characters only, so every slice falls on a
/// character boundary.
fn mask_urls_in(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut word_start = 0;
    for (at, character) in text.char_indices() {
        if character.is_whitespace() || URL_STOPS.contains(&character) {
            push_word(&mut out, &text[word_start..at]);
            out.push(character);
            word_start = at + character.len_utf8();
        }
    }
    push_word(&mut out, &text[word_start..]);
    out
}

fn push_word(out: &mut String, word: &str) {
    if word.contains('/') || word.contains('?') {
        out.push_str(&mask_url(word));
    } else {
        out.push_str(word);
    }
}

/// The value after each credential header name of `text` masked: a quoted value inside its
/// quotes, an unquoted one up to the end of its line. A name counts as a word of its own
/// (`set-cookie` is one name, `xcookie` none), followed by an optional closing quote, `:` or `=`.
fn mask_credential_values(text: &str) -> String {
    let lowered = text.to_ascii_lowercase();
    let mut out = String::with_capacity(text.len());
    let mut copied = 0;
    let mut search = 0;
    while let Some((start, name)) = next_name(&lowered, search) {
        search = start + name.len();
        let Some((value_start, value_end)) = value_after(text, start + name.len()) else {
            continue;
        };
        out.push_str(&text[copied..value_start]);
        out.push_str(PLACEHOLDER);
        copied = value_end;
        search = value_end.max(search);
    }
    out.push_str(&text[copied..]);
    out
}

/// The first credential name in `lowered` at or after `from`, standing as a word of its own.
fn next_name(lowered: &str, from: usize) -> Option<(usize, &'static str)> {
    let word = |c: u8| c.is_ascii_alphanumeric() || c == b'-' || c == b'_';
    let bytes = lowered.as_bytes();
    let mut best: Option<(usize, &'static str)> = None;
    for name in CREDENTIAL_NAMES {
        let mut at = from;
        while let Some(found) = lowered.get(at..).and_then(|rest| rest.find(name)) {
            let start = at + found;
            let end = start + name.len();
            let before = start.checked_sub(1).and_then(|index| bytes.get(index));
            let after = bytes.get(end);
            if !before.is_some_and(|c| word(*c)) && !after.is_some_and(|c| word(*c)) {
                if best.is_none_or(|(known, _)| start < known) {
                    best = Some((start, name));
                }
                break;
            }
            at = end;
        }
    }
    best
}

/// Where the value after a name ending at `at` starts and ends: an optional closing quote of the
/// name, blanks, `:` or `=`, blanks, then a quoted value (inside the quotes, up to the closing
/// one or the end of the line) or an unquoted one up to the end of the line. `None` without a
/// separator or with an empty value.
fn value_after(text: &str, at: usize) -> Option<(usize, usize)> {
    let bytes = text.as_bytes();
    let mut at = at;
    if matches!(bytes.get(at), Some(b'"' | b'\'')) {
        at += 1;
    }
    at += blanks(&bytes[at..]);
    if !matches!(bytes.get(at), Some(b':' | b'=')) {
        return None;
    }
    at += 1;
    at += blanks(&bytes[at..]);
    let line_end = text[at..]
        .find(['\n', '\r'])
        .map_or(text.len(), |end| at + end);
    let (start, end) = match bytes.get(at) {
        Some(quote @ (b'"' | b'\'')) => {
            let inner = at + 1;
            let close = text[inner..line_end]
                .find(char::from(*quote))
                .map_or(line_end, |end| inner + end);
            (inner, close)
        }
        _ => (at, line_end),
    };
    (end > start).then_some((start, end))
}

fn blanks(bytes: &[u8]) -> usize {
    bytes
        .iter()
        .take_while(|byte| **byte == b' ' || **byte == b'\t')
        .count()
}

#[cfg(test)]
#[path = "mask_tests.rs"]
mod tests;
