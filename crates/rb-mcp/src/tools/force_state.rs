//! Forced pseudo-class states (RB-090-53): `views_force_state`, the commands
//! `view.force_state.set` (states on an element at a point or matching a selector) and
//! `view.force_state.reset` (`clear` without an element), once per resolved target.

use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};
use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::RbMcpServer;
use crate::answer::{ToolResult, respond};
use crate::backend::ToolFailure;
use crate::target::TargetArg;

/// `views_force_state`.
#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ForceStateParams {
    /// `"all"`, a view id or a breakpoint id: the views to force in; the active view when
    /// absent (every view for a `clear` without an element).
    #[serde(default)]
    pub target: Option<TargetArg>,
    /// The states the element gets, replacing what it had: `hover`, `active`, `focus`,
    /// `focus-visible`, `focus-within`, `visited`, `target` (with or without the colon);
    /// `["hover"]` when absent, `[]` takes the element's states back.
    #[serde(default)]
    pub states: Option<Vec<String>>,
    /// A CSS selector: its first match in each targeted view.
    #[serde(default)]
    pub selector: Option<String>,
    /// The point's x in CSS px of the page's viewport (with `y`), in `view`.
    #[serde(default)]
    pub x: Option<f64>,
    /// The point's y in CSS px of the page's viewport (with `x`), in `view`.
    #[serde(default)]
    pub y: Option<f64>,
    /// The view the point is in; the same element is found in the other targeted views. The
    /// target's view when it is one, else the active view.
    #[serde(default)]
    pub view: Option<u32>,
    /// `true`: take states back - the element's, or with neither point nor selector every
    /// forced state of the target.
    #[serde(default)]
    pub clear: Option<bool>,
}

#[tool_router(router = force_state_router, vis = "pub(crate)")]
impl RbMcpServer {
    #[tool(
        description = "Force CSS pseudo-class states on a page element so it stays styled as hovered, focused or active while nothing touches it - e.g. keep a CSS hover menu (li:hover > ul) open in every view to measure or screenshot it. Chromium's CSS.forcePseudoState: hover, active and focus-within reach every ancestor too, as a real hover does; menus that open only through JavaScript (mouseenter) do not react. The element: a point (x, y in CSS px of the page's viewport, in `view`; rBrowser's own overlays are passed over) - the same element is found in the other targeted views by the sync's selectors - or a CSS selector, matched in each view. states: hover, active, focus, focus-visible, focus-within, visited, target; default [\"hover\"], [] takes the element's back. clear: true takes the element's states back, or without point and selector every forced state of the target (default then every view). States stay until cleared or the page navigates; the status bar says while states are forced. Only Chromium views. target: \"all\", a view id or a breakpoint id; default the active view. Answers views: [{ view, states } | { view, missing: true } | { view, error }] and forced: the views with forced states."
    )]
    pub(crate) async fn views_force_state(
        &self,
        Parameters(params): Parameters<ForceStateParams>,
    ) -> ToolResult {
        respond(self.force_state(params).await)
    }
}

impl RbMcpServer {
    async fn force_state(&self, params: ForceStateParams) -> Result<Value, ToolFailure> {
        let clear = params.clear == Some(true);
        let point = match (params.x, params.y) {
            (Some(x), Some(y)) => Some((x, y)),
            (None, None) => None,
            _ => return Err(invalid("x and y come together")),
        };
        if point.is_some() && params.selector.is_some() {
            return Err(invalid("a point (x, y) or a selector, not both"));
        }
        if clear && params.states.is_some() {
            return Err(invalid("states or clear, not both"));
        }
        if point.is_none() && params.selector.is_none() {
            if clear {
                return self
                    .per_target("view.force_state.reset", params.target.as_ref(), json!({}))
                    .await;
            }
            // Without an element the command waits for a click in the view; an agent names one.
            return Err(invalid(
                "give the element: a point (x, y) or a selector; clear: true without one resets",
            ));
        }
        let mut args = Map::new();
        if let Some((x, y)) = point {
            args.insert("x".to_owned(), json!(x));
            args.insert("y".to_owned(), json!(y));
        }
        if let Some(selector) = params.selector {
            args.insert("selector".to_owned(), json!(selector));
        }
        if let Some(view) = params.view {
            args.insert("view".to_owned(), json!(view));
        }
        if let Some(states) = params.states {
            args.insert("states".to_owned(), json!(states));
        }
        if clear {
            args.insert("clear".to_owned(), json!(true));
        }
        self.per_target(
            "view.force_state.set",
            params.target.as_ref(),
            Value::Object(args),
        )
        .await
    }
}

fn invalid(text: &str) -> ToolFailure {
    ToolFailure::new("invalid_arguments", text)
}

#[cfg(test)]
#[path = "force_state_tests.rs"]
mod tests;
