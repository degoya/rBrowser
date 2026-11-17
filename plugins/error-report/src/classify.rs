//! What each captured entry is: one of the report's categories, with the key that makes it the
//! same error in every view, or nothing.
//!
//! - **JS exceptions** - every uncaught exception; the key is its message without numbers and ids
//!   and its stack trace without positions (the script when there is none).
//! - **HTTP** - a response 404 or 410 (`http-not-found`) or 500 and up (`http-server-error`), any
//!   resource type, the document included; the key is the status and the URL without its cache
//!   buster. A request that failed without an answer (blocked, offline) is none.
//! - **Mixed content, CSP, deprecations** - the host's `Audits` issues; for a view without them
//!   ([`crate::feed::ViewFeed::issues`] `None`) the console's `security` and `deprecation`
//!   messages instead, so nothing is counted twice.

use rb_plugin_api::{Severity, ViewId};

use crate::feed::{Console, Exception, Issue, Request, ViewFeed};
use crate::normalize::{self, split_stack, url_key};

/// The longest text kept per entry.
pub const MAX_TEXT_CHARS: usize = 2000;

/// A category of the report; one rule each.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Category {
    JsException,
    HttpNotFound,
    HttpServerError,
    MixedContentBlocked,
    MixedContentPassive,
    CspViolation,
    Deprecation,
}

impl Category {
    /// Every category, in the report's order.
    pub const ALL: [Self; 7] = [
        Self::JsException,
        Self::HttpNotFound,
        Self::HttpServerError,
        Self::MixedContentBlocked,
        Self::MixedContentPassive,
        Self::CspViolation,
        Self::Deprecation,
    ];

    /// Its rule id (`describe`, findings, settings, answers).
    #[must_use]
    pub const fn rule(self) -> &'static str {
        match self {
            Self::JsException => "js-exception",
            Self::HttpNotFound => "http-not-found",
            Self::HttpServerError => "http-server-error",
            Self::MixedContentBlocked => "mixed-content-blocked",
            Self::MixedContentPassive => "mixed-content-passive",
            Self::CspViolation => "csp-violation",
            Self::Deprecation => "deprecation",
        }
    }

    /// Its name in i18n keys (`rules.<name>`, `findings.<name>`).
    #[must_use]
    pub fn key_name(self) -> String {
        self.rule().replace('-', "_")
    }

    /// The category of rule id `rule`.
    #[must_use]
    pub fn parse(rule: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|category| category.rule() == rule)
    }

    /// The usual severity: errors for exceptions, failed responses and blocked mixed content,
    /// warnings for CSP violations and passive mixed content, a note for deprecations.
    #[must_use]
    pub const fn severity(self) -> Severity {
        match self {
            Self::JsException
            | Self::HttpNotFound
            | Self::HttpServerError
            | Self::MixedContentBlocked => Severity::Error,
            Self::MixedContentPassive | Self::CspViolation => Severity::Warning,
            Self::Deprecation => Severity::Info,
        }
    }
}

/// Where an entry came from: a script position, or the document or script that caused it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Source {
    pub url: String,
    pub line: Option<u64>,
    pub column: Option<u64>,
}

/// The request of an HTTP entry.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RequestInfo {
    pub method: String,
    pub status: Option<u64>,
    pub resource_type: Option<String>,
    pub mime_type: Option<String>,
}

/// One error of one view.
#[derive(Clone, Debug, PartialEq)]
pub struct Event {
    pub view: ViewId,
    pub category: Category,
    /// The dedupe key, the same for the same error in every view.
    pub key: String,
    pub text: String,
    /// The resource concerned (HTTP, mixed content, CSP).
    pub url: Option<String>,
    pub source: Option<Source>,
    pub stack: Option<String>,
    pub request: Option<RequestInfo>,
    /// The directive (CSP), resolution (mixed content) or type (deprecation).
    pub detail: Option<String>,
    /// Milliseconds since the epoch.
    pub time: f64,
}

/// The errors of one view, in the capture's order.
#[must_use]
pub fn events(feed: &ViewFeed) -> Vec<Event> {
    let mut out: Vec<Event> = feed
        .errors
        .iter()
        .map(|e| exception(feed.view, e))
        .collect();
    out.extend(feed.network.iter().filter_map(|r| http(feed.view, r)));
    match &feed.issues {
        Some(issues) => out.extend(issues.iter().filter_map(|i| issue(feed.view, i))),
        None => out.extend(feed.console.iter().filter_map(|c| console(feed.view, c))),
    }
    out
}

fn source(url: Option<&str>, line: Option<u64>, column: Option<u64>) -> Option<Source> {
    url.filter(|url| !url.is_empty()).map(|url| Source {
        url: url.to_owned(),
        line,
        column,
    })
}

fn key(category: Category, parts: &[&str]) -> String {
    let mut key = category.rule().to_owned();
    for part in parts {
        key.push('|');
        key.push_str(part);
    }
    key
}

fn event(view: ViewId, category: Category, key: String, text: &str, time: f64) -> Event {
    Event {
        view,
        category,
        key,
        text: normalize::cut(text, MAX_TEXT_CHARS),
        url: None,
        source: None,
        stack: None,
        request: None,
        detail: None,
        time,
    }
}

fn exception(view: ViewId, error: &Exception) -> Event {
    let (message, stack) = split_stack(&error.text);
    let place = match &stack {
        Some(stack) => normalize::stack_key(stack),
        None => url_key(error.url.as_deref().unwrap_or_default()),
    };
    let key = key(Category::JsException, &[&normalize::text(&message), &place]);
    let mut event = event(view, Category::JsException, key, &message, error.time);
    event.source = source(error.url.as_deref(), error.line, error.column);
    event.stack = stack.map(|s| normalize::cut(&s, MAX_TEXT_CHARS));
    event
}

fn http(view: ViewId, request: &Request) -> Option<Event> {
    let status = request.status?;
    let category = match status {
        404 | 410 => Category::HttpNotFound,
        500..=599 => Category::HttpServerError,
        _ => return None,
    };
    let method = if request.method.is_empty() {
        "GET"
    } else {
        &request.method
    };
    let key = key(category, &[&status.to_string(), &url_key(&request.url)]);
    let text = format!("{method} {} {status}", request.url);
    let mut event = event(view, category, key, &text, request.time);
    event.url = Some(normalize::cut(&request.url, MAX_TEXT_CHARS));
    event.request = Some(RequestInfo {
        method: method.to_owned(),
        status: Some(status),
        resource_type: request.resource_type.clone(),
        mime_type: request.mime_type.clone(),
    });
    Some(event)
}

fn issue(view: ViewId, issue: &Issue) -> Option<Event> {
    let url = issue.url.as_deref().filter(|url| !url.is_empty());
    let kind = issue.kind.as_deref().unwrap_or_default();
    let (category, key, text) = match issue.code.as_str() {
        "ContentSecurityPolicyIssue" => {
            let target = match url {
                Some(url) => url_key(url),
                None => format!(
                    "inline@{}",
                    url_key(issue.source_url.as_deref().unwrap_or_default())
                ),
            };
            let what = url.or(issue.detail.as_deref()).unwrap_or("inline");
            let report_only = if issue.report_only {
                " (report-only)"
            } else {
                ""
            };
            (
                Category::CspViolation,
                key(Category::CspViolation, &[kind, &target]),
                format!("{kind}: {what}{report_only}"),
            )
        }
        "MixedContentIssue" => {
            let category = if kind == "MixedContentBlocked" {
                Category::MixedContentBlocked
            } else {
                Category::MixedContentPassive
            };
            let what = issue.detail.as_deref().unwrap_or("resource");
            (
                category,
                key(category, &[&url_key(url.unwrap_or_default())]),
                format!("{what}: {}", url.unwrap_or_default()),
            )
        }
        "DeprecationIssue" => (
            Category::Deprecation,
            key(
                Category::Deprecation,
                &[
                    kind,
                    &url_key(issue.source_url.as_deref().unwrap_or_default()),
                ],
            ),
            kind.to_owned(),
        ),
        _ => return None,
    };
    let mut event = event(view, category, key, &text, issue.time);
    event.url = url.map(|url| normalize::cut(url, MAX_TEXT_CHARS));
    event.source = source(issue.source_url.as_deref(), issue.line, issue.column);
    event.detail = issue.kind.clone();
    Some(event)
}

/// The first quoted (`'...'`) URL in `text` whose scheme `accept` takes.
fn quoted_url(text: &str, accept: impl Fn(&str) -> bool) -> Option<&str> {
    text.split('\'')
        .skip(1)
        .step_by(2)
        .find(|part| accept(part))
}

fn console(view: ViewId, message: &Console) -> Option<Event> {
    let text = message.text.trim();
    let (category, url, detail) = if message.source == "security" && text.contains("Mixed Content")
    {
        let category = if text.contains("blocked") {
            Category::MixedContentBlocked
        } else {
            Category::MixedContentPassive
        };
        let insecure = quoted_url(text, |p| p.starts_with("http://") || p.starts_with("ws://"));
        (category, insecure, None)
    } else if message.source == "security" && text.contains("Content Security Policy") {
        let blocked = quoted_url(text, |p| p.contains("://"));
        let directive = text
            .split_once("directive: \"")
            .and_then(|(_, rest)| rest.split([' ', '"']).next())
            .filter(|d| !d.is_empty());
        (Category::CspViolation, blocked, directive)
    } else if message.source == "deprecation"
        || (message.source != "console-api" && text.starts_with("[Deprecation]"))
    {
        (Category::Deprecation, None, None)
    } else {
        return None;
    };
    let target = match url {
        Some(url) => url_key(url),
        None => normalize::text(text),
    };
    let key = key(category, &[detail.unwrap_or_default(), &target]);
    let mut event = event(view, category, key, text, message.time);
    event.url = url.map(str::to_owned);
    event.source = source(message.url.as_deref(), message.line, None);
    event.detail = detail.map(str::to_owned);
    Some(event)
}

#[cfg(test)]
#[path = "classify_tests.rs"]
mod tests;
