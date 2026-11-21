//! The arguments of the consent probe (RB-094-09): what the host takes out of a caller's
//! arguments, what it offers a plugin granted `consent-probe`, how the plugin asks for a probe
//! and what the second call carries.
//!
//! - Every call of every plugin loses the [`PROBE_KEY`] and [`ASK_KEY`] a caller sent
//!   ([`offered`]): no MCP client, panel or command argument can forge a capture.
//! - A plugin granted `consent-probe` finds `"$consentProbe": true` in its arguments; it may
//!   answer `{"$consentProbe": {"url", "read", "reject"}}` ([`requested`]): the address to load
//!   (`http`/`https` only, [`probe_url`]), its collector in mode `read` and, when its "Reject all"
//!   step is on, in mode `reject` (`null` otherwise).
//! - The host then runs the probe and calls the same tool or command once more with the
//!   capture under [`PROBE_KEY`] and without the offer ([`with_consent`]).

use rb_plugin_types::{Failure, FailureKind, codes};
use serde_json::{Map, Value};

/// The argument the host's capture arrives under.
pub const PROBE_KEY: &str = "$consent";
/// The host's offer in a call's arguments (`true`), and the plugin's request in its answer.
pub const ASK_KEY: &str = "$consentProbe";
/// The longest address probed, in characters.
pub const MAX_URL_CHARS: usize = 2048;
/// The longest collector a plugin hands over, in UTF-8 bytes.
pub const MAX_SCRIPT_BYTES: usize = 256 * 1024;

/// The probe a plugin asks for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProbeRequest {
    /// The address to load, checked by [`probe_url`].
    pub url: String,
    /// The plugin's collector in mode `read`: banner, storage keys.
    pub read: String,
    /// Its collector in mode `reject` ("Reject all" clicked); `None`: no reject step.
    pub reject: Option<String>,
}

fn invalid(reason: &str) -> Failure {
    Failure::coded(FailureKind::BadData, codes::INVALID).with_param("reason", reason)
}

/// `args` without the probe keys a caller put there and, for a plugin `granted` the probe,
/// with the offer; `null` becomes an object only for such a plugin, other non-objects stay as
/// they are.
#[must_use]
pub fn offered(args: &Value, granted: bool) -> Value {
    match args {
        Value::Object(map) => {
            let mut map = map.clone();
            map.remove(PROBE_KEY);
            map.remove(ASK_KEY);
            if granted {
                map.insert(ASK_KEY.to_owned(), Value::Bool(true));
            }
            Value::Object(map)
        }
        Value::Null if granted => {
            let mut map = Map::new();
            map.insert(ASK_KEY.to_owned(), Value::Bool(true));
            Value::Object(map)
        }
        other => other.clone(),
    }
}

/// Whether `args` (as [`offered`] made them) offer the probe.
#[must_use]
pub fn is_offered(args: &Value) -> bool {
    args.get(ASK_KEY) == Some(&Value::Bool(true))
}

/// `args` of the call after the probe: without the offer, `consent` under [`PROBE_KEY`].
#[must_use]
pub fn with_consent(args: &Value, consent: Value) -> Value {
    let mut map = match args {
        Value::Object(map) => map.clone(),
        _ => Map::new(),
    };
    map.remove(ASK_KEY);
    map.insert(PROBE_KEY.to_owned(), consent);
    Value::Object(map)
}

fn script(value: &Value, what: &str) -> Result<String, Failure> {
    match value.as_str() {
        Some(text) if !text.trim().is_empty() && text.len() <= MAX_SCRIPT_BYTES => {
            Ok(text.to_owned())
        }
        _ => Err(invalid(what)),
    }
}

/// The probe the plugin's answer `answer` asks for: `None` when it asks for none (no JSON
/// object, no [`ASK_KEY`]); `Some(Err)` - `plugins.error.invalid` - for a request without a
/// valid address or collector.
#[must_use]
pub fn requested(answer: &str) -> Option<Result<ProbeRequest, Failure>> {
    let Ok(Value::Object(answer)) = serde_json::from_str::<Value>(answer) else {
        return None;
    };
    answer.get(ASK_KEY).map(request_of)
}

fn request_of(request: &Value) -> Result<ProbeRequest, Failure> {
    let url = probe_url(request["url"].as_str().unwrap_or_default())?;
    let read = script(&request["read"], "consent probe read")?;
    let reject = match &request["reject"] {
        Value::Null => None,
        other => Some(script(other, "consent probe reject")?),
    };
    Ok(ProbeRequest { url, read, reject })
}

/// `text` as the address a probe loads: an `http` or `https` URL with a host, without user
/// information, whitespace, control characters or backslashes, at most [`MAX_URL_CHARS`]; in
/// its parsed form.
///
/// # Errors
///
/// `plugins.error.invalid` (reason `consent probe url`) for any other.
pub fn probe_url(text: &str) -> Result<String, Failure> {
    let refused = || invalid("consent probe url");
    if text.is_empty()
        || text.chars().count() > MAX_URL_CHARS
        || text
            .chars()
            .any(|c| c.is_whitespace() || c.is_control() || c == '\\')
    {
        return Err(refused());
    }
    let url = url::Url::parse(text).map_err(|_| refused())?;
    let plain = matches!(url.scheme(), "http" | "https")
        && url.host_str().is_some_and(|host| !host.is_empty())
        && url.username().is_empty()
        && url.password().is_none();
    if !plain || url.as_str().chars().count() > MAX_URL_CHARS {
        return Err(refused());
    }
    Ok(url.into())
}

#[cfg(test)]
#[path = "args_tests.rs"]
mod tests;
