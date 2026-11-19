//! The first-party plugins' group and recommendation (RB-091-04): `plugins/first-party.json`,
//! compiled in, names for every first-party plugin the first-run wizard's and the catalogue's
//! group (`checks`, `devtools`, `overlays`, `capture`, `tools`) and whether it belongs to the
//! recommended set a fresh installation starts with. The release's index takes the groups from the
//! same file (`rb-plugin-pack index --categories`).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use serde::Deserialize;

/// The list, as the repository keeps it.
const LIST: &str = include_str!("../../../../plugins/first-party.json");

/// The groups, in the order the wizard shows them.
pub const CATEGORIES: [&str; 5] = ["checks", "devtools", "overlays", "capture", "tools"];

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    category: String,
    #[serde(default)]
    recommended: bool,
}

fn list() -> &'static BTreeMap<String, Entry> {
    static PARSED: OnceLock<BTreeMap<String, Entry>> = OnceLock::new();
    PARSED.get_or_init(|| {
        serde_json::from_str(LIST).unwrap_or_else(|error| {
            tracing::error!(%error, "plugins/first-party.json does not read");
            BTreeMap::new()
        })
    })
}

/// The ids of the first-party plugins.
pub fn ids() -> BTreeSet<String> {
    list().keys().cloned().collect()
}

/// Whether `id` is a first-party plugin.
pub fn is_first_party(id: &str) -> bool {
    list().contains_key(id)
}

/// The group of plugin `id`.
pub fn category(id: &str) -> Option<&'static str> {
    list().get(id).map(|entry| entry.category.as_str())
}

/// The recommended set: what a fresh installation's first start installs without the wizard.
pub fn recommended() -> BTreeSet<String> {
    list()
        .iter()
        .filter(|(_, entry)| entry.recommended)
        .map(|(id, _)| id.clone())
        .collect()
}

#[cfg(test)]
#[path = "first_party_tests.rs"]
mod tests;
