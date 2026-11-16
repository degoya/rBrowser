//! `canonical.*`: exactly one canonical (a `<link rel="canonical">` or a `Link` header), written
//! absolute, pointing at an indexable page that answers 200 without a redirect; one that points
//! elsewhere is a hint, since it may be meant.

use rb_plugin_api::PluginHost;

use crate::audit::Page;
use crate::fetch;
use crate::page::{says_noindex, sel};
use crate::rules::{Element, Sink, short};
use crate::url::Url;

/// The `rel="canonical"` targets of `Link` header values (RFC 8288).
#[must_use]
pub fn link_header_canonicals<'a>(values: impl Iterator<Item = &'a str>) -> Vec<String> {
    let mut found = Vec::new();
    for value in values {
        let mut rest = value;
        while let Some(open) = rest.find('<') {
            let Some(close) = rest[open..].find('>') else {
                break;
            };
            let target = rest[open + 1..open + close].trim().to_owned();
            let after = &rest[open + close + 1..];
            let end = after.find('<').unwrap_or(after.len());
            let params = after[..end].split(',').next().unwrap_or_default();
            let canonical = params.split(';').any(|param| {
                param.split_once('=').is_some_and(|(key, value)| {
                    key.trim().eq_ignore_ascii_case("rel")
                        && value
                            .trim()
                            .trim_matches('"')
                            .split_ascii_whitespace()
                            .any(|rel| rel.eq_ignore_ascii_case("canonical"))
                })
            });
            if canonical {
                found.push(target);
            }
            rest = &after[end..];
        }
    }
    found
}

fn absolute(href: &str) -> bool {
    let lower = href.to_ascii_lowercase();
    lower.starts_with("https://") || lower.starts_with("http://")
}

pub fn check(host: &mut dyn PluginHost, page: &Page<'_>, sink: &mut Sink<'_>) {
    let Some((html, facts)) = page.primary() else {
        return;
    };
    let headers = page
        .fetched
        .response
        .as_ref()
        .map(|response| link_header_canonicals(response.header("link")))
        .unwrap_or_default();
    let element = html
        .select(&sel("link[rel]"))
        .find(|link| {
            link.value().attr("rel").is_some_and(|rel| {
                rel.split_ascii_whitespace()
                    .any(|t| t.eq_ignore_ascii_case("canonical"))
            })
        })
        .map(Element::of);
    let declared: Vec<&String> = facts.canonicals.iter().chain(headers.iter()).collect();
    let Some(first) = declared.first() else {
        sink.emit("canonical.missing", "canonical.missing", None, &[]);
        return;
    };
    if declared.len() > 1 {
        let values: Vec<&str> = declared.iter().map(|value| value.as_str()).collect();
        sink.emit(
            "canonical.multiple",
            "canonical.multiple",
            element.clone(),
            &[
                ("count", declared.len().to_string()),
                ("values", short(&values.join(", "), 1000)),
            ],
        );
    }
    if !absolute(first) {
        sink.emit(
            "canonical.relative",
            "canonical.relative",
            element.clone(),
            &[("href", short(first, 300))],
        );
    }
    let Some(target) = Some(first)
        .filter(|href| !href.is_empty())
        .and_then(|href| page.url.join(href))
    else {
        sink.emit(
            "canonical.target",
            "canonical.target_invalid",
            element,
            &[("href", short(first, 300))],
        );
        return;
    };
    if target == page.url {
        return;
    }
    let target_text = target.to_string();
    sink.emit(
        "canonical.not_self",
        "canonical.not_self",
        element.clone(),
        &[("target", target_text.clone())],
    );
    if !sink.wants("canonical.target") {
        return;
    }
    let (message, detail) = match fetch::get(host, &target) {
        Err(failure) => (
            "canonical.target_failed",
            failure.code().unwrap_or_default().to_owned(),
        ),
        Ok(response) if (300..400).contains(&response.status) => {
            ("canonical.target_redirect", response.status.to_string())
        }
        Ok(response) if response.status != 200 => {
            ("canonical.target_status", response.status.to_string())
        }
        Ok(response) => {
            let header = response.header("x-robots-tag").any(says_noindex);
            let meta = response.is_html() && {
                let doc = crate::audit::Doc::parse(&response.text());
                doc.facts.robots.iter().any(|value| says_noindex(value))
            };
            if !header && !meta {
                return;
            }
            ("canonical.target_noindex", String::new())
        }
    };
    sink.emit(
        "canonical.target",
        message,
        element,
        &[("target", target_text), ("detail", detail)],
    );
}

/// The canonical of a document resolved against its page, for comparisons.
#[must_use]
pub fn resolved(page_url: &Url, href: Option<&String>) -> Option<String> {
    href.and_then(|href| page_url.join(href))
        .map(|url| url.to_string())
}

#[cfg(test)]
#[path = "canonical_tests.rs"]
mod tests;
