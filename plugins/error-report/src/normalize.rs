//! What makes two reports the same error: texts without numbers and ids, URLs without a cache
//! buster, stack traces without their line and column noise.
//!
//! A text is compared without its numbers and ids - a width a view logs, a time stamp, a counter,
//! a UUID or a hash differ from view to view and run to run - and a URL without its fragment and
//! its cache-busting query parameters: those named like one ([`BUSTER_NAMES`]) and any whose
//! value is a long number or hex id. A stack trace counts by its functions and scripts, never by
//! its positions, so a rebuilt bundle with shifted lines is the same error.

use rb_util::url::host_of;

/// The stand-in for a run of digits.
pub const NUMBER: &str = "#";
/// The stand-in for an id: a UUID or a run of at least [`MIN_ID_CHARS`] hex digits with a digit.
pub const ID: &str = "<id>";
/// The shortest hex run that counts as an id.
pub const MIN_ID_CHARS: usize = 8;
/// Query parameters that only bust caches.
pub const BUSTER_NAMES: &[&str] = &[
    "_",
    "v",
    "ver",
    "version",
    "t",
    "ts",
    "timestamp",
    "cb",
    "cachebust",
    "cachebuster",
    "rev",
    "hash",
    "nocache",
    "bust",
];

/// `text` without numbers and ids, whitespace collapsed.
///
/// Words are runs of ASCII letters, digits and `-`; a word whose `-`-separated parts are all hex,
/// at least [`MIN_ID_CHARS`] hex digits together and holding a digit, is an id ([`ID`]); in any
/// other word each run of digits is [`NUMBER`]. Everything between words is kept.
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

fn is_buster(name: &str, value: &str) -> bool {
    let name = name.to_ascii_lowercase();
    BUSTER_NAMES.contains(&name.as_str())
        || (value.len() >= MIN_ID_CHARS && value.chars().all(|c| c.is_ascii_digit()))
        || is_id(value)
}

/// `url` as the key compares it: without its fragment and cache-busting query parameters.
#[must_use]
pub fn url_key(url: &str) -> String {
    let url = url.split('#').next().unwrap_or_default();
    let Some((base, query)) = url.split_once('?') else {
        return url.to_owned();
    };
    let kept: Vec<&str> = query
        .split('&')
        .filter(|pair| !pair.is_empty())
        .filter(|pair| {
            let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
            !is_buster(name, value)
        })
        .collect();
    if kept.is_empty() {
        base.to_owned()
    } else {
        format!("{base}?{}", kept.join("&"))
    }
}

/// An exception's text split into its message (the first line) and its stack trace (the lines
/// after it, trimmed), as V8 writes `Error.stack` into the description.
#[must_use]
pub fn split_stack(text: &str) -> (String, Option<String>) {
    let mut lines = text.lines();
    let message = lines.next().unwrap_or_default().trim().to_owned();
    let stack: Vec<&str> = lines.map(str::trim).filter(|l| !l.is_empty()).collect();
    (message, (!stack.is_empty()).then(|| stack.join("\n")))
}

/// A stack trace as the key compares it: per frame the function and the script without its
/// position and cache buster (`f@https://a.test/app.js`), joined by `|`.
#[must_use]
pub fn stack_key(stack: &str) -> String {
    stack
        .lines()
        .map(|line| {
            let line = line.trim().trim_start_matches("at ").trim();
            let (function, location) = match line.rsplit_once(" (") {
                Some((function, location)) => (function, location.trim_end_matches(')')),
                None => ("", line),
            };
            format!("{function}@{}", url_key(strip_position(location)))
        })
        .collect::<Vec<_>>()
        .join("|")
}

/// `location` without a trailing `:line` or `:line:column`.
fn strip_position(location: &str) -> &str {
    let mut rest = location;
    for _ in 0..2 {
        match rest.rsplit_once(':') {
            Some((head, tail)) if !tail.is_empty() && tail.chars().all(|c| c.is_ascii_digit()) => {
                rest = head;
            }
            _ => break,
        }
    }
    rest
}

/// Whether `pattern` (`*` any text, `?` one character) matches all of `text`, case-blind.
#[must_use]
pub fn glob(pattern: &str, text: &str) -> bool {
    let pattern: Vec<char> = pattern.to_lowercase().chars().collect();
    let text: Vec<char> = text.to_lowercase().chars().collect();
    let (mut p, mut t) = (0, 0);
    let mut star: Option<(usize, usize)> = None;
    while t < text.len() {
        match pattern.get(p) {
            Some('*') => {
                star = Some((p, t));
                p += 1;
            }
            Some(&c) if c == '?' || c == text[t] => {
                p += 1;
                t += 1;
            }
            _ => match star {
                Some((sp, st)) => {
                    p = sp + 1;
                    t = st + 1;
                    star = Some((sp, st + 1));
                }
                None => return false,
            },
        }
    }
    pattern[p..].iter().all(|&c| c == '*')
}

/// Whether `url` belongs to the site of `page`: the same host, or one a subdomain of the other.
/// A URL without a host (inline, `data:`) belongs to the page.
#[must_use]
pub fn same_site(url: &str, page: &str) -> bool {
    let (Some(host), Some(page)) = (host_of(url), host_of(page)) else {
        return true;
    };
    host == page || host.ends_with(&format!(".{page}")) || page.ends_with(&format!(".{host}"))
}

/// FNV-1a, 64 bit: the same in every call and on every machine.
#[must_use]
pub fn fnv1a(text: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in text.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// The id of a dedupe key: its [`fnv1a`] as 16 hex digits.
#[must_use]
pub fn id_of(key: &str) -> String {
    format!("{:016x}", fnv1a(key))
}

/// `text` cut to `max` characters, an ellipsis (`...`) marking the cut: the app's own
/// (RB-094-10).
pub use rb_util::text::cut_ellipsis as cut;

#[cfg(test)]
#[path = "normalize_tests.rs"]
mod tests;
