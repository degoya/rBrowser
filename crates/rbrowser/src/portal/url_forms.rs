//! The forms in which a body writes the punctuation of an origin (RB-090-75): plain
//! (`http://host:port`), with JSON's escaped slashes (`http:\/\/host:port`, RB-090-69), and as
//! HTML character references, the way an attribute escaper writes `src` and `srcset` - every byte
//! but letters, digits and `,.-_` as a reference, hex upper (`http&#x3A;&#x2F;&#x2F;`), hex
//! lower or decimal (`http&#58;&#47;&#47;`); the dots of a host or an IP address stay literal.
//! The origin map ([`super::origin_map`]) rewrites each form into the same form and the body scan
//! reads the host of each. Every reader here takes the body in lower case. Pure.

/// What every `http` URL starts with, in any form.
pub(super) const SCHEME: &[u8] = b"http";

/// `http://` in each form, in lower case (hex upper reads as hex lower).
const SCHEMES: [(Form, &[u8]); 4] = [
    (Form::Plain, b"http://"),
    (Form::Escaped, b"http:\\/\\/"),
    (Form::Hex { upper: false }, b"http&#x3a;&#x2f;&#x2f;"),
    (Form::Decimal, b"http&#58;&#47;&#47;"),
];

/// The longest character reference read: `&#x` with six hex digits or `&#` with seven decimal
/// ones, and the `;`.
pub(super) const LONGEST_REFERENCE: usize = 10;

/// How a body writes the punctuation of a URL.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Form {
    /// As it is.
    Plain,
    /// With escaped slashes (`\/`), as JSON may write it.
    Escaped,
    /// As hex character references (`&#x3A;`), upper or lower case.
    Hex { upper: bool },
    /// As decimal character references (`&#58;`).
    Decimal,
}

impl Form {
    /// `origin` written in this form.
    pub(super) fn encode(self, origin: &str) -> Vec<u8> {
        match self {
            Self::Plain => origin.as_bytes().to_vec(),
            Self::Escaped => origin.replace('/', "\\/").into_bytes(),
            Self::Hex { .. } | Self::Decimal => {
                let mut out = Vec::with_capacity(origin.len() * 2);
                for byte in origin.bytes() {
                    let literal = !byte.is_ascii()
                        || byte.is_ascii_alphanumeric()
                        || matches!(byte, b',' | b'.' | b'-' | b'_');
                    if literal {
                        out.push(byte);
                    } else {
                        let reference = match self {
                            Self::Hex { upper: true } => format!("&#x{byte:02X};"),
                            Self::Hex { upper: false } => format!("&#x{byte:02x};"),
                            _ => format!("&#{byte};"),
                        };
                        out.extend_from_slice(reference.as_bytes());
                    }
                }
                out
            }
        }
    }
}

/// The length of the `http://`, in any form, at `at` of `lower`, if one starts there.
pub(super) fn scheme_at(lower: &[u8], at: usize) -> Option<usize> {
    scheme_form_at(lower, at).map(|(_, length)| length)
}

/// The form and length of the `http://` at `at` of `lower`, if one starts there (hex in lower
/// case).
pub(super) fn scheme_form_at(lower: &[u8], at: usize) -> Option<(Form, usize)> {
    let rest = lower.get(at..)?;
    SCHEMES
        .iter()
        .find(|(_, scheme)| rest.starts_with(scheme))
        .map(|(form, scheme)| (*form, scheme.len()))
}

/// Whether `rest` is the start of an `http://`, in any form, that the next bytes may finish.
pub(super) fn could_be_scheme(rest: &[u8]) -> bool {
    !rest.is_empty()
        && SCHEMES
            .iter()
            .any(|(_, scheme)| rest.len() < scheme.len() && scheme.starts_with(rest))
}

/// The byte at `at` of `lower` and how many bytes stand for it: a numeric character reference
/// of an ASCII byte (`&#x3a;`, `&#58;`) as that byte, anything else as itself.
pub(super) fn char_at(lower: &[u8], at: usize) -> Option<(u8, usize)> {
    let byte = *lower.get(at)?;
    Some(reference_at(&lower[at..]).unwrap_or((byte, 1)))
}

/// The ASCII byte the numeric character reference at the start of `rest` stands for, and its
/// length.
fn reference_at(rest: &[u8]) -> Option<(u8, usize)> {
    let body = rest.strip_prefix(b"&#")?;
    let (prefix, radix, most) = if body.first() == Some(&b'x') {
        (1, 16, 6)
    } else {
        (0, 10, 7)
    };
    let digits = &body[prefix..];
    let count = digits
        .iter()
        .take(most + 1)
        .take_while(|byte| {
            if radix == 16 {
                byte.is_ascii_hexdigit()
            } else {
                byte.is_ascii_digit()
            }
        })
        .count();
    if count == 0 || count > most || digits.get(count) != Some(&b';') {
        return None;
    }
    let text = std::str::from_utf8(&digits[..count]).ok()?;
    let value = u32::from_str_radix(text, radix).ok()?;
    let byte = u8::try_from(value).ok().filter(u8::is_ascii)?;
    Some((byte, 2 + prefix + count + 1))
}

/// Whether `rest`, the end of what came so far, is the start of a numeric character reference
/// that the next bytes may finish.
pub(super) fn partial_reference(rest: &[u8]) -> bool {
    let Some(body) = rest.strip_prefix(b"&") else {
        return false;
    };
    let Some(body) = body.strip_prefix(b"#") else {
        return body.is_empty();
    };
    match body.strip_prefix(b"x") {
        Some(digits) => digits.len() <= 6 && digits.iter().all(u8::is_ascii_hexdigit),
        None => body.len() <= 7 && body.iter().all(u8::is_ascii_digit),
    }
}

/// Whether the origin that ends at `end` of `lower` ends there: no host or port goes on, written
/// as a byte or as a character reference.
pub(super) fn is_whole(lower: &[u8], end: usize) -> bool {
    match char_at(lower, end) {
        None => true,
        Some((next, _)) => {
            !next.is_ascii_alphanumeric() && !matches!(next, b'.' | b'-' | b'_' | b':')
        }
    }
}

/// The host and port of a URL, read through its character references.
#[derive(Debug)]
pub(super) struct Authority {
    /// The host and port, decoded.
    pub(super) text: String,
    /// Where they end in the body.
    pub(super) end: usize,
    /// Whether they run to the end of what came (a reference included), so may still go on.
    pub(super) open: bool,
}

/// The host and port that start at `at` of `lower`.
pub(super) fn authority_at(lower: &[u8], at: usize) -> Authority {
    let mut text = String::new();
    let mut end = at;
    loop {
        if end >= lower.len() || partial_reference(&lower[end..]) {
            return Authority {
                text,
                end: lower.len(),
                open: true,
            };
        }
        match char_at(lower, end) {
            Some((byte, length)) if byte.is_ascii_alphanumeric() || b".-:[]".contains(&byte) => {
                text.push(char::from(byte));
                end += length;
            }
            _ => {
                return Authority {
                    text,
                    end,
                    open: false,
                };
            }
        }
    }
}

/// Whether the first character reference in `original` (not lower case) is written in upper
/// case (`&#x3A;`, `&#X3a;`): the form a hex-encoded origin is written back in.
pub(super) fn upper_hex(original: &[u8]) -> bool {
    original
        .split(|byte| *byte == b'&')
        .nth(1)
        .is_some_and(|reference| {
            reference
                .iter()
                .take_while(|byte| **byte != b';')
                .any(u8::is_ascii_uppercase)
        })
}

#[cfg(test)]
#[path = "url_forms_tests.rs"]
mod tests;
