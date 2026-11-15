//! The panel's columns: the views of the grid in grid order, each with its session - the
//! breakpoint's `session` in `.rbrowser/config.json` (`default` without one) - and whether the
//! plugin can read it. Views of one session share cookies and localStorage; Firefox and WebKit
//! views are columns too, shown as not readable (the `cdp` and `evaluate` capabilities reach
//! Chromium views only).

use std::collections::BTreeMap;

use rb_plugin_api::{PluginHost, ViewId};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::origin;

/// The project configuration the sessions are read from.
pub const CONFIG_PATH: &str = ".rbrowser/config.json";
/// The session of a breakpoint without one, and of a view with a free size.
pub const DEFAULT_SESSION: &str = "default";
/// The engine the plugin reads.
pub const CHROMIUM: &str = "chromium";

/// One view as a column.
#[derive(Clone, Debug, PartialEq)]
pub struct Column {
    pub id: ViewId,
    pub label: String,
    pub breakpoint: String,
    pub engine: String,
    pub session: String,
    pub url: String,
    /// The origin of `url`; `None` for `about:blank` and other non-web pages.
    pub origin: Option<String>,
}

impl Column {
    /// Whether the plugin reads and writes this view (Chromium only).
    #[must_use]
    pub fn supported(&self) -> bool {
        self.engine == CHROMIUM
    }

    /// The column as the panel and MCP show it, with its read `status` (`ok`, `unsupported`,
    /// `error`).
    #[must_use]
    pub fn to_json(&self, status: &str) -> Value {
        json!({
            "id": self.id,
            "label": self.label,
            "breakpoint": self.breakpoint,
            "engine": self.engine,
            "session": self.session,
            "url": self.url,
            "origin": self.origin,
            "status": status,
        })
    }
}

#[derive(Deserialize)]
struct ProjectConfig {
    #[serde(default)]
    breakpoints: Vec<BreakpointEntry>,
}

#[derive(Deserialize)]
struct BreakpointEntry {
    id: String,
    #[serde(default)]
    session: Option<String>,
}

/// The session of each breakpoint of the project's config; empty without a readable config.
pub fn sessions(host: &mut dyn PluginHost) -> BTreeMap<String, String> {
    let Ok(bytes) = host.project_read(CONFIG_PATH) else {
        return BTreeMap::new();
    };
    let Ok(config) = serde_json::from_slice::<ProjectConfig>(&bytes) else {
        return BTreeMap::new();
    };
    config
        .breakpoints
        .into_iter()
        .filter_map(|entry| Some((entry.id, entry.session.filter(|s| !s.is_empty())?)))
        .collect()
}

/// Every view as a column, in grid order.
pub fn columns(host: &mut dyn PluginHost) -> Vec<Column> {
    let views = host.views().unwrap_or_default();
    if views.is_empty() {
        return Vec::new();
    }
    let sessions = sessions(host);
    views
        .into_iter()
        .map(|view| Column {
            session: sessions
                .get(&view.breakpoint)
                .cloned()
                .unwrap_or_else(|| DEFAULT_SESSION.to_owned()),
            origin: origin::origin_of(&view.url),
            id: view.id,
            label: view.label,
            breakpoint: view.breakpoint,
            engine: view.engine,
            url: view.url,
        })
        .collect()
}

/// Which views a call means: `"all"` or none for every view, a view id for that one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    All,
    View(ViewId),
}

impl Target {
    /// `target` of a call's arguments; `None` for one that is neither `"all"` nor a view id.
    #[must_use]
    pub fn parse(target: &Value) -> Option<Self> {
        match target {
            Value::Null => Some(Self::All),
            Value::String(all) if all == "all" => Some(Self::All),
            value => value
                .as_u64()
                .and_then(|id| ViewId::try_from(id).ok())
                .map(Self::View),
        }
    }

    /// Whether `column` is meant.
    #[must_use]
    pub fn covers(self, column: &Column) -> bool {
        match self {
            Self::All => true,
            Self::View(id) => column.id == id,
        }
    }
}

#[cfg(test)]
#[path = "columns_tests.rs"]
mod tests;
