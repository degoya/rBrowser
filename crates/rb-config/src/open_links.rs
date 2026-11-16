//! `ui.openLinksIn`: where a link from rBrowser's own surfaces opens - a finding's help page, a
//! plugin panel's report or help link (RB-080-92). The command `app.open_link` follows it unless
//! it is told a target.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Where a link opens.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum OpenLinksIn {
    /// The system's default browser.
    #[default]
    System,
    /// The active view of the grid.
    ActiveView,
    /// Ask on every link: the system browser or the active view.
    Ask,
}

#[cfg(test)]
#[path = "open_links_tests.rs"]
mod tests;
