//! The capture a probe hands the plugin as `$consent` (RB-094-09), in the shape
//! `plugins/consent-check/src/capture.rs` reads:
//!
//! ```json
//! { "url": "https://example.test/", "capturedAt": 1790000000,
//!   "session": { "fresh": true, "discarded": true },
//!   "banner": { "cmp": "onetrust", "visible": true, "rejected": true },
//!   "before": { "requests": [{ "url": "...", "resourceType": "Script" }], "requestsCut": false,
//!               "cookies": [{ "name": "_ga", "domain": ".example.test", "expires": 1821536000,
//!                             "session": false }],
//!               "storage": ["key"] },
//!   "afterReject": { ... } }
//! ```
//!
//! Only names leave the probe: a request's URL (masked as the `$console` feed masks it,
//! `rb_diag::page_text`) and CDP resource type; a cookie's name, domain, expiry and whether it is
//! a session cookie - its value, path, flags and partition are never copied; localStorage keys,
//! never values. Every list is bounded as the plugin bounds it.

use rb_diag::page_text::redact_page_text;
use rb_util::text::cut;
use serde_json::{Value, json};

/// The most requests kept of one phase.
pub const MAX_REQUESTS: usize = 2000;
/// The most cookies kept of one phase.
pub const MAX_COOKIES: usize = 500;
/// The most storage keys kept of one phase.
pub const MAX_KEYS: usize = 200;
/// The longest URL kept, in characters.
const MAX_URL_CHARS: usize = 2048;
/// The longest name, domain, key or resource type kept, in characters.
const MAX_NAME_CHARS: usize = 256;
/// The schemes of a request the capture keeps: those with a host to judge.
const SCHEMES: &[&str] = &["http://", "https://", "ws://", "wss://"];

fn text(value: &Value, max: usize) -> Option<String> {
    value
        .as_str()
        .filter(|text| !text.is_empty())
        .map(|text| cut(text, max))
}

/// A `Network.requestWillBeSent` event's parameters as `{url, resourceType}`; `None` for one
/// without an address that has a host (`data:`, `blob:`).
#[must_use]
pub fn request_of(params: &Value) -> Option<Value> {
    let url = params["request"]["url"].as_str()?;
    let has_host = SCHEMES.iter().any(|scheme| {
        url.get(..scheme.len())
            .is_some_and(|head| head.eq_ignore_ascii_case(scheme))
    });
    if !has_host {
        return None;
    }
    let kind = text(&params["type"], MAX_NAME_CHARS).unwrap_or_else(|| "Other".to_owned());
    Some(json!({
        "url": cut(&redact_page_text(url), MAX_URL_CHARS),
        "resourceType": kind,
    }))
}

/// One cookie of a `Network.getAllCookies` answer without its value: name, domain, expiry
/// (`null` for a session cookie) and the session flag.
fn cookie_of(cookie: &Value) -> Option<Value> {
    let session = cookie["session"] == Value::Bool(true)
        || cookie["expires"]
            .as_f64()
            .is_none_or(|expires| expires <= 0.0);
    let expires = if session {
        Value::Null
    } else {
        json!(cookie["expires"].as_f64())
    };
    Some(json!({
        "name": text(&cookie["name"], MAX_NAME_CHARS)?,
        "domain": text(&cookie["domain"], MAX_NAME_CHARS)?,
        "expires": expires,
        "session": session,
    }))
}

/// The cookies of a `Network.getAllCookies` answer, without values, at most [`MAX_COOKIES`].
#[must_use]
pub fn cookies_of(answer: &Value) -> Vec<Value> {
    answer["cookies"]
        .as_array()
        .map(|cookies| {
            cookies
                .iter()
                .filter_map(cookie_of)
                .take(MAX_COOKIES)
                .collect()
        })
        .unwrap_or_default()
}

/// The localStorage keys of a collector's answer, at most [`MAX_KEYS`].
#[must_use]
pub fn keys_of(collected: &Value) -> Vec<String> {
    collected["storage"]
        .as_array()
        .map(|keys| {
            keys.iter()
                .filter_map(|key| text(key, MAX_NAME_CHARS))
                .take(MAX_KEYS)
                .collect()
        })
        .unwrap_or_default()
}

/// The banner of the read collector's answer `read`, `rejected` when the reject collector's
/// answer `clicked` says it clicked.
#[must_use]
pub fn banner_of(read: &Value, clicked: Option<&Value>) -> Value {
    let banner = &read["banner"];
    json!({
        "cmp": text(&banner["cmp"], MAX_NAME_CHARS),
        "visible": banner["visible"] == Value::Bool(true),
        "rejected": clicked.is_some_and(|answer| answer["clicked"] == Value::Bool(true)),
    })
}

/// What one phase saw.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Phase {
    pub requests: Vec<Value>,
    /// More requests came than [`MAX_REQUESTS`].
    pub requests_cut: bool,
    pub cookies: Vec<Value>,
    pub storage: Vec<String>,
}

impl Phase {
    /// Adds the request of a `Network.requestWillBeSent` event's parameters.
    pub fn add_request(&mut self, params: &Value) {
        let Some(request) = request_of(params) else {
            return;
        };
        if self.requests.len() < MAX_REQUESTS {
            self.requests.push(request);
        } else {
            self.requests_cut = true;
        }
    }

    fn to_value(&self) -> Value {
        json!({
            "requests": self.requests,
            "requestsCut": self.requests_cut,
            "cookies": self.cookies,
            "storage": self.storage,
        })
    }
}

/// One probe's capture.
#[derive(Clone, Debug, PartialEq)]
pub struct Capture {
    pub url: String,
    /// Seconds since the epoch.
    pub captured_at: u64,
    /// The session held no cookie before the page loaded.
    pub fresh: bool,
    /// The session's cookies were deleted and its browser closed.
    pub discarded: bool,
    /// [`banner_of`].
    pub banner: Value,
    pub before: Phase,
    /// After "Reject all" was clicked; `None` without the step or without a click.
    pub after_reject: Option<Phase>,
}

impl Capture {
    /// The `$consent` argument.
    #[must_use]
    pub fn to_value(&self) -> Value {
        let mut value = json!({
            "url": self.url,
            "capturedAt": self.captured_at,
            "session": { "fresh": self.fresh, "discarded": self.discarded },
            "banner": self.banner,
            "before": self.before.to_value(),
        });
        if let Some(after) = &self.after_reject {
            value["afterReject"] = after.to_value();
        }
        value
    }
}

#[cfg(test)]
#[path = "shape_tests.rs"]
mod tests;
