//! The rules over what one page declares: the codes valid, each `href` absolute, no code and no
//! URL twice in one source, the page itself among its alternates, `x-default` (with
//! `seo.requireXDefault`), and the three sources in agreement - the same URL for a code
//! wherever it is declared, and no code that one declaring source has and another lacks.

use std::collections::{BTreeMap, BTreeSet};

use crate::config::Config;
use crate::hreflang::code::{self, Checked, X_DEFAULT};
use crate::hreflang::load::Loaded;
use crate::hreflang::sources::Source;
use crate::rules::{Sink, short};

/// The form two values of one code compare in: lower case, `-` between the parts.
#[must_use]
pub fn key(value: &str) -> String {
    match code::check(value) {
        Checked::Valid(code) => code.to_string().to_ascii_lowercase(),
        Checked::XDefault => X_DEFAULT.to_owned(),
        Checked::Invalid { .. } => value.trim().to_ascii_lowercase(),
    }
}

fn joined<'a>(items: impl Iterator<Item = &'a str>) -> String {
    short(&items.collect::<Vec<_>>().join(", "), 500)
}

fn codes(page: &Loaded, sink: &mut Sink<'_>) {
    let mut seen = BTreeSet::new();
    for (_, alternate) in page.alternates() {
        if !seen.insert(alternate.code.to_ascii_lowercase()) {
            continue;
        }
        if let Checked::Invalid { suggestion } = code::check(&alternate.code) {
            let code = short(&alternate.code, 64);
            match suggestion {
                Some(suggestion) => sink.emit(
                    "hreflang.code_invalid",
                    "hreflang.code_suggestion",
                    None,
                    &[("code", code), ("suggestion", suggestion)],
                ),
                None => sink.emit(
                    "hreflang.code_invalid",
                    "hreflang.code_invalid",
                    None,
                    &[("code", code)],
                ),
            }
        }
    }
}

fn hrefs(page: &Loaded, sink: &mut Sink<'_>) {
    let mut seen = BTreeSet::new();
    for (_, alternate) in page.alternates() {
        if alternate.is_absolute() || !seen.insert(alternate.href.clone()) {
            continue;
        }
        sink.emit(
            "hreflang.target_relative",
            "hreflang.target_relative",
            None,
            &[
                ("code", short(&alternate.code, 64)),
                ("href", short(&alternate.href, 300)),
            ],
        );
    }
}

fn duplicates(page: &Loaded, sink: &mut Sink<'_>) {
    for (source, list) in &page.declared {
        let mut by_code: BTreeMap<String, usize> = BTreeMap::new();
        let mut by_url: BTreeMap<String, Vec<&str>> = BTreeMap::new();
        for alternate in list {
            let code = key(&alternate.code);
            *by_code.entry(code.clone()).or_default() += 1;
            if code != X_DEFAULT
                && let Some(url) = &alternate.url
            {
                by_url
                    .entry(url.to_string())
                    .or_default()
                    .push(alternate.code.as_str());
            }
        }
        for (code, count) in by_code.iter().filter(|(_, count)| **count > 1) {
            sink.emit(
                "hreflang.duplicate_code",
                "hreflang.duplicate_code",
                None,
                &[
                    ("code", short(code, 64)),
                    ("count", count.to_string()),
                    ("source", source.as_str().to_owned()),
                ],
            );
        }
        for (url, codes) in by_url.iter().filter(|(_, codes)| codes.len() > 1) {
            sink.emit(
                "hreflang.duplicate_url",
                "hreflang.duplicate_url",
                None,
                &[
                    ("url", url.clone()),
                    ("codes", joined(codes.iter().copied())),
                    ("source", source.as_str().to_owned()),
                ],
            );
        }
    }
}

fn consistency(page: &Loaded, sink: &mut Sink<'_>) {
    let declaring: Vec<Source> = page.declared.keys().copied().collect();
    if declaring.len() < 2 {
        return;
    }
    let mut by_code: BTreeMap<String, BTreeMap<Source, BTreeSet<String>>> = BTreeMap::new();
    for (source, alternate) in page.alternates() {
        let target = alternate
            .url
            .as_ref()
            .map_or_else(|| alternate.href.clone(), ToString::to_string);
        by_code
            .entry(key(&alternate.code))
            .or_default()
            .entry(source)
            .or_default()
            .insert(target);
    }
    for (code, sources) in &by_code {
        let targets: BTreeSet<&BTreeSet<String>> = sources.values().collect();
        if targets.len() > 1 {
            let values = sources.iter().map(|(source, urls)| {
                format!(
                    "{}: {}",
                    source.as_str(),
                    urls.iter().cloned().collect::<Vec<_>>().join(" ")
                )
            });
            sink.emit(
                "hreflang.source_conflict",
                "hreflang.source_conflict",
                None,
                &[
                    ("code", short(code, 64)),
                    (
                        "values",
                        short(&values.collect::<Vec<_>>().join("; "), 1000),
                    ),
                ],
            );
        }
        let missing: Vec<&str> = declaring
            .iter()
            .filter(|source| !sources.contains_key(source))
            .map(|source| source.as_str())
            .collect();
        if !missing.is_empty() {
            sink.emit(
                "hreflang.source_missing",
                "hreflang.source_missing",
                None,
                &[
                    ("code", short(code, 64)),
                    ("in", joined(sources.keys().map(|source| source.as_str()))),
                    ("missing", joined(missing.into_iter())),
                ],
            );
        }
    }
}

/// Every declaration rule over `page`; nothing for a page that declares no alternate.
pub fn check(config: &Config, page: &Loaded, sink: &mut Sink<'_>) {
    if page.declared.is_empty() {
        return;
    }
    consistency(page, sink);
    codes(page, sink);
    hrefs(page, sink);
    duplicates(page, sink);
    if !page.declares(&page.url) {
        sink.emit("hreflang.self_missing", "hreflang.self_missing", None, &[]);
    }
    if config.require_x_default
        && !page
            .alternates()
            .any(|(_, alternate)| code::check(&alternate.code) == Checked::XDefault)
    {
        sink.emit(
            "hreflang.x_default_missing",
            "hreflang.x_default_missing",
            None,
            &[],
        );
    }
}

#[cfg(test)]
#[path = "declared_tests.rs"]
mod tests;
