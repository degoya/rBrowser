//! The rules over each page of the set as a target: it answers 200 itself (no redirect), is
//! indexable (no `noindex`, not blocked for search engines by `robots.txt`), its canonical points
//! to itself - not to the page of another language - and its `lang` and `Content-Language`
//! name the language its code gives it; with `seo.hreflang.checkContentLanguage` its text is
//! held to that language too (a warning: a guess).

use crate::config::Config;
use crate::hreflang::code::{self, Checked, Code};
use crate::hreflang::load::{Answer, Loaded, Skipped};
use crate::rules::images::alt::foreign_language;
use crate::rules::{Sink, short};
use crate::url::Url;

/// What the set knows of the other pages: each page's URL and the code it is given.
pub struct Codes<'a> {
    pub by_url: &'a [(Url, String)],
}

impl Codes<'_> {
    /// The code the set gives `url`.
    #[must_use]
    pub fn of(&self, url: &Url) -> Option<&str> {
        self.by_url
            .iter()
            .find(|(page, _)| page == url)
            .map(|(_, code)| code.as_str())
    }
}

fn valid(code: Option<&str>) -> Option<Code> {
    match code.map(code::check) {
        Some(Checked::Valid(code)) => Some(code),
        _ => None,
    }
}

fn answer(page: &Loaded, sink: &mut Sink<'_>) -> bool {
    let (message, detail) = match &page.answer {
        Answer::Status(200) => return true,
        Answer::Status(status) => ("hreflang.target_status", status.to_string()),
        Answer::Redirect(status) => ("hreflang.target_redirect", status.to_string()),
        Answer::Failed(code) => ("hreflang.target_failed", short(code, 100)),
        Answer::Skipped(reason) => {
            let message = match reason {
                Skipped::Robots => "hreflang.unchecked_robots",
                Skipped::Local => "hreflang.unchecked_local",
                Skipped::Limit => "hreflang.unchecked_limit",
            };
            sink.emit("hreflang.target_unchecked", message, None, &[]);
            return false;
        }
    };
    sink.emit(
        "hreflang.target_status",
        message,
        None,
        &[("detail", detail)],
    );
    false
}

fn canonical(page: &Loaded, codes: &Codes<'_>, own: Option<&Code>, sink: &mut Sink<'_>) {
    let Some(target) = page
        .canonicals
        .first()
        .filter(|href| !href.is_empty())
        .and_then(|href| page.url.join(href))
    else {
        return;
    };
    if target == page.url {
        return;
    }
    let other = valid(codes.of(&target));
    let language = match (own, &other) {
        (Some(own), Some(other)) => own.language != other.language,
        _ => false,
    };
    let canonical = target.to_string();
    if language && let Some(other) = other {
        sink.emit(
            "hreflang.target_canonical",
            "hreflang.target_canonical_language",
            None,
            &[("canonical", canonical), ("code", other.to_string())],
        );
    } else {
        sink.emit(
            "hreflang.target_canonical",
            "hreflang.target_canonical",
            None,
            &[("canonical", canonical)],
        );
    }
}

fn language(config: &Config, page: &Loaded, code: &Code, sink: &mut Sink<'_>) {
    if let Some(lang) = &page.lang
        && !code.matches_lang(lang)
    {
        sink.emit(
            "hreflang.lang_mismatch",
            "hreflang.lang_mismatch",
            None,
            &[("lang", short(lang, 64)), ("code", code.to_string())],
        );
    }
    if !page.content_language.is_empty()
        && !page
            .content_language
            .iter()
            .any(|value| code.matches_lang(value))
    {
        sink.emit(
            "hreflang.lang_mismatch",
            "hreflang.content_language_mismatch",
            None,
            &[
                ("value", short(&page.content_language.join(", "), 200)),
                ("code", code.to_string()),
            ],
        );
    }
    if config.check_content_language
        && let Some(language) = foreign_language(&page.text, &code.language)
    {
        sink.emit(
            "hreflang.content_detected",
            "hreflang.content_detected",
            None,
            &[
                ("language", language.to_owned()),
                ("code", code.to_string()),
            ],
        );
    }
}

/// Every target rule over `page`, given `code` by the set.
pub fn check(config: &Config, page: &Loaded, codes: &Codes<'_>, sink: &mut Sink<'_>) {
    if page.engine_blocked {
        sink.emit(
            "hreflang.target_noindex",
            "hreflang.target_robots",
            None,
            &[],
        );
    }
    if !answer(page, sink) {
        return;
    }
    if page.noindex {
        sink.emit(
            "hreflang.target_noindex",
            "hreflang.target_noindex",
            None,
            &[],
        );
    }
    let own = valid(codes.of(&page.url));
    canonical(page, codes, own.as_ref(), sink);
    if let Some(own) = &own {
        language(config, page, own, sink);
    }
}

#[cfg(test)]
#[path = "target_tests.rs"]
mod tests;
