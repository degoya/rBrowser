//! What MCP needs of navigation sync (RB-030-01 registers the tools): the per-view model of
//! `views_list`, and the coupling `sync_set { target: viewId, feature: "navigation", enabled }`
//! switches (`sync_hub::mcp::sync_set` calls [`super::couple`]). The tools `navigate`, `back`,
//! `forward` and `reload` with `target: "all"` reach the coupled views through
//! [`super::navigate`], [`super::back`], [`super::forward`] and [`super::reload`];
//! `breakpoint_update { syncNavigation }` changes the project, which navigation sync follows.

use rb_engine::ViewId;
use serde::{Deserialize, Serialize};

use super::for_host;
use crate::host::Host;

/// A view's navigation sync in `views_list`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewNavigation {
    /// Whether the view follows the active view's navigation and the explicit navigation of
    /// `target: "all"`; `false` for `breakpoints[].syncNavigation: false` or `Leader S U`.
    pub sync_navigation: bool,
}

/// `view`'s navigation sync for `views_list`; coupled before navigation sync runs.
pub fn view_navigation(host: &Host, view: ViewId) -> ViewNavigation {
    ViewNavigation {
        sync_navigation: for_host(host).is_none_or(|nav| nav.is_coupled(view)),
    }
}

#[cfg(test)]
#[path = "mcp_tests.rs"]
mod tests;
