//! Pinned values: cookies and storage keys rBrowser sets again after every navigation. They live
//! in `.rbrowser/out/storage-panel/pins.json` - the project's `out/` folder, which `.rbrowser/
//! .gitignore` keeps out of the repository, because a pinned value may be a token - and are kept
//! by the panel. Applying them to a view sets the cookies over CDP and hands the storage keys of
//! the view's origin to `src/ops.js`, which sets them and keeps them in the tab's sessionStorage,
//! where `page/pins.js` sets them again at the next document start, before the page's scripts.
//!
//! A repository can still commit the file, so the pins are read only in a trusted workspace
//! (review finding M3 of Charge 7): without trust none is loaded or applied, and pinning,
//! unpinning and switching are refused, which also keeps the file from being overwritten.

use rb_plugin_api::{Failure, FailureKind, PluginHost, codes};
use serde_json::{Map, Value, json};

use crate::columns::Column;
use crate::cookie;
use crate::matrix::{Row, fnv};
use crate::origin;
use crate::read::{self, Kind};

/// The pins file, relative to the plugin's out folder.
pub const PINS_FILE: &str = "pins.json";
/// The pins file, relative to the project.
pub const PINS_PATH: &str = ".rbrowser/out/storage-panel/pins.json";
/// The most pins kept.
pub const MAX_PINS: usize = 100;
/// The longest storage key or value of a pin, in characters.
pub const MAX_PIN_CHARS: usize = 8192;
/// The most bytes of pinned storage values handed to one document (the page bridge carries
/// 64 KiB per message).
pub const MAX_RECORD_BYTES: usize = 48 * 1024;

/// One pinned value.
#[derive(Clone, Debug, PartialEq)]
pub struct Pin {
    pub kind: Kind,
    /// The origin a storage key belongs to; the origin a cookie was pinned from.
    pub origin: String,
    pub key: String,
    pub value: String,
    /// Only views of this session; every session when `None`.
    pub session: Option<String>,
    pub enabled: bool,
    /// A cookie's attributes (`domain`, `path`, `secure`, `httpOnly`, `sameSite`).
    pub cookie: Value,
}

fn is_session_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

impl Pin {
    /// The pin's stable id: kind, origin, key and session.
    #[must_use]
    pub fn id(&self) -> String {
        fnv(&[
            self.kind.name(),
            &self.origin,
            &self.key,
            self.session.as_deref().unwrap_or(""),
        ])
    }

    /// A pin as the file keeps it.
    #[must_use]
    pub fn to_json(&self) -> Value {
        let mut out = json!({
            "id": self.id(),
            "kind": self.kind.name(),
            "origin": self.origin,
            "key": self.key,
            "value": self.value,
            "session": self.session,
            "enabled": self.enabled,
        });
        if self.kind == Kind::Cookies {
            out["cookie"] = self.cookie.clone();
        }
        out
    }

    /// A pin as the file keeps it, checked.
    ///
    /// # Errors
    ///
    /// The name of the field that does not hold.
    pub fn parse(value: &Value) -> Result<Self, String> {
        let text = |key: &str| value.get(key).and_then(Value::as_str);
        let kind = text("kind")
            .and_then(Kind::parse)
            .ok_or_else(|| "kind".to_owned())?;
        let origin = text("origin")
            .filter(|origin| origin::is_origin(origin))
            .ok_or_else(|| "origin".to_owned())?;
        let key = text("key").ok_or_else(|| "key".to_owned())?;
        let pinned = text("value").ok_or_else(|| "value".to_owned())?;
        let session = match value.get("session") {
            None | Some(Value::Null) => None,
            Some(Value::String(name)) if is_session_name(name) => Some(name.clone()),
            Some(_) => return Err("session".to_owned()),
        };
        let enabled = value
            .get("enabled")
            .map_or(Some(true), Value::as_bool)
            .ok_or_else(|| "enabled".to_owned())?;
        let cookie = if kind == Kind::Cookies {
            let attrs = value.get("cookie").cloned().unwrap_or(json!({}));
            let spec = cookie::spec(key, pinned, &attrs)?;
            json!({ "domain": spec.domain, "path": spec.path, "secure": spec.secure,
                    "httpOnly": spec.http_only, "sameSite": spec.same_site,
                    "partition": spec.partition })
        } else {
            if key.is_empty()
                || key.chars().count() > MAX_PIN_CHARS
                || pinned.chars().count() > MAX_PIN_CHARS
            {
                return Err("value".to_owned());
            }
            Value::Null
        };
        Ok(Self {
            kind,
            origin: origin.to_owned(),
            key: key.to_owned(),
            value: pinned.to_owned(),
            session,
            enabled,
            cookie,
        })
    }

    /// The domain a pinned cookie is set for: its `domain`, else its origin's host.
    #[must_use]
    pub fn cookie_domain(&self) -> String {
        self.cookie["domain"]
            .as_str()
            .map(str::to_owned)
            .or_else(|| origin::host_of(&self.origin))
            .unwrap_or_default()
    }

    /// Whether the pin applies in a view of `session` showing a document of `doc_origin`.
    #[must_use]
    pub fn applies(&self, session: &str, doc_origin: &str) -> bool {
        if !self.enabled || self.session.as_deref().is_some_and(|s| s != session) {
            return false;
        }
        match self.kind {
            Kind::Cookies => origin::host_of(doc_origin)
                .is_some_and(|host| origin::domain_matches(&host, &self.cookie_domain())),
            Kind::Local | Kind::Session => self.origin == doc_origin,
        }
    }

    /// Whether the pin holds the entry of `row`.
    #[must_use]
    pub fn holds(&self, row: &Row) -> bool {
        let bare = |domain: &str| domain.trim_start_matches('.').to_ascii_lowercase();
        self.kind == row.kind
            && self.key == row.name
            && match self.kind {
                Kind::Cookies => {
                    bare(&self.cookie_domain()) == bare(&row.scope)
                        && self.cookie["path"].as_str() == Some(row.path.as_str())
                }
                Kind::Local | Kind::Session => self.origin == row.scope,
            }
    }
}

/// A folder workspace trust protects: the host refuses to list it while the workspace is not
/// trusted, before it looks it up (`docs/plugins.md`, `project.list`).
pub const TRUST_PROBE: &str = ".rbrowser/flows";

/// Whether the open workspace is trusted, as the host's answer for [`TRUST_PROBE`] tells it: a
/// listing or `not_found` is trust; every other answer (untrusted, no project) is not.
pub fn workspace_trusted(host: &mut dyn PluginHost) -> bool {
    match host.project_list(TRUST_PROBE) {
        Ok(_) => true,
        Err(failure) => failure.code.as_deref() == Some(codes::NOT_FOUND),
    }
}

/// The pins of the file, as [`load_trusted`]; none in an untrusted workspace.
pub fn load(host: &mut dyn PluginHost) -> Vec<Pin> {
    load_trusted(host).unwrap_or_default()
}

/// The pins of the file; entries that do not read are left out (only their count is logged,
/// never a value).
///
/// # Errors
///
/// `plugins.error.untrusted_workspace`: the workspace is not trusted, the file is not read.
pub fn load_trusted(host: &mut dyn PluginHost) -> Result<Vec<Pin>, Failure> {
    if !workspace_trusted(host) {
        return Err(Failure::coded(
            FailureKind::AccessDenied,
            codes::UNTRUSTED_WORKSPACE,
        ));
    }
    let Ok(bytes) = host.project_read(PINS_PATH) else {
        return Ok(Vec::new());
    };
    let file: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    let entries = file["pins"].as_array().cloned().unwrap_or_default();
    let mut pins: Vec<Pin> = Vec::new();
    let mut dropped = 0;
    for entry in &entries {
        match Pin::parse(entry) {
            Ok(pin) if pins.len() < MAX_PINS && pins.iter().all(|p| p.id() != pin.id()) => {
                pins.push(pin);
            }
            _ => dropped += 1,
        }
    }
    if dropped > 0 {
        host.log("warn", &format!("pins.json: {dropped} entries left out"));
    }
    Ok(pins)
}

/// Writes `pins` to the file.
///
/// # Errors
///
/// The host's, when the file cannot be written.
pub fn save(host: &mut dyn PluginHost, pins: &[Pin]) -> Result<(), Failure> {
    let file = json!({ "version": 1, "pins": pins.iter().map(Pin::to_json).collect::<Vec<_>>() });
    let text = serde_json::to_string_pretty(&file).unwrap_or_default();
    host.out_write(PINS_FILE, text.as_bytes())
}

/// The pinned storage values for a document of `doc_origin` in a view of `session`, as
/// `src/ops.js` and `page/pins.js` take them: `{"local": {...}, "session": {...}}`; pins past
/// [`MAX_RECORD_BYTES`] are left out.
#[must_use]
pub fn record(pins: &[Pin], session: &str, doc_origin: &str) -> Value {
    let mut local = Map::new();
    let mut tab = Map::new();
    let mut bytes = 0;
    for pin in pins.iter().filter(|pin| pin.applies(session, doc_origin)) {
        let area = match pin.kind {
            Kind::Local => &mut local,
            Kind::Session => &mut tab,
            Kind::Cookies => continue,
        };
        bytes += pin.key.len() + pin.value.len() + 8;
        if bytes > MAX_RECORD_BYTES {
            break;
        }
        area.insert(pin.key.clone(), json!(pin.value));
    }
    json!({ "local": local, "session": tab })
}

/// Whether a record holds no value.
#[must_use]
pub fn record_is_empty(record: &Value) -> bool {
    ["local", "session"]
        .iter()
        .all(|area| record[area].as_object().is_none_or(Map::is_empty))
}

/// Sets the pinned cookies that apply to a document of `doc_origin` in `column`; the codes of
/// the writes that failed.
pub fn apply_cookies(
    host: &mut dyn PluginHost,
    column: &Column,
    pins: &[Pin],
    doc_origin: &str,
) -> Vec<String> {
    let mut failures = Vec::new();
    for pin in pins
        .iter()
        .filter(|pin| pin.kind == Kind::Cookies && pin.applies(&column.session, doc_origin))
    {
        let Ok(spec) = cookie::spec(&pin.key, &pin.value, &pin.cookie) else {
            continue;
        };
        // The document's own origin: the cookie goes to the host the pin applies to.
        let params = cookie::set_params(&spec, doc_origin).to_string();
        if let Err(failure) = host.cdp_send(column.id, "Network.setCookie", &params) {
            failures.push(failure.code.unwrap_or_default());
        }
    }
    failures
}

/// Applies `pins` to `column` now: the cookies, and the storage values of the view's origin
/// (their record replaced, removed when none apply). Nothing for a view the plugin cannot reach
/// or a page without an origin. The codes of what failed.
pub fn apply_view(host: &mut dyn PluginHost, column: &Column, pins: &[Pin]) -> Vec<String> {
    let Some(doc_origin) = column.origin.clone().filter(|_| column.supported()) else {
        return Vec::new();
    };
    let mut failures = apply_cookies(host, column, pins, &doc_origin);
    let record = record(pins, &column.session, &doc_origin);
    let op = json!({ "op": "pins", "origin": doc_origin, "local": record["local"],
                     "session": record["session"] });
    match host.evaluate(column.id, &read::script(&op)) {
        Ok(answer) => {
            let answer: Value = serde_json::from_str(&answer).unwrap_or(Value::Null);
            if answer["ok"] == false && answer["reason"] != "origin" {
                failures.push(answer["reason"].as_str().unwrap_or("failed").to_owned());
            }
        }
        Err(failure) => failures.push(failure.code.unwrap_or_default()),
    }
    failures
}

#[cfg(test)]
#[path = "pins_tests.rs"]
mod tests;
