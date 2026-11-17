//! RB-080-11, the group `wellknown`, once per origin and run: `/llms.txt` (llmstxt.org) and
//! `/.well-known/security.txt` (RFC 9116). `wellknown.llms_missing` (a hint),
//! `wellknown.llms_h1`, `wellknown.llms_structure` and `wellknown.llms_link` (a listed link that
//! does not answer 2xx, the first `seo.wellknown.maxLinks`); `wellknown.security_missing` (a
//! hint), `wellknown.security_location` (only at `/security.txt`), `wellknown.security_transport`
//! (not over HTTPS, or not `text/plain`), `wellknown.security_contact`,
//! `wellknown.security_expires` (missing, twice, unreadable or past) and
//! `wellknown.security_expires_far` (more than a year ahead). A 200 that is an HTML page (a
//! single-page app's fallback) counts as missing. Every fetch goes through the host.

use rb_plugin_api::PluginHost;

use crate::config::Config;
use crate::fetch::{self, ACCEPT_TEXT, Response};
use crate::rules::{Sink, short};
use crate::url::Url;

pub mod llms;
pub mod security;

use llms::Problem;

/// Redirects followed for the files and their links.
const HOPS: usize = 5;
/// How far ahead `Expires` may lie: a year, and a day for the leap year.
pub const MAX_EXPIRES_AHEAD: i64 = 366 * 86_400;

fn wants_any(sink: &Sink<'_>, prefix: &str) -> bool {
    crate::rules::RULES
        .iter()
        .any(|rule| rule.id.starts_with(prefix) && sink.wants(rule.id))
}

/// Whether the group runs at all in this run.
#[must_use]
pub fn wanted(sink: &Sink<'_>) -> bool {
    wants_any(sink, "wellknown.")
}

/// The file at `url`: a 200 that is no HTML page; `None` otherwise.
fn text_file(host: &mut dyn PluginHost, url: &Url) -> Option<Response> {
    let response = fetch::follow_with(host, url, HOPS, ACCEPT_TEXT).response?;
    let html = response.is_html()
        && String::from_utf8_lossy(&response.body[..response.body.len().min(512)])
            .trim_start()
            .starts_with('<');
    (response.status == 200 && !html).then_some(response)
}

pub fn check(host: &mut dyn PluginHost, config: &Config, start: &Url, sink: &mut Sink<'_>) {
    if wants_any(sink, "wellknown.llms_") {
        llms_txt(host, config, start, sink);
    }
    if wants_any(sink, "wellknown.security_") {
        security_txt(host, start, sink);
    }
}

fn llms_txt(host: &mut dyn PluginHost, config: &Config, start: &Url, sink: &mut Sink<'_>) {
    let Some(url) = Url::parse(&format!("{}/llms.txt", start.origin())) else {
        return;
    };
    sink.page(&url.to_string(), None);
    let Some(response) = text_file(host, &url) else {
        sink.emit(
            "wellknown.llms_missing",
            "wellknown.llms_missing",
            None,
            &[],
        );
        return;
    };
    let parsed = llms::parse(&response.text());
    for problem in &parsed.problems {
        let (rule, message, params) = match problem {
            Problem::NoH1 => ("llms_h1", "llms_no_h1", vec![]),
            Problem::H1NotFirst(line) => (
                "llms_h1",
                "llms_h1_not_first",
                vec![("line", line.to_string())],
            ),
            Problem::H1Multiple(line) => (
                "llms_h1",
                "llms_h1_multiple",
                vec![("line", line.to_string())],
            ),
            Problem::NoSummary => ("llms_structure", "llms_no_summary", vec![]),
            Problem::BadItem(line) => (
                "llms_structure",
                "llms_bad_item",
                vec![("line", line.to_string())],
            ),
            Problem::EmptySection(line, title) => (
                "llms_structure",
                "llms_empty_section",
                vec![("line", line.to_string()), ("section", title.clone())],
            ),
        };
        sink.emit(
            &format!("wellknown.{rule}"),
            &format!("wellknown.{message}"),
            None,
            &params,
        );
    }
    if !sink.wants("wellknown.llms_link") {
        return;
    }
    let base = Url::parse(&response.url).unwrap_or(url);
    for link in parsed.links.iter().take(config.llms_max_links) {
        let Some(target) = base.join(link) else {
            sink.emit(
                "wellknown.llms_link",
                "wellknown.llms_link_invalid",
                None,
                &[("url", short(link, 200))],
            );
            continue;
        };
        let mut fetched = fetch::follow_method(host, &target, HOPS, "HEAD", ACCEPT_TEXT);
        if fetched
            .response
            .as_ref()
            .is_some_and(|response| matches!(response.status, 405 | 501))
        {
            fetched = fetch::follow_with(host, &target, HOPS, ACCEPT_TEXT);
        }
        let status = fetched.response.as_ref().map(|response| response.status);
        if !status.is_some_and(|status| (200..300).contains(&status)) {
            let shown = status.map_or_else(|| "-".to_owned(), |status| status.to_string());
            sink.emit(
                "wellknown.llms_link",
                "wellknown.llms_link",
                None,
                &[("url", short(&target.to_string(), 200)), ("status", shown)],
            );
        }
    }
}

fn security_txt(host: &mut dyn PluginHost, start: &Url, sink: &mut Sink<'_>) {
    let origin = start.origin();
    let (Some(well_known), Some(legacy)) = (
        Url::parse(&format!("{origin}/.well-known/security.txt")),
        Url::parse(&format!("{origin}/security.txt")),
    ) else {
        return;
    };
    sink.page(&well_known.to_string(), None);
    let response = match text_file(host, &well_known) {
        Some(response) => response,
        None => match text_file(host, &legacy) {
            Some(response) => {
                sink.page(&legacy.to_string(), None);
                sink.emit(
                    "wellknown.security_location",
                    "wellknown.security_location",
                    None,
                    &[],
                );
                response
            }
            None => {
                sink.emit(
                    "wellknown.security_missing",
                    "wellknown.security_missing",
                    None,
                    &[],
                );
                return;
            }
        },
    };
    let now = i64::try_from(host.now_unix_seconds()).unwrap_or(i64::MAX);
    held(&response, now, sink);
}

/// The file's transport, `Contact` and `Expires`.
fn held(response: &Response, now: i64, sink: &mut Sink<'_>) {
    let url = Url::parse(&response.url);
    if url
        .as_ref()
        .is_some_and(|url| url.scheme() == "http" && !url.is_local())
    {
        sink.emit(
            "wellknown.security_transport",
            "wellknown.security_http",
            None,
            &[],
        );
    }
    let kind = response.header("content-type").next().unwrap_or_default();
    if !kind.trim().to_ascii_lowercase().starts_with("text/plain") {
        sink.emit(
            "wellknown.security_transport",
            "wellknown.security_type",
            None,
            &[("type", short(if kind.is_empty() { "-" } else { kind }, 100))],
        );
    }
    let fields = security::fields(&response.text());
    if fields.contact.iter().all(|contact| contact.is_empty()) {
        sink.emit(
            "wellknown.security_contact",
            "wellknown.security_contact",
            None,
            &[],
        );
    }
    let rule = "wellknown.security_expires";
    match fields.expires.as_slice() {
        [] => sink.emit(rule, "wellknown.security_expires_missing", None, &[]),
        [value] => {
            let shown = short(value, 60);
            match security::parse_time(value) {
                None => sink.emit(
                    rule,
                    "wellknown.security_expires_invalid",
                    None,
                    &[("value", shown)],
                ),
                Some(time) if time <= now => {
                    sink.emit(
                        rule,
                        "wellknown.security_expired",
                        None,
                        &[("value", shown)],
                    );
                }
                Some(time) if time - now > MAX_EXPIRES_AHEAD => sink.emit(
                    "wellknown.security_expires_far",
                    "wellknown.security_expires_far",
                    None,
                    &[("value", shown)],
                ),
                Some(_) => {}
            }
        }
        values => sink.emit(
            rule,
            "wellknown.security_expires_multiple",
            None,
            &[("count", values.len().to_string())],
        ),
    }
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
