//! A sitemap read (sitemaps.org protocol 0.9): a `<urlset>` of pages with their `lastmod` and
//! `xhtml:link` alternates (`hreflang`), or a `<sitemapindex>` of further sitemaps; and the
//! plain URL list (`.txt`, one URL a line). Elements are matched by their local name, so a
//! sitemap without the namespace reads as well. A DTD is refused (no entity expansion) and the
//! number of nodes is capped; a `<loc>` that is no `http(s)` URL is skipped and counted.

use crate::url::Url;

/// The namespace of the `xhtml:link` alternates.
pub const XHTML_NS: &str = "http://www.w3.org/1999/xhtml";
/// The most XML nodes one sitemap may have (50 000 URLs with a few alternates each).
pub const MAX_NODES: u32 = 2_000_000;

/// One page of a sitemap.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// The page.
    pub loc: Url,
    /// `<lastmod>` as written (W3C datetime), trimmed.
    pub lastmod: Option<String>,
    /// The `hreflang` of the alternate that points at the page itself, lower-case.
    pub language: Option<String>,
}

/// What a sitemap holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Parsed {
    /// A `<urlset>`: its pages, and how many `<loc>` were skipped.
    Pages {
        /// The pages in document order.
        entries: Vec<Entry>,
        /// `<loc>` that were no `http(s)` URL.
        skipped: usize,
    },
    /// A `<sitemapindex>`: the sitemaps it names.
    Index {
        /// The sitemaps in document order.
        sitemaps: Vec<Url>,
        /// `<loc>` that were no `http(s)` URL.
        skipped: usize,
    },
}

/// Why a sitemap could not be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParseError {
    /// Not UTF-8.
    Encoding,
    /// No well-formed XML (or a DTD, or too many nodes); the parser's reason.
    Xml(String),
    /// Well-formed, but neither `<urlset>` nor `<sitemapindex>`.
    NotASitemap(String),
}

fn text(bytes: &[u8]) -> Result<&str, ParseError> {
    let text = std::str::from_utf8(bytes).map_err(|_| ParseError::Encoding)?;
    Ok(text.strip_prefix('\u{feff}').unwrap_or(text))
}

fn child_text<'a>(node: roxmltree::Node<'a, '_>, name: &str) -> Option<&'a str> {
    node.children()
        .find(|child| child.is_element() && child.tag_name().name() == name)
        .and_then(|child| child.text())
        .map(str::trim)
        .filter(|text| !text.is_empty())
}

fn resolve(base: Option<&Url>, loc: &str) -> Option<Url> {
    Url::parse(loc).or_else(|| base.and_then(|base| base.join(loc)))
}

/// The language of the alternate of `node` (an `<url>`) that points at `loc`.
fn own_language(node: roxmltree::Node<'_, '_>, loc: &Url, base: Option<&Url>) -> Option<String> {
    node.children()
        .filter(|child| {
            child.is_element()
                && child.tag_name().name() == "link"
                && child.tag_name().namespace().is_none_or(|ns| ns == XHTML_NS)
                && child.attribute("rel") == Some("alternate")
        })
        .find(|link| {
            link.attribute("href")
                .and_then(|href| resolve(base, href))
                .is_some_and(|href| &href == loc)
        })
        .and_then(|link| link.attribute("hreflang"))
        .map(|language| language.trim().to_ascii_lowercase())
        .filter(|language| !language.is_empty())
}

/// The sitemap in `bytes`; a relative `<loc>` is resolved against `base` (the sitemap's URL).
///
/// # Errors
///
/// [`ParseError`] when the bytes are no UTF-8, no well-formed XML or no sitemap.
pub fn parse_xml(bytes: &[u8], base: Option<&Url>) -> Result<Parsed, ParseError> {
    let options = roxmltree::ParsingOptions {
        allow_dtd: false,
        nodes_limit: MAX_NODES,
        ..roxmltree::ParsingOptions::default()
    };
    let document = roxmltree::Document::parse_with_options(text(bytes)?, options)
        .map_err(|error| ParseError::Xml(error.to_string()))?;
    let root = document.root_element();
    let mut skipped = 0;
    match root.tag_name().name() {
        "urlset" => {
            let mut entries = Vec::new();
            for node in root
                .children()
                .filter(|node| node.is_element() && node.tag_name().name() == "url")
            {
                let Some(loc) = child_text(node, "loc").and_then(|loc| resolve(base, loc)) else {
                    skipped += 1;
                    continue;
                };
                let language = own_language(node, &loc, base);
                entries.push(Entry {
                    lastmod: child_text(node, "lastmod").map(str::to_owned),
                    language,
                    loc,
                });
            }
            Ok(Parsed::Pages { entries, skipped })
        }
        "sitemapindex" => {
            let mut sitemaps = Vec::new();
            for node in root
                .children()
                .filter(|node| node.is_element() && node.tag_name().name() == "sitemap")
            {
                match child_text(node, "loc").and_then(|loc| resolve(base, loc)) {
                    Some(loc) => sitemaps.push(loc),
                    None => skipped += 1,
                }
            }
            Ok(Parsed::Index { sitemaps, skipped })
        }
        other => Err(ParseError::NotASitemap(other.chars().take(64).collect())),
    }
}

/// A URL list: one URL a line, blank lines and `#` comments skipped; a line that is no
/// `http(s)` URL is skipped and counted.
///
/// # Errors
///
/// [`ParseError::Encoding`] when the bytes are no UTF-8.
pub fn parse_list(bytes: &[u8], base: Option<&Url>) -> Result<Parsed, ParseError> {
    let mut entries = Vec::new();
    let mut skipped = 0;
    for line in text(bytes)?.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        match resolve(base, line) {
            Some(loc) => entries.push(Entry {
                loc,
                lastmod: None,
                language: None,
            }),
            None => skipped += 1,
        }
    }
    Ok(Parsed::Pages { entries, skipped })
}

/// `bytes` read as XML when they look like XML, else as a URL list.
///
/// # Errors
///
/// As [`parse_xml`] and [`parse_list`].
pub fn parse_any(bytes: &[u8], base: Option<&Url>) -> Result<Parsed, ParseError> {
    let start = text(bytes)?.trim_start();
    if start.starts_with('<') {
        parse_xml(bytes, base)
    } else {
        parse_list(bytes, base)
    }
}

#[cfg(test)]
#[path = "sitemap_tests.rs"]
mod tests;
