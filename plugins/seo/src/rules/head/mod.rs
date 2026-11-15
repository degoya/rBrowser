//! RB-080-11, the group `head`: the order of the head as the raw HTML writes it (`scan`) - what
//! a parser sees first. `head.charset_late` (`<meta charset>` not within the first 1024 bytes),
//! `head.charset_not_first` (not the head's first element), `head.viewport_late` and
//! `head.title_late` (after the first external resource), `head.preconnect_late` (a preconnect
//! after a resource of its origin), `head.script_before_css` (a synchronous script before a
//! stylesheet), `head.in_body` (a head element in the body, often moved there by an element that
//! ended the head early) and `head.rendered_differs` (the rendered DOM orders charset, viewport
//! and title otherwise - a hint).

use crate::audit::Page;
use crate::rules::{Element, Sink, short};
use crate::url::Url;

pub mod scan;

use scan::{Scan, Tag};

/// The bytes a `<meta charset>` has to lie within (HTML's prescan).
pub const CHARSET_BYTES: usize = 1024;
/// The most `head.in_body` findings of one page.
const MAX_IN_BODY: usize = 20;

const RULES: [&str; 8] = [
    "head.charset_late",
    "head.charset_not_first",
    "head.viewport_late",
    "head.title_late",
    "head.preconnect_late",
    "head.script_before_css",
    "head.in_body",
    "head.rendered_differs",
];

fn is_charset(tag: &Tag) -> bool {
    tag.name == "meta"
        && (tag.attr("charset").is_some()
            || (tag
                .attr("http-equiv")
                .is_some_and(|name| name.eq_ignore_ascii_case("content-type"))
                && tag
                    .attr("content")
                    .is_some_and(|content| content.to_ascii_lowercase().contains("charset="))))
}

fn is_viewport(tag: &Tag) -> bool {
    tag.name == "meta"
        && tag
            .attr("name")
            .is_some_and(|name| name.trim().eq_ignore_ascii_case("viewport"))
}

fn is_stylesheet(tag: &Tag) -> bool {
    tag.name == "link" && tag.has_token("rel", "stylesheet")
}

/// An element that loads something: a script with `src`, a stylesheet, a preload or prefetch.
fn resource_url(tag: &Tag) -> Option<&str> {
    let url = match tag.name.as_str() {
        "script" => tag.attr("src"),
        "link"
            if ["stylesheet", "preload", "modulepreload", "prefetch"]
                .iter()
                .any(|rel| tag.has_token("rel", rel)) =>
        {
            tag.attr("href")
        }
        _ => None,
    }?;
    (!url.trim().is_empty()).then_some(url)
}

fn describe(tag: &Tag) -> String {
    let url = resource_url(tag).unwrap_or_default();
    short(&format!("<{} {}>", tag.name, url), 120)
}

fn at(tag: &Tag) -> Option<Element> {
    Some(Element::at(&format!("html > head > {}", tag.name)))
}

pub fn check(page: &Page<'_>, sink: &mut Sink<'_>) {
    if !RULES.iter().any(|rule| sink.wants(rule)) {
        return;
    }
    let Some(response) = page.fetched.response.as_ref().filter(|r| r.is_html()) else {
        return;
    };
    let raw = scan::scan(&response.text());
    charset(&raw, sink);
    order(&raw, &page.url, sink);
    in_body(&raw, sink);
    if let Some((rendered, _)) = &page.rendered {
        let rendered = scan::scan(&rendered.html.html());
        let (before, after) = (keys(&raw), keys(&rendered));
        if before != after {
            sink.emit(
                "head.rendered_differs",
                "head.rendered_differs",
                None,
                &[("raw", before.join(", ")), ("rendered", after.join(", "))],
            );
        }
    }
}

fn charset(scan: &Scan, sink: &mut Sink<'_>) {
    let Some(index) = scan.head.iter().position(is_charset) else {
        return;
    };
    let tag = &scan.head[index];
    if tag.end > CHARSET_BYTES {
        sink.emit(
            "head.charset_late",
            "head.charset_late",
            at(tag),
            &[("offset", tag.start.to_string())],
        );
    }
    if index > 0 {
        sink.emit(
            "head.charset_not_first",
            "head.charset_not_first",
            at(tag),
            &[("first", scan.head[0].name.clone())],
        );
    }
}

fn order(scan: &Scan, page: &Url, sink: &mut Sink<'_>) {
    let head = &scan.head;
    if let Some(first) = head.iter().position(|tag| resource_url(tag).is_some()) {
        let resource = describe(&head[first]);
        for (rule, found) in [
            ("head.viewport_late", head.iter().position(is_viewport)),
            (
                "head.title_late",
                head.iter().position(|tag| tag.name == "title"),
            ),
        ] {
            if let Some(index) = found.filter(|index| *index > first) {
                sink.emit(
                    rule,
                    rule,
                    at(&head[index]),
                    &[("resource", resource.clone())],
                );
            }
        }
    }
    for (index, tag) in head.iter().enumerate() {
        let hint = tag.name == "link"
            && (tag.has_token("rel", "preconnect") || tag.has_token("rel", "dns-prefetch"));
        let Some(origin) = hint
            .then(|| tag.attr("href"))
            .flatten()
            .and_then(|href| page.join(href))
            .map(|url| url.origin())
        else {
            continue;
        };
        let earlier = head[..index].iter().find(|earlier| {
            resource_url(earlier)
                .and_then(|url| page.join(url))
                .is_some_and(|url| url.origin() == origin)
        });
        if let Some(earlier) = earlier {
            sink.emit(
                "head.preconnect_late",
                "head.preconnect_late",
                at(tag),
                &[
                    ("origin", short(&origin, 120)),
                    ("resource", describe(earlier)),
                ],
            );
        }
    }
    let sync = head.iter().position(|tag| {
        tag.name == "script"
            && resource_url(tag).is_some()
            && tag.attr("async").is_none()
            && tag.attr("defer").is_none()
            && !tag
                .attr("type")
                .is_some_and(|kind| kind.trim().eq_ignore_ascii_case("module"))
    });
    if let Some(script) = sync
        && let Some(css) = head[script..].iter().find(|tag| is_stylesheet(tag))
    {
        sink.emit(
            "head.script_before_css",
            "head.script_before_css",
            at(&head[script]),
            &[
                ("script", describe(&head[script])),
                ("stylesheet", describe(css)),
            ],
        );
    }
}

/// A body tag that belongs into the head: `title`, `base`, a `meta` that is no Microdata
/// property, a `link` to a canonical, alternate, icon or manifest.
fn belongs_in_head(tag: &Tag) -> bool {
    match tag.name.as_str() {
        "title" | "base" => true,
        "meta" => {
            tag.attr("itemprop").is_none() && tag.attr("property").is_none_or(|p| p.contains(':'))
        }
        "link" => {
            tag.attr("itemprop").is_none()
                && ["canonical", "alternate", "icon", "manifest"]
                    .iter()
                    .any(|rel| tag.has_token("rel", rel))
        }
        _ => false,
    }
}

fn in_body(scan: &Scan, sink: &mut Sink<'_>) {
    let stray = scan
        .body
        .iter()
        .filter(|tag| belongs_in_head(tag))
        .take(MAX_IN_BODY);
    for tag in stray {
        let element = Some(Element::at(&format!("html > body > {}", tag.name)));
        match &scan.breaker {
            Some(breaker) => sink.emit(
                "head.in_body",
                "head.in_body_after",
                element,
                &[
                    ("element", tag.name.clone()),
                    ("breaker", breaker.name.clone()),
                ],
            ),
            None => sink.emit(
                "head.in_body",
                "head.in_body",
                element,
                &[("element", tag.name.clone())],
            ),
        }
    }
}

/// The order of charset, viewport and title in a head.
fn keys(scan: &Scan) -> Vec<&'static str> {
    let mut keys = Vec::new();
    for tag in &scan.head {
        let key = if is_charset(tag) {
            "charset"
        } else if is_viewport(tag) {
            "viewport"
        } else if tag.name == "title" {
            "title"
        } else {
            continue;
        };
        if !keys.contains(&key) {
            keys.push(key);
        }
    }
    keys
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
