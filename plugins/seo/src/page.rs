//! What the rules read from one HTML document, raw or rendered: the head's facts, the headings
//! and the links. Parsed with `scraper` (html5ever), the same parser for both, so a difference
//! between them is the page's, not the parser's.

use std::collections::BTreeMap;

use scraper::{ElementRef, Html, Selector};

use crate::path::element_path;
use crate::url::Url;

/// The most links of one page a crawl looks at.
pub const MAX_LINKS: usize = 2000;
/// The longest text kept of a title, a description or a heading.
const MAX_TEXT: usize = 1024;

/// A selector of this crate's own; each is a constant, so a parse failure is a bug.
#[must_use]
pub fn sel(text: &str) -> Selector {
    Selector::parse(text).expect("a selector of the plugin's own parses")
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

/// The visible text of an element, collapsed.
#[must_use]
pub fn text_of(element: ElementRef<'_>) -> String {
    squash(&element.text().collect::<String>())
}

/// One `h1`..`h6`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Heading {
    pub level: u8,
    pub path: String,
    pub text: String,
}

/// One `<script type="application/ld+json">`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JsonLd {
    pub path: String,
    pub text: String,
}

/// The facts of one document the rules read.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Facts {
    /// Every `<title>` in the head, collapsed.
    pub titles: Vec<String>,
    /// Every `<meta name="description">`'s content.
    pub descriptions: Vec<String>,
    /// Every `<meta name="robots|googlebot">`'s content, lower case.
    pub robots: Vec<String>,
    /// Every `<link rel="canonical">`'s `href`, as written.
    pub canonicals: Vec<String>,
    /// `<html lang>`, trimmed; `None` when absent or empty.
    pub lang: Option<String>,
    pub viewport: Option<String>,
    /// `<meta charset>` or the `http-equiv` content type with a charset.
    pub charset: bool,
    /// `og:*` and `twitter:*` metadata (`property` or `name`, lower case), the first value each.
    pub social: BTreeMap<String, String>,
    pub json_ld: Vec<JsonLd>,
    pub headings: Vec<Heading>,
}

fn content(element: ElementRef<'_>) -> String {
    squash(element.value().attr("content").unwrap_or_default())
}

fn has_token(value: Option<&str>, token: &str) -> bool {
    value.is_some_and(|value| {
        value
            .split_ascii_whitespace()
            .any(|part| part.eq_ignore_ascii_case(token))
    })
}

/// The facts of `html`.
#[must_use]
pub fn facts(html: &Html) -> Facts {
    let mut facts = Facts {
        titles: html.select(&sel("head title")).map(text_of).collect(),
        lang: html
            .root_element()
            .value()
            .attr("lang")
            .map(str::trim)
            .filter(|lang| !lang.is_empty())
            .map(str::to_owned),
        ..Facts::default()
    };
    for meta in html.select(&sel("meta")) {
        let element = meta.value();
        if element.attr("charset").is_some() {
            facts.charset = true;
        }
        if element
            .attr("http-equiv")
            .is_some_and(|name| name.eq_ignore_ascii_case("content-type"))
            && content(meta).to_ascii_lowercase().contains("charset=")
        {
            facts.charset = true;
        }
        let name = element
            .attr("name")
            .or_else(|| element.attr("property"))
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase();
        match name.as_str() {
            "description" => facts.descriptions.push(content(meta)),
            "robots" | "googlebot" => facts.robots.push(content(meta).to_ascii_lowercase()),
            "viewport" => facts.viewport = Some(content(meta)),
            social if social.starts_with("og:") || social.starts_with("twitter:") => {
                facts
                    .social
                    .entry(name.clone())
                    .or_insert_with(|| content(meta));
            }
            _ => {}
        }
    }
    facts.canonicals = html
        .select(&sel("link[rel]"))
        .filter(|link| has_token(link.value().attr("rel"), "canonical"))
        .map(|link| {
            link.value()
                .attr("href")
                .unwrap_or_default()
                .trim()
                .to_owned()
        })
        .collect();
    facts.json_ld = html
        .select(&sel("script[type]"))
        .filter(|script| {
            script
                .value()
                .attr("type")
                .is_some_and(|kind| kind.trim().eq_ignore_ascii_case("application/ld+json"))
        })
        .map(|script| JsonLd {
            path: element_path(script),
            text: script.text().collect(),
        })
        .collect();
    facts.headings = html
        .select(&sel("h1, h2, h3, h4, h5, h6"))
        .map(|heading| Heading {
            level: heading.value().name()[1..].parse().unwrap_or(1),
            path: element_path(heading),
            text: text_of(heading),
        })
        .collect();
    facts
}

/// Whether a `robots` value (meta content or `X-Robots-Tag`) says `noindex` (or `none`); an
/// `X-Robots-Tag` aimed at one crawler (`otherbot: noindex`) counts only for Google's.
#[must_use]
pub fn says_noindex(value: &str) -> bool {
    let value = value.to_ascii_lowercase();
    let directives = match value.split_once(':') {
        Some((agent, rest))
            if !agent.contains(',')
                && !matches!(
                    agent.trim(),
                    "unavailable_after" | "max-snippet" | "max-image-preview" | "max-video-preview"
                )
                && !agent.trim().contains(' ') =>
        {
            if agent.trim() != "googlebot" && agent.trim() != "*" {
                return false;
            }
            rest.to_owned()
        }
        _ => value,
    };
    directives
        .split(',')
        .map(str::trim)
        .any(|directive| directive == "noindex" || directive == "none")
}

/// The `<a href>` targets of `html`, resolved against `base`, `http(s)` only, at most
/// [`MAX_LINKS`]; a `rel="nofollow"` link is left out.
#[must_use]
pub fn links(html: &Html, base: &Url) -> Vec<Url> {
    html.select(&sel("a[href]"))
        .filter(|link| !has_token(link.value().attr("rel"), "nofollow"))
        .filter_map(|link| base.join(link.value().attr("href").unwrap_or_default()))
        .take(MAX_LINKS)
        .collect()
}

#[cfg(test)]
#[path = "page_tests.rs"]
mod tests;
