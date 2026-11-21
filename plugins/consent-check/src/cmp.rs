//! The consent managers the check knows (`data/cmp.json`, rBrowser's own collection): each
//! one's banner and "Reject all" selectors and its own consent cookies, the button texts a
//! banner of an unknown manager is rejected by, and the cookie names that count as essential
//! (sessions, CSRF tokens, bot protection). The collector (`src/collect.js`) gets the selectors
//! and texts; the cookie names stay here.

use std::sync::OnceLock;

use serde::Deserialize;
use serde_json::{Value, json};

/// The bundled data.
pub const DATA_JSON: &str = include_str!("../data/cmp.json");
/// The collector, a function expression called with its configuration.
pub const COLLECT_JS: &str = include_str!("collect.js");
/// Where the collector's configuration goes in [`COLLECT_JS`].
pub const CONFIG_SLOT: &str = "__CONSENT_CONFIG__";

/// Where an entry of the data comes from.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct Source {
    pub id: String,
    pub name: String,
    pub licence: String,
}

/// One consent manager.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct Cmp {
    pub id: String,
    pub name: String,
    pub source: String,
    /// Selectors of its banner (the first visible one counts).
    pub banner: Vec<String>,
    /// Selectors of its "Reject all" button.
    pub reject: Vec<String>,
    /// Its own consent cookies (a trailing `*` is a prefix).
    pub cookies: Vec<String>,
}

/// `data/cmp.json`.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Data {
    pub version: u32,
    pub sources: Vec<Source>,
    pub cmps: Vec<Cmp>,
    /// Lowercase button texts that reject (`alle ablehnen`), in the order they are tried.
    pub reject_texts: Vec<String>,
    /// Cookie names that count as essential (a trailing `*` is a prefix).
    pub essential_cookies: Vec<String>,
}

/// The bundled data, parsed once.
///
/// # Panics
///
/// Never at run time: `data/cmp.json` is compiled in, and a test holds it valid.
#[must_use]
pub fn data() -> &'static Data {
    static DATA: OnceLock<Data> = OnceLock::new();
    DATA.get_or_init(|| serde_json::from_str(DATA_JSON).expect("data/cmp.json is valid"))
}

/// The name of the consent manager `id`.
#[must_use]
pub fn name_of(id: &str) -> Option<&'static str> {
    data()
        .cmps
        .iter()
        .find(|cmp| cmp.id == id)
        .map(|cmp| cmp.name.as_str())
}

fn matches(pattern: &str, name: &str) -> bool {
    match pattern.strip_suffix('*') {
        Some(prefix) => name.starts_with(prefix),
        None => pattern == name,
    }
}

/// Whether the cookie `name` counts as essential: one of the bundled names, a consent
/// manager's own, or one of `own` (`consent-check.essentialCookies`).
#[must_use]
pub fn essential(name: &str, own: &[String]) -> bool {
    let data = data();
    data.essential_cookies
        .iter()
        .chain(data.cmps.iter().flat_map(|cmp| cmp.cookies.iter()))
        .chain(own.iter())
        .any(|pattern| matches(pattern, name))
}

/// What the collector does: read the page, or click "Reject all".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Read only: the banner, the storage keys.
    Read,
    /// Click the banner's "Reject all" (only ever in the probe session, never in a view).
    Reject,
}

impl Mode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Reject => "reject",
        }
    }
}

/// The collector's configuration: the mode, each manager's selectors, the reject texts.
#[must_use]
pub fn collector_config(mode: Mode) -> Value {
    let data = data();
    let cmps: Vec<Value> = data
        .cmps
        .iter()
        .map(|cmp| json!({ "id": cmp.id, "banner": cmp.banner, "reject": cmp.reject }))
        .collect();
    json!({ "mode": mode.as_str(), "cmps": cmps, "rejectTexts": data.reject_texts })
}

/// The collector as one expression for `evaluate`.
#[must_use]
pub fn collector(mode: Mode) -> String {
    COLLECT_JS.replace(CONFIG_SLOT, &collector_config(mode).to_string())
}

#[cfg(test)]
#[path = "cmp_tests.rs"]
mod tests;
