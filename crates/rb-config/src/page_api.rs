//! `pageApi` and `bus`: the page API in every view's main world and its message bus (RB-020-08,
//! `docs/sync-protocol.md`), in Settings > Sync > Page API and Message bus.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// `pageApi`: `window.__rbrowser` and the user-agent suffix.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct PageApiConfig {
    /// `window.__rbrowser` (view info and message bus) in every view's page, from the next
    /// document on.
    pub enabled: bool,
    /// ` rBrowser/<version>` at the end of every view's user agent, an emulated one included.
    pub ua_suffix: bool,
}

impl Default for PageApiConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            ua_suffix: true,
        }
    }
}

/// `bus`: the message bus between the views' pages (`window.__rbrowser.bus`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct BusConfig {
    /// Messages of a page reach the same page in the other views.
    pub enabled: bool,
    /// The largest payload, in bytes of its JSON text; larger ones are refused with an error.
    #[schemars(range(min = 1, max = 196_608))]
    pub max_payload_bytes: u32,
    /// Messages per second a view may send; more are dropped.
    #[schemars(range(min = 1, max = 1000))]
    pub max_per_second: u32,
}

impl Default for BusConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_payload_bytes: 65_536,
            max_per_second: 100,
        }
    }
}

#[cfg(test)]
#[path = "page_api_tests.rs"]
mod tests;
