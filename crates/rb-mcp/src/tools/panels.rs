//! The plugin panels' place (RB-080-57): `panels_dock`, the commands `panels.dock.left`,
//! `panels.dock.right`, `panels.dock.bottom`, `panels.undock` and `panels.close` by `position`
//! (`closed` as `panels.closeTo` says, `hidden` with `to: "hidden"`, RB-080-93); `collapsed`
//! folds a dock to its icon rail or opens it - `panels.dock.toggle` (RB-080-61) with a state
//! set, never flipped.
//! It acts on the window, on no view. Opening a panel's content is the plugins' own commands.

use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};
use serde::Deserialize;
use serde_json::Value;

use super::object;
use crate::RbMcpServer;
use crate::answer::{ToolResult, respond};
use crate::backend::ToolFailure;

/// Where the panels go.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum PanelPosition {
    /// Docked left of the views.
    Left,
    /// Docked right of the views.
    Right,
    /// Docked below the views.
    Bottom,
    /// In a window of their own.
    Floating,
    /// Closed: folded to the icon rail, or hidden, as the setting `panels.closeTo` says.
    Closed,
    /// Hidden: no rail, no window, until a panel is opened again.
    Hidden,
}

impl PanelPosition {
    fn command(self) -> &'static str {
        match self {
            Self::Left => "panels.dock.left",
            Self::Right => "panels.dock.right",
            Self::Bottom => "panels.dock.bottom",
            Self::Floating => "panels.undock",
            Self::Closed | Self::Hidden => "panels.close",
        }
    }

    fn docked(self) -> bool {
        matches!(self, Self::Left | Self::Right | Self::Bottom)
    }
}

/// `panels_dock`.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PanelsDockParams {
    /// `left`, `right` or `bottom` docks the panels there; `floating` puts them in a window of
    /// their own; `closed` closes them (to the icon rail, or hidden, as `panels.closeTo` says);
    /// `hidden` hides them.
    pub position: PanelPosition,
    /// The dock's width (left, right) or height (bottom) in DIP, 120 to 4000, kept for that side;
    /// the side's kept size when absent. Only with a docked position.
    #[serde(default)]
    pub size: Option<u32>,
    /// `true` folds the dock to a strip (the header below the views, the icon column beside
    /// them) so the views get the room, `false` opens it at its size; kept in
    /// `panels.dockState`. Absent: the fold stays as it is. Only with a docked position.
    #[serde(default)]
    pub collapsed: Option<bool>,
    /// The plugin whose panel to show, with `panel`.
    #[serde(default)]
    pub plugin: Option<String>,
    /// The panel to show, with `plugin`.
    #[serde(default)]
    pub panel: Option<String>,
}

#[tool_router(router = panels_router, vis = "pub(crate)")]
impl RbMcpServer {
    #[tool(
        description = "Place the plugin panels (Network, Console, Storage, Findings and the others): position left, right or bottom docks them in the main window (the views make room), floating puts them in a window of their own, closed closes them (to the icon rail or hidden, as panels.closeTo says), hidden hides them. Docking sets panels.dock and, with size (DIP, 120 to 4000), panels.dockSize; collapsed true folds the dock to its rail, false opens it. plugin and panel pick the panel shown. Answers position and, docked, size and collapsed; hidden answers state hidden."
    )]
    pub(crate) async fn panels_dock(
        &self,
        Parameters(params): Parameters<PanelsDockParams>,
    ) -> ToolResult {
        let result = async {
            if params.size.is_some() && !params.position.docked() {
                return Err(ToolFailure::new(
                    "args.invalid",
                    "size goes with left, right or bottom".to_owned(),
                ));
            }
            if params.collapsed.is_some() && !params.position.docked() {
                return Err(ToolFailure::new(
                    "args.invalid",
                    "collapsed goes with left, right or bottom".to_owned(),
                ));
            }
            let hidden = (params.position == PanelPosition::Hidden).then(|| Value::from("hidden"));
            let args = object([
                ("to", hidden),
                ("size", params.size.map(Value::from)),
                ("collapsed", params.collapsed.map(Value::from)),
                ("plugin", params.plugin.map(Value::from)),
                ("panel", params.panel.map(Value::from)),
            ]);
            self.command(params.position.command(), args).await
        }
        .await;
        respond(result)
    }
}

#[cfg(test)]
#[path = "panels_tests.rs"]
mod tests;
