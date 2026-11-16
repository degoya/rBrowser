//! The alternates one page declares, from each of the three sources: the head's
//! `<link rel="alternate" hreflang>`, the HTTP `Link` header (RFC 8288) and the XML sitemap's
//! `xhtml:link` (`crate::sitemap`).

use scraper::Html;
use serde::Serialize;

use crate::page::sel;
use crate::url::Url;

/// The most alternates read from one source of one page.
pub const MAX_ALTERNATES: usize = 500;
/// The longest `hreflang` value kept.
const MAX_CODE: usize = 64;

/// Where an alternate is declared.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Html,
    Header,
    Sitemap,
}

impl Source {
    pub const ALL: [Self; 3] = [Self::Html, Self::Header, Self::Sitemap];

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Html => "html",
            Self::Header => "header",
            Self::Sitemap => "sitemap",
        }
    }
}

/// One declared alternate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Alternate {
    /// The `hreflang` value as written, trimmed.
    pub code: String,
    /// The `href` as written, trimmed.
    pub href: String,
    /// The `href` resolved against the declaring document; `None` when it is no `http(s)` URL.
    pub url: Option<Url>,
}

impl Alternate {
    #[must_use]
    pub fn new(code: &str, href: &str, base: &Url) -> Self {
        let href = href.trim();
        Self {
            code: code.trim().chars().take(MAX_CODE).collect(),
            href: href.chars().take(crate::url::MAX_URL_CHARS).collect(),
            url: base.join(href),
        }
    }

    /// Whether the `href` is written as an absolute `http(s)` URL, as `hreflang` requires.
    #[must_use]
    pub fn is_absolute(&self) -> bool {
        let lower = self.href.to_ascii_lowercase();
        lower.starts_with("https://") || lower.starts_with("http://")
    }
}

fn has_token(value: &str, token: &str) -> bool {
    value
        .split_ascii_whitespace()
        .any(|part| part.eq_ignore_ascii_case(token))
}

/// The `<link rel="alternate" hreflang>` of `html`, resolved against `base`.
#[must_use]
pub fn html_alternates(html: &Html, base: &Url) -> Vec<Alternate> {
    html.select(&sel("link[rel][hreflang]"))
        .filter(|link| has_token(link.value().attr("rel").unwrap_or_default(), "alternate"))
        .take(MAX_ALTERNATES)
        .map(|link| {
            let element = link.value();
            Alternate::new(
                element.attr("hreflang").unwrap_or_default(),
                element.attr("href").unwrap_or_default(),
                base,
            )
        })
        .collect()
}

/// One link of a `Link` header: its target and its parameters (names lower case, values
/// unquoted).
fn header_links(value: &str) -> Vec<(String, Vec<(String, String)>)> {
    let mut found = Vec::new();
    let mut rest = value;
    while let Some(open) = rest.find('<') {
        let Some(close) = rest[open..].find('>') else {
            break;
        };
        let target = rest[open + 1..open + close].trim().to_owned();
        let after = &rest[open + close + 1..];
        let end = after.find('<').unwrap_or(after.len());
        let params = after[..end]
            .split(';')
            .filter_map(|param| {
                let (key, value) = param.split_once('=')?;
                let value = value.trim().trim_end_matches(',').trim().trim_matches('"');
                Some((key.trim().to_ascii_lowercase(), value.trim().to_owned()))
            })
            .collect();
        found.push((target, params));
        rest = &after[end..];
    }
    found
}

/// The `rel="alternate"` links with a `hreflang` of `Link` header values, resolved against
/// `base`.
#[must_use]
pub fn header_alternates<'a>(values: impl Iterator<Item = &'a str>, base: &Url) -> Vec<Alternate> {
    values
        .flat_map(header_links)
        .filter_map(|(target, params)| {
            let alternate = params
                .iter()
                .any(|(key, value)| key == "rel" && has_token(value, "alternate"));
            let code = params.iter().find(|(key, _)| key == "hreflang")?;
            alternate.then(|| Alternate::new(&code.1, &target, base))
        })
        .take(MAX_ALTERNATES)
        .collect()
}

#[cfg(test)]
#[path = "sources_tests.rs"]
mod tests;
