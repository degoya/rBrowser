//! What one check run saw: the requests, cookies and localStorage keys of the page before any
//! consent and, after "Reject all", once more. Two sources:
//!
//! - **The probe session** (`$consent`, [`from_probe`]): the core loads the page in a fresh,
//!   empty session of its own, captures it, clicks "Reject all" with this plugin's collector
//!   (`src/collect.js`, mode `reject`) when `consent-check.rejectStep` is on, captures again
//!   and discards the session (the consent probe, RB-094-09: the tool and the command answer
//!   the host's offer `$consentProbe` with the URL and the collectors, `tool.rs`, and are called
//!   again with the capture):
//!
//!   ```json
//!   { "$consent": { "url": "https://example.test/", "capturedAt": 1790000000,
//!       "session": { "fresh": true, "discarded": true },
//!       "banner": { "cmp": "onetrust", "visible": true, "rejected": true },
//!       "before": { "requests": [{ "url": "...", "resourceType": "Script" }],
//!                   "cookies": [{ "name": "_ga", "domain": ".example.test", "expires": 1821536000 }],
//!                   "storage": ["key"] },
//!       "afterReject": { ... } } }
//!   ```
//!
//! - **The view** ([`from_view`]), for `checks_run`, which is offered no probe: what a grid
//!   view already shows, read only - the current document's requests from the host's console feed
//!   (`$console`), the cookies sent to the view's own URL (`Network.getCookies`, first party
//!   only) and the collector in mode `read`. Its session is the view's own, so a consent given
//!   earlier hides findings; the answer says so.
//!
//! A cookie's value is never read into a [`Cookie`]: no answer, finding or export can carry it.

use rb_plugin_api::{Failure, FailureKind, PluginHost, ViewId, ViewInfo, codes};
use rb_util::text::cut;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::cmp::{self, Mode as CollectorMode};

/// The argument the core's probe capture arrives under.
pub const PROBE_KEY: &str = "$consent";
/// The argument the host's console feed arrives under.
pub const FEED_KEY: &str = "$console";
/// The most requests taken from one phase.
pub const MAX_REQUESTS: usize = 2000;
/// The most cookies taken from one phase.
pub const MAX_COOKIES: usize = 500;
/// The most storage keys taken from one phase.
pub const MAX_KEYS: usize = 200;
/// The longest URL kept, in characters.
const MAX_URL_CHARS: usize = 2048;
/// The longest name, domain or key kept, in characters.
const MAX_NAME_CHARS: usize = 256;

/// Where a run's capture comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// The core's fresh, discarded probe session.
    Probe,
    /// A grid view as it is, read only.
    View,
}

impl Mode {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Probe => "session",
            Self::View => "view",
        }
    }
}

/// One request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    pub url: String,
    /// CDP's `resourceType` (`Script`, `Image`, ...).
    pub resource_type: String,
}

/// One cookie, without its value.
#[derive(Clone, Debug, PartialEq)]
pub struct Cookie {
    pub name: String,
    pub domain: String,
    /// Seconds since the epoch; `None` for a session cookie.
    pub expires: Option<f64>,
}

/// What one phase saw.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Phase {
    pub requests: Vec<Request>,
    pub cookies: Vec<Cookie>,
    /// localStorage keys of the main frame.
    pub storage: Vec<String>,
}

/// The consent banner of the page.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Banner {
    /// The consent manager's id of `data/cmp.json`.
    pub cmp: Option<String>,
    pub visible: bool,
    /// "Reject all" was clicked (the probe only).
    pub rejected: bool,
}

/// One run.
#[derive(Clone, Debug, PartialEq)]
pub struct Run {
    pub mode: Mode,
    pub url: String,
    /// The grid view read (the view mode only).
    pub view: Option<ViewId>,
    /// Seconds since the epoch.
    pub captured_at: f64,
    pub before: Phase,
    pub after_reject: Option<Phase>,
    pub banner: Banner,
    /// The core reported the probe session deleted (the probe only).
    pub session_discarded: Option<bool>,
    /// The view's cookies could not be read.
    pub cookies_unread: bool,
}

fn text(value: &Value, max: usize) -> Option<String> {
    value
        .as_str()
        .filter(|text| !text.is_empty())
        .map(|text| cut(text, max))
}

fn request(value: &Value) -> Option<Request> {
    Some(Request {
        url: text(&value["url"], MAX_URL_CHARS)?,
        resource_type: text(&value["resourceType"], MAX_NAME_CHARS)
            .unwrap_or_else(|| "Other".to_owned()),
    })
}

/// A cookie of CDP's or the core's shape; `expires` of `-1` or `session: true` is a session
/// cookie. Whatever else the entry holds (its value) is not read.
fn cookie(value: &Value) -> Option<Cookie> {
    let session = value["session"] == Value::Bool(true);
    let expires = value["expires"]
        .as_f64()
        .filter(|expires| *expires > 0.0 && !session);
    Some(Cookie {
        name: text(&value["name"], MAX_NAME_CHARS)?,
        domain: text(&value["domain"], MAX_NAME_CHARS)?,
        expires,
    })
}

fn list<T>(value: &Value, max: usize, read: impl Fn(&Value) -> Option<T>) -> Vec<T> {
    value
        .as_array()
        .map(|items| items.iter().filter_map(read).take(max).collect())
        .unwrap_or_default()
}

fn keys(value: &Value) -> Vec<String> {
    list(value, MAX_KEYS, |key| text(key, MAX_NAME_CHARS))
}

fn phase(value: &Value) -> Phase {
    Phase {
        requests: list(&value["requests"], MAX_REQUESTS, request),
        cookies: list(&value["cookies"], MAX_COOKIES, cookie),
        storage: keys(&value["storage"]),
    }
}

fn banner(value: &Value) -> Banner {
    Banner {
        cmp: text(&value["cmp"], MAX_NAME_CHARS),
        visible: value["visible"] == Value::Bool(true),
        rejected: value["rejected"] == Value::Bool(true),
    }
}

/// The core's probe capture in `args`, when there is one with a URL.
#[must_use]
pub fn from_probe(args: &Value) -> Option<Run> {
    let probe = args.get(PROBE_KEY).filter(|probe| probe.is_object())?;
    let after = &probe["afterReject"];
    Some(Run {
        mode: Mode::Probe,
        url: text(&probe["url"], MAX_URL_CHARS)?,
        view: None,
        captured_at: probe["capturedAt"].as_f64().unwrap_or(0.0),
        before: phase(&probe["before"]),
        after_reject: after.is_object().then(|| phase(after)),
        banner: banner(&probe["banner"]),
        session_discarded: probe["session"]["discarded"].as_bool(),
        cookies_unread: false,
    })
}

/// The current document's requests of `view` in the console feed: those after its last
/// navigation (`navigatedAt`), the whole list without one.
fn feed_requests(args: &Value, view: ViewId) -> Vec<Request> {
    let Some(entry) = args[FEED_KEY]["views"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|entry| entry["view"].as_u64() == Some(u64::from(view)))
    else {
        return Vec::new();
    };
    let since = entry["navigatedAt"].as_f64().unwrap_or(f64::MIN);
    entry["network"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|item| item["time"].as_f64().unwrap_or(0.0) > since)
        .filter_map(request)
        .take(MAX_REQUESTS)
        .collect()
}

fn bad_output(what: &str) -> Failure {
    Failure::coded(FailureKind::BadData, codes::BAD_OUTPUT).with_param("what", what)
}

/// What a grid view shows, read only, at `now` (seconds since the epoch).
///
/// # Errors
///
/// The collector's failure (no page, no evaluate) or an answer it cannot read.
pub fn from_view(
    host: &mut dyn PluginHost,
    view: &ViewInfo,
    args: &Value,
    now: f64,
) -> Result<Run, Failure> {
    let answer = host.evaluate(view.id, &cmp::collector(CollectorMode::Read))?;
    let collected: Value = serde_json::from_str(&answer).map_err(|_| bad_output("collector"))?;
    if !collected.is_object() {
        return Err(bad_output("collector"));
    }
    let url = text(&collected["url"], MAX_URL_CHARS).unwrap_or_else(|| view.url.clone());
    let mut cookies = Vec::new();
    let mut cookies_unread = false;
    if url.starts_with("http://") || url.starts_with("https://") {
        let params = json!({ "urls": [url] }).to_string();
        match host.cdp_send(view.id, "Network.getCookies", &params) {
            Ok(answer) => match serde_json::from_str::<CookieAnswer>(&answer) {
                Ok(parsed) => cookies = list(&parsed.cookies, MAX_COOKIES, cookie),
                Err(_) => cookies_unread = true,
            },
            Err(_) => cookies_unread = true,
        }
    }
    let mut seen = banner(&collected["banner"]);
    seen.rejected = false;
    Ok(Run {
        mode: Mode::View,
        url,
        view: Some(view.id),
        captured_at: now,
        before: Phase {
            requests: feed_requests(args, view.id),
            cookies,
            storage: keys(&collected["storage"]),
        },
        after_reject: None,
        banner: seen,
        session_discarded: None,
        cookies_unread,
    })
}

/// `Network.getCookies`' answer; only the list is read.
#[derive(Deserialize)]
struct CookieAnswer {
    #[serde(default)]
    cookies: Value,
}

#[cfg(test)]
#[path = "capture_tests.rs"]
mod tests;
