//! The rules over the language set as a whole: the locales `seo.locales` expects, titles and
//! descriptions no two languages share (`meta.title_duplicate`, `meta.description_duplicate`),
//! and alt texts taken over untranslated from another language (`images.alt_untranslated`).

use std::collections::BTreeMap;

use crate::audit::PageSummary;
use crate::config::Config;
use crate::hreflang::code::{self, Checked};
use crate::hreflang::declared::key;
use crate::hreflang::load::Loaded;
use crate::rules::images::alt::foreign_language;
use crate::rules::{Sink, meta, short};

/// The fewest characters an alt text needs to count as untranslated (short ones, like a brand
/// name, are the same in every language).
const MIN_ALT_CHARS: usize = 12;
/// The fewest characters before the language of an alt text is guessed (as `images.alt_language`).
const GUESS_MIN_CHARS: usize = 24;

/// Whether the declared `code` gives the expected `locale`: the same code, or for a bare
/// language (`de`) any code of that language (`de-AT`).
fn covers(code: &str, locale: &str) -> bool {
    if key(code) == key(locale) {
        return true;
    }
    match (code::check(code), code::check(locale)) {
        (Checked::Valid(code), Checked::Valid(locale)) => {
            locale.region.is_none() && locale.script.is_none() && code.language == locale.language
        }
        _ => false,
    }
}

/// `hreflang.locale_missing` for each locale of `seo.locales` the start page's set lacks, and
/// `hreflang.none` when it declares no alternate at all.
pub fn locales(config: &Config, start: &Loaded, sink: &mut Sink<'_>) {
    sink.page(&start.url.to_string(), start.view);
    if start.declared.is_empty() {
        sink.emit("hreflang.none", "hreflang.none", None, &[]);
    }
    for locale in &config.locales {
        if !start
            .alternates()
            .any(|(_, alternate)| covers(&alternate.code, locale))
        {
            sink.emit(
                "hreflang.locale_missing",
                "hreflang.locale_missing",
                None,
                &[("locale", short(locale, 64))],
            );
        }
    }
}

/// Titles and descriptions shared across the pages of the set that answer 200.
pub fn duplicates(pages: &[(Loaded, String)], sink: &mut Sink<'_>) {
    let summaries: Vec<PageSummary> = pages
        .iter()
        .filter(|(page, _)| page.ok())
        .map(|(page, _)| PageSummary {
            url: page.url.to_string(),
            final_url: page.url.to_string(),
            status: Some(200),
            title: page.title.clone(),
            description: page.description.clone(),
            ..PageSummary::default()
        })
        .collect();
    meta::duplicates(&summaries, sink);
}

/// The primary language of a page of the set: its code's, else its `lang`'s.
fn language(page: &Loaded, code: &str) -> Option<String> {
    match code::check(code) {
        Checked::Valid(code) => Some(code.language),
        _ => page
            .lang
            .as_deref()
            .and_then(|lang| lang.split(['-', '_']).next())
            .map(str::to_ascii_lowercase)
            .filter(|lang| !lang.is_empty()),
    }
}

/// `images.alt_untranslated`: the same alt text on pages of different languages. The page whose
/// language the text is written in (as far as it can be told) keeps it; every page of another
/// language gets the hint - without a reliable guess, every page after the first.
pub fn alts(pages: &[(Loaded, String)], sink: &mut Sink<'_>) {
    let mut by_alt: BTreeMap<&str, Vec<(usize, String)>> = BTreeMap::new();
    for (index, (page, code)) in pages.iter().enumerate() {
        let Some(lang) = language(page, code).filter(|_| page.ok()) else {
            continue;
        };
        let mut seen = Vec::new();
        for alt in page
            .alts
            .iter()
            .filter(|alt| alt.chars().count() >= MIN_ALT_CHARS)
        {
            if !seen.contains(&alt) {
                seen.push(alt);
                by_alt.entry(alt).or_default().push((index, lang.clone()));
            }
        }
    }
    for (alt, holders) in by_alt {
        let first = &holders[0].1;
        if holders.iter().all(|(_, lang)| lang == first) {
            continue;
        }
        let guessable = alt.chars().count() >= GUESS_MIN_CHARS
            && whatlang::detect(alt).is_some_and(|info| info.is_reliable());
        let origin = holders
            .iter()
            .find(|(_, lang)| guessable && foreign_language(alt, lang).is_none())
            .unwrap_or(&holders[0]);
        let source = &pages[origin.0].0.url;
        for (index, lang) in &holders {
            if lang == &origin.1 {
                continue;
            }
            let page = &pages[*index].0;
            sink.page(&page.url.to_string(), page.view);
            sink.emit(
                "images.alt_untranslated",
                "images.alt_untranslated",
                None,
                &[("alt", short(alt, 200)), ("other", source.to_string())],
            );
        }
    }
}

#[cfg(test)]
#[path = "set_tests.rs"]
mod tests;
