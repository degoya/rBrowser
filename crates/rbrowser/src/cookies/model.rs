//! What copying cookies reads, takes and answers (RB-040-05): a Chromium cookie as CDP's
//! `Network.getAllCookies` gives it, the arguments of `cookies.copy` / `cookies_copy`, and the
//! report - counts and reasons per view, never a cookie's value.

use std::collections::BTreeMap;

use rb_config::EngineKind;
use rb_engine::{Target, ViewId};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One cookie of a Chromium session (CDP `Network.Cookie`; fields this copy does not use are
/// ignored). Its `Debug` hides the value.
#[derive(Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChromeCookie {
    /// The name.
    pub name: String,
    /// The value: a secret.
    pub value: String,
    /// `.example.com` for a domain cookie, `example.com` for a host-only one.
    pub domain: String,
    /// The path.
    pub path: String,
    /// Seconds since the epoch; -1 for a session cookie.
    #[serde(default = "session_expiry")]
    pub expires: f64,
    /// `HttpOnly`.
    #[serde(default)]
    pub http_only: bool,
    /// `Secure`.
    #[serde(default)]
    pub secure: bool,
    /// A session cookie: it ends with the browser.
    #[serde(default)]
    pub session: bool,
    /// `Strict`, `Lax` or `None`; absent when the site set none (Chromium treats it as `Lax`).
    #[serde(default)]
    pub same_site: Option<String>,
    /// The partition of a partitioned cookie (CHIPS).
    #[serde(default)]
    pub partition_key: Option<Value>,
    /// A partitioned cookie whose partition cannot be named.
    #[serde(default)]
    pub partition_key_opaque: bool,
}

fn session_expiry() -> f64 {
    -1.0
}

impl std::fmt::Debug for ChromeCookie {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChromeCookie")
            .field("name", &self.name)
            .field("value", &"<masked>")
            .field("domain", &self.domain)
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

/// The cookies of a `Network.getAllCookies` answer; an entry that does not read is left out.
pub fn cookies_of(answer: &Value) -> Vec<ChromeCookie> {
    answer
        .get("cookies")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(|entry| ChromeCookie::deserialize(entry).ok())
                .collect()
        })
        .unwrap_or_default()
}

/// Where the cookies come from: a session by id, or the session of a view.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Source {
    /// The view's session; the view is read first.
    View(ViewId),
    /// A session (`default` or one of the project's `sessions`).
    Session(String),
}

/// Which external views get cookies: `"all"`, a view id, or a list of view ids (MCP's
/// breakpoint target).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Targets {
    /// `"all"` or one view.
    One(Target),
    /// These views.
    Many(Vec<ViewId>),
}

impl Targets {
    /// Whether `view` is one of them.
    pub fn covers(&self, view: ViewId) -> bool {
        match self {
            Targets::One(Target::All) => true,
            Targets::One(Target::View(one)) => *one == view,
            Targets::Many(views) => views.contains(&view),
        }
    }
}

/// The arguments of `cookies.copy` / `cookies_copy`; every one optional.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CopyArgs {
    /// The session to copy from; absent: each target view gets its own session's cookies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<Source>,
    /// The Firefox and WebKit views to copy into; every one when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<Targets>,
    /// Only the cookies of these sites (host names; a cookie of a parent domain counts);
    /// `engines.cookies.domainFilter` decides when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domains: Option<Vec<String>>,
}

/// Why a cookie was not copied.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SkipReason {
    /// Its site is outside the filter (`engines.cookies.domainFilter`, `domains`).
    OtherDomain,
    /// On the list `engines.cookies.exclude`.
    Excluded,
    /// A partitioned cookie (CHIPS): neither Firefox's nor WebKit's copy can name the partition.
    Partitioned,
    /// It expired.
    Expired,
    /// No name, no domain or a path that does not start with `/`.
    Invalid,
}

/// What one Firefox or WebKit view got.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewReport {
    /// The view.
    pub view: ViewId,
    /// Its engine.
    pub engine: EngineKind,
    /// Its session, whose cookies it got.
    pub session: String,
    /// Cookies the engine took.
    pub copied: usize,
    /// Cookies the engine refused.
    pub refused: usize,
    /// Cookies left out, by reason.
    pub skipped: BTreeMap<SkipReason, usize>,
    /// Whether the view loaded its page again.
    pub reloaded: bool,
    /// Why the view got nothing (its engine is not running, another session was asked for).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// The answer of `cookies.copy`: per view, counts and reasons only.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CopyReport {
    /// Cookies copied, over every view.
    pub copied: usize,
    /// Each Firefox and WebKit view asked for.
    pub views: Vec<ViewReport>,
    /// A note for the reader when nothing could be done (no Firefox or WebKit view).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl CopyReport {
    /// The views that got nothing because of an error.
    pub fn failed(&self) -> usize {
        self.views
            .iter()
            .filter(|view| view.error.is_some())
            .count()
    }

    /// Cookies left out or refused, over every view.
    pub fn skipped(&self) -> usize {
        self.views
            .iter()
            .map(|view| view.refused + view.skipped.values().sum::<usize>())
            .sum()
    }
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;
