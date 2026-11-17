//! The start tags of a raw HTML document with their byte offsets, in the order the source
//! writes them - what html5ever's tree forgets: which element comes first in the head, where the
//! head ends, and what the source puts into the body that belongs into the head. A small
//! tokenizer: comments, the doctype and processing instructions skipped, the raw text of
//! `script`, `style`, `title`, `textarea`, `xmp` and `noscript` skipped to its end tag, and so are
//! the contents of `svg` and `math`.

/// The elements a head may hold; any other start tag in the head ends it (HTML's "in head"
/// insertion mode).
pub const HEAD_ELEMENTS: [&str; 8] = [
    "base", "link", "meta", "noscript", "script", "style", "template", "title",
];
/// The most tags read.
const MAX_TAGS: usize = 20_000;

/// One start tag.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tag {
    /// Lower case.
    pub name: String,
    /// Names in lower case, values as written (without quotes).
    pub attrs: Vec<(String, String)>,
    /// The byte offset of `<` and the one after `>`.
    pub start: usize,
    pub end: usize,
}

impl Tag {
    #[must_use]
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }

    /// Whether `attr` holds `token` among its space-separated values (case-blind).
    #[must_use]
    pub fn has_token(&self, attr: &str, token: &str) -> bool {
        self.attr(attr).is_some_and(|value| {
            value
                .split_ascii_whitespace()
                .any(|part| part.eq_ignore_ascii_case(token))
        })
    }
}

/// What the source's head holds.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Scan {
    /// The head's elements, in source order.
    pub head: Vec<Tag>,
    /// The start tag that ended the head early (no head element, before `<body>`).
    pub breaker: Option<Tag>,
    /// The start tags after the head, in source order.
    pub body: Vec<Tag>,
}

fn find_ci(haystack: &str, from: usize, needle: &str) -> Option<usize> {
    let bytes = haystack.as_bytes();
    let needle = needle.as_bytes();
    (from..bytes.len().saturating_sub(needle.len() - 1))
        .find(|&at| bytes[at..at + needle.len()].eq_ignore_ascii_case(needle))
}

/// The attributes of the tag text between the name and `>`.
fn attributes(text: &str) -> Vec<(String, String)> {
    let mut attrs = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut at = 0;
    while at < chars.len() {
        while at < chars.len() && (chars[at].is_whitespace() || chars[at] == '/') {
            at += 1;
        }
        let start = at;
        while at < chars.len() && !chars[at].is_whitespace() && !matches!(chars[at], '=' | '/') {
            at += 1;
        }
        if start == at {
            break;
        }
        let name: String = chars[start..at]
            .iter()
            .collect::<String>()
            .to_ascii_lowercase();
        while at < chars.len() && chars[at].is_whitespace() {
            at += 1;
        }
        let mut value = String::new();
        if chars.get(at) == Some(&'=') {
            at += 1;
            while at < chars.len() && chars[at].is_whitespace() {
                at += 1;
            }
            match chars.get(at) {
                Some(&quote) if quote == '"' || quote == '\'' => {
                    at += 1;
                    while at < chars.len() && chars[at] != quote {
                        value.push(chars[at]);
                        at += 1;
                    }
                    at += 1;
                }
                _ => {
                    while at < chars.len() && !chars[at].is_whitespace() {
                        value.push(chars[at]);
                        at += 1;
                    }
                }
            }
        }
        attrs.push((name, value));
    }
    attrs
}

/// The end of the tag that starts at `start` (the byte after `>`, quotes respected).
fn tag_end(html: &str, start: usize) -> usize {
    let mut quote: Option<u8> = None;
    for (offset, byte) in html.as_bytes()[start..].iter().enumerate() {
        match (quote, byte) {
            (Some(q), b) if *b == q => quote = None,
            (None, b'"' | b'\'') => quote = Some(*byte),
            (None, b'>') => return start + offset + 1,
            _ => {}
        }
    }
    html.len()
}

/// The start tags of `html`.
#[must_use]
pub fn tags(html: &str) -> Vec<Tag> {
    let bytes = html.as_bytes();
    let mut out = Vec::new();
    let mut at = 0;
    while let Some(offset) = html.get(at..).and_then(|rest| rest.find('<')) {
        let start = at + offset;
        if out.len() >= MAX_TAGS {
            break;
        }
        if html[start..].starts_with("<!--") {
            at = html[start + 4..]
                .find("-->")
                .map_or(html.len(), |end| start + 4 + end + 3);
            continue;
        }
        let next = bytes.get(start + 1).copied().unwrap_or_default();
        if matches!(next, b'!' | b'?' | b'/') {
            at = tag_end(html, start);
            continue;
        }
        if !next.is_ascii_alphabetic() {
            at = start + 1;
            continue;
        }
        let end = tag_end(html, start);
        let inner = html[start + 1..end].trim_end_matches('>');
        let name_end = inner
            .find(|c: char| c.is_whitespace() || c == '/')
            .unwrap_or(inner.len());
        let name = inner[..name_end].to_ascii_lowercase();
        let tag = Tag {
            attrs: attributes(&inner[name_end..]),
            name,
            start,
            end,
        };
        at = end;
        let raw = matches!(
            tag.name.as_str(),
            "script" | "style" | "title" | "textarea" | "xmp" | "noscript"
        );
        // Foreign content: an SVG's `title` is no document title.
        let foreign = matches!(tag.name.as_str(), "svg" | "math") && !inner.ends_with('/');
        if raw || foreign {
            at = find_ci(html, end, &format!("</{}", tag.name)).unwrap_or(html.len());
        }
        out.push(tag);
    }
    out
}

/// The head of `html` as the source writes it.
#[must_use]
pub fn scan(html: &str) -> Scan {
    let mut scan = Scan::default();
    let mut in_head = true;
    for tag in tags(html) {
        match tag.name.as_str() {
            "html" | "head" if in_head => {}
            "body" => in_head = false,
            name if in_head && HEAD_ELEMENTS.contains(&name) => scan.head.push(tag),
            _ if in_head => {
                in_head = false;
                scan.breaker = Some(tag);
            }
            _ => scan.body.push(tag),
        }
    }
    scan
}

#[cfg(test)]
#[path = "scan_tests.rs"]
mod tests;
