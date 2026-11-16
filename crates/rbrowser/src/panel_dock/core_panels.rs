//! The core's own panels in the panels page (RB-080-69; Videos, RB-110-12): Findings was a window of its own
//! (`page_window.rs`); it is an entry at the top of the panels page's side bar now, shown in its
//! content area as a component of the page itself - no plugin frame, the page's own rights - and
//! floating or docked with the plugin panels. A core panel's key has no dot
//! (`#/panels/findings`), so it never meets a plugin's `<plugin>.<panel>`.

use rb_ipc::CoreToShell;
use serde_json::{Value, json};

use super::{show, state};
use crate::cdp::lock;
use crate::control::Core;
use crate::page_window::ROUTE_PANELS;
use crate::shell::page_url;

/// The findings of the checks (`panels.findings`, the Checks commands).
pub const FINDINGS: &str = "findings";
/// The recorded videos of the project (`capture.video.panel`, RB-110-12).
pub const VIDEOS: &str = "videos";
/// Every core panel, in the side bar's order.
pub const ALL: [&str; 2] = [FINDINGS, VIDEOS];

/// The panels page at core panel `key`; `None` for a key that is none.
#[must_use]
pub fn url(key: &str) -> Option<String> {
    ALL.contains(&key)
        .then(|| page_url(&format!("{ROUTE_PANELS}/{key}")))
}

/// The `pluginPanel` event that selects core panel `key` in an open panels page.
#[must_use]
pub fn select_message(key: &str) -> Value {
    json!({ "select": { "core": key } })
}

/// Opens the panels - their window or the dock, as `panels.dock` says - at core panel `key`: a
/// new page from its route, one open already from the `select` event. A key that is none opens
/// nothing.
pub fn open(core: &Core, key: &str) {
    let Some(url) = url(key) else {
        return;
    };
    show(core, url);
    core.send(&CoreToShell::PluginPanel {
        message: select_message(key),
    });
}

/// A panels page shows core panel `key` now (its `selectedCore` request): the page the panels
/// move with when they dock, undock or follow the setting.
///
/// # Errors
///
/// A key that is no core panel; nothing is kept then.
pub fn selected(core: &Core, key: &str) -> Result<(), String> {
    let url = url(key).ok_or_else(|| format!("`{key}` is no core panel"))?;
    lock(&state(core)).selected = Some(url);
    Ok(())
}

#[cfg(test)]
#[path = "core_panels_tests.rs"]
mod tests;
