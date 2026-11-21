//! The rules of the consent check (the host shows them as `consent-check.<rule>`) and the
//! findings made of them.
//!
//! A rule's title is `plugin.consent-check.rules.<id>`, a finding's message
//! `plugin.consent-check.findings.<id>` (a session cookie's `<id>_session`). A finding's `path`
//! names the phase and the host and kind or the cookie (`consent-check:before/cookie/<domain>/
//! <name>`), so each stays one finding across runs; its `data` carries the notice that this is a
//! technical pre-check and no legal advice, into every export of the findings. No finding ever
//! carries a cookie's value or a request's query.

use rb_plugin_api::{Finding, RuleInfo, Severity, ViewId};
use serde_json::Value;

/// A request to another site before any consent.
pub const REQUEST_BEFORE: &str = "request.before_consent";
/// A cookie that is not essential, set before any consent.
pub const COOKIE_BEFORE: &str = "cookie.before_consent";
/// A request to another site after "Reject all".
pub const REQUEST_AFTER: &str = "request.after_reject";
/// A cookie that is not essential, newly set after "Reject all".
pub const COOKIE_AFTER: &str = "cookie.after_reject";

/// The longest message parameter, in characters (the host takes 256).
pub const MAX_PARAM_CHARS: usize = 200;

/// Every rule with its severity, in the order the findings come.
pub const RULES: &[(&str, Severity)] = &[
    (REQUEST_BEFORE, Severity::Warning),
    (COOKIE_BEFORE, Severity::Warning),
    (REQUEST_AFTER, Severity::Error),
    (COOKIE_AFTER, Severity::Error),
];

/// The rules as `check.describe` answers them.
#[must_use]
pub fn describe() -> Vec<RuleInfo> {
    RULES
        .iter()
        .map(|(id, severity)| RuleInfo {
            id: (*id).to_owned(),
            title: crate::key(&format!("rules.{id}")),
            severity: *severity,
            shared_id: None,
            help_url: None,
        })
        .collect()
}

/// The severity of `rule`.
#[must_use]
pub fn severity(rule: &str) -> Severity {
    RULES
        .iter()
        .find(|(id, _)| *id == rule)
        .map_or(Severity::Warning, |(_, severity)| *severity)
}

/// The message key of a finding of `rule`; a session cookie's has its own.
#[must_use]
pub fn message(rule: &str, session: bool) -> String {
    if session {
        crate::key(&format!("findings.{rule}_session"))
    } else {
        crate::key(&format!("findings.{rule}"))
    }
}

/// Whether `rule` runs: every rule when `enabled` is empty.
#[must_use]
pub fn enabled(rule: &str, enabled: &[String]) -> bool {
    enabled.is_empty() || enabled.iter().any(|id| id == rule)
}

/// Where a finding is.
pub struct Place<'a> {
    pub view: Option<ViewId>,
    pub url: &'a str,
}

/// A finding of `rule` at `place`, its `path` and `data`.
#[must_use]
pub fn finding(
    rule: &str,
    session: bool,
    params: &[(&str, String)],
    place: &Place<'_>,
    path: String,
    data: &Value,
) -> Finding {
    Finding {
        rule: rule.to_owned(),
        severity: severity(rule),
        message: message(rule, session),
        params: params
            .iter()
            .map(|(name, value)| {
                (
                    (*name).to_owned(),
                    value.chars().take(MAX_PARAM_CHARS).collect(),
                )
            })
            .collect(),
        view: place.view,
        url: Some(place.url.to_owned()),
        selector: None,
        path: Some(path),
        data: Some(data.to_string()),
    }
}
