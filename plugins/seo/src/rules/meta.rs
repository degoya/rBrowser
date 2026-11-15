//! `meta.*`: one title and one description of a sensible length (`seo.title.*`,
//! `seo.description.*`), `<html lang>`, a viewport and a charset. Duplicates across a crawl are
//! [`duplicates`].

use std::collections::BTreeMap;

use crate::audit::{Page, PageSummary};
use crate::config::Config;
use crate::page::sel;
use crate::rules::{Element, Sink, short};

fn length(
    sink: &mut Sink<'_>,
    rule: &str,
    kind: &str,
    text: &str,
    (min, max): (usize, usize),
    element: Option<Element>,
) {
    let count = text.chars().count();
    let message = if count < min {
        format!("meta.{kind}_short")
    } else if count > max {
        format!("meta.{kind}_long")
    } else {
        return;
    };
    sink.emit(
        rule,
        &message,
        element,
        &[
            ("length", count.to_string()),
            ("min", min.to_string()),
            ("max", max.to_string()),
            ("text", short(text, 200)),
        ],
    );
}

pub fn check(page: &Page<'_>, sink: &mut Sink<'_>) {
    let Some((html, facts)) = page.primary() else {
        return;
    };
    let config: &Config = page.config;
    let title_element = html.select(&sel("head title")).next().map(Element::of);
    match facts.titles.first() {
        None => sink.emit("meta.title_missing", "meta.title_missing", None, &[]),
        Some(title) if title.is_empty() => sink.emit(
            "meta.title_missing",
            "meta.title_empty",
            title_element.clone(),
            &[],
        ),
        Some(title) => length(
            sink,
            "meta.title_length",
            "title",
            title,
            (config.title_min, config.title_max),
            title_element.clone(),
        ),
    }
    if facts.titles.len() > 1 {
        sink.emit(
            "meta.title_multiple",
            "meta.title_multiple",
            title_element,
            &[("count", facts.titles.len().to_string())],
        );
    }
    let description_element = html
        .select(&sel("meta[name]"))
        .find(|meta| {
            meta.value()
                .attr("name")
                .is_some_and(|name| name.trim().eq_ignore_ascii_case("description"))
        })
        .map(Element::of);
    match facts.descriptions.first() {
        None => sink.emit(
            "meta.description_missing",
            "meta.description_missing",
            None,
            &[],
        ),
        Some(text) if text.is_empty() => sink.emit(
            "meta.description_missing",
            "meta.description_empty",
            description_element.clone(),
            &[],
        ),
        Some(text) => length(
            sink,
            "meta.description_length",
            "description",
            text,
            (config.description_min, config.description_max),
            description_element.clone(),
        ),
    }
    if facts.descriptions.len() > 1 {
        sink.emit(
            "meta.description_multiple",
            "meta.description_multiple",
            description_element,
            &[("count", facts.descriptions.len().to_string())],
        );
    }
    if facts.lang.is_none() {
        sink.emit(
            "meta.lang_missing",
            "meta.lang_missing",
            Some(Element::at("html")),
            &[],
        );
    }
    if facts.viewport.is_none() {
        sink.emit("meta.viewport_missing", "meta.viewport_missing", None, &[]);
    }
    let header_charset = page.fetched.response.as_ref().is_some_and(|response| {
        response
            .header("content-type")
            .any(|kind| kind.to_ascii_lowercase().contains("charset="))
    });
    let raw_charset = page.raw.as_ref().is_some_and(|doc| doc.facts.charset);
    if !header_charset && !raw_charset && !facts.charset {
        sink.emit("meta.charset_missing", "meta.charset_missing", None, &[]);
    }
}

/// `meta.title_duplicate` and `meta.description_duplicate` across the pages of a crawl: each
/// page whose title (description) another page has too, with that page's URL.
pub fn duplicates(pages: &[PageSummary], sink: &mut Sink<'_>) {
    for (rule, pick) in [
        (
            "meta.title_duplicate",
            (|page: &PageSummary| page.title.clone()) as fn(&PageSummary) -> Option<String>,
        ),
        ("meta.description_duplicate", |page: &PageSummary| {
            page.description.clone()
        }),
    ] {
        let mut by_text: BTreeMap<String, Vec<&PageSummary>> = BTreeMap::new();
        for page in pages.iter().filter(|page| page.status == Some(200)) {
            if let Some(text) = pick(page).filter(|text| !text.is_empty()) {
                by_text.entry(text.to_lowercase()).or_default().push(page);
            }
        }
        for same in by_text.values().filter(|same| same.len() > 1) {
            for (index, page) in same.iter().enumerate() {
                let other = same[if index == 0 { 1 } else { 0 }];
                sink.page(&page.final_url, None);
                sink.emit(
                    rule,
                    rule,
                    None,
                    &[
                        ("other", other.final_url.clone()),
                        ("count", same.len().to_string()),
                        ("text", short(&pick(page).unwrap_or_default(), 200)),
                    ],
                );
            }
        }
    }
}

#[cfg(test)]
#[path = "meta_tests.rs"]
mod tests;
