//! `render.differs`: what the raw HTML says against what the rendered DOM says. A canonical,
//! title, description, robots directive, language or `h1` that only JavaScript sets is seen by
//! a crawler that does not render (and late by one that does); one that JavaScript changes or
//! removes sends two signals.

use crate::audit::Page;
use crate::page::{Facts, says_noindex};
use crate::rules::canonical::resolved;
use crate::rules::{Sink, short};
use crate::url::Url;

fn fields(facts: &Facts, url: &Url) -> [(&'static str, Option<String>); 6] {
    let noindex = facts.robots.iter().any(|value| says_noindex(value));
    [
        ("canonical", resolved(url, facts.canonicals.first())),
        ("title", facts.titles.first().cloned()),
        ("description", facts.descriptions.first().cloned()),
        ("robots", noindex.then(|| "noindex".to_owned())),
        ("lang", facts.lang.clone()),
        (
            "h1",
            facts
                .headings
                .iter()
                .find(|heading| heading.level == 1)
                .map(|heading| heading.text.clone()),
        ),
    ]
}

pub fn check(page: &Page<'_>, sink: &mut Sink<'_>) {
    let (Some((_, rendered)), Some(raw)) = (&page.rendered, &page.raw) else {
        return;
    };
    let before = fields(&raw.facts, &page.url);
    let after = fields(rendered, &page.url);
    for ((field, raw), (_, rendered)) in before.into_iter().zip(after) {
        let message = match (&raw, &rendered) {
            (None, Some(_)) => "render.js_only",
            (Some(_), None) => "render.removed",
            (Some(a), Some(b)) if a != b => "render.changed",
            _ => continue,
        };
        sink.emit(
            "render.differs",
            message,
            None,
            &[
                ("field", field.to_owned()),
                ("raw", short(&raw.unwrap_or_default(), 200)),
                ("rendered", short(&rendered.unwrap_or_default(), 200)),
            ],
        );
    }
}

#[cfg(test)]
#[path = "diff_tests.rs"]
mod tests;
