//! Sync and input: `sync_set` (RB-020-02), `click`, `type`, `hover`, `press_key` (RB-020-03),
//! `scroll` (RB-020-04) and `bus_send` (RB-020-08). The host runs each with the function its job
//! delivered (`Backend::host_tool`): the arguments here are the tool's schema, the host checks
//! them again with its own model. Typed text never appears in an answer or a log.

use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::forward::Answers;
use crate::RbMcpServer;
use crate::answer::{ToolResult, respond};
use crate::backend::ToolFailure;
use crate::target::TargetArg;

/// What `sync_set` does.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum SyncAction {
    /// Switch sync, a kind of it or a view on or off (`enabled`).
    Set,
    /// The target view(s) load the active view's URL; their desync badge goes.
    Resync,
    /// Do not sync the next click.
    SkipNextClick,
}

/// The kind of sync `sync_set` switches.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum SyncFeature {
    /// Sync as a whole.
    All,
    /// Navigation.
    Navigation,
    /// Scrolling.
    Scroll,
    /// Clicks.
    Click,
    /// Text input.
    Input,
    /// Keyboard.
    Keyboard,
    /// Hover.
    Hover,
}

/// How scroll sync moves the views.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ScrollMode {
    /// The same content at the same edge (an anchor element).
    Anchor,
    /// Real wheel events, for pages that take over scrolling.
    Wheel,
}

/// `sync_set`.
#[derive(Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SyncSetParams {
    /// `"all"` (default: sync as a whole or one kind of it), a view id or a breakpoint id (that
    /// view in or out of sync, or its navigation with `feature: "navigation"`).
    #[serde(default, skip_serializing)]
    pub target: Option<TargetArg>,
    /// `set` (default), `resync` or `skipNextClick`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<SyncAction>,
    /// `set`: the kind to switch; sync as a whole when absent or `all`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feature: Option<SyncFeature>,
    /// `set`: on or off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    /// How scroll sync moves the views from now on; with it `set` needs no `enabled`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scroll_mode: Option<ScrollMode>,
}

/// A mouse button.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ClickButton {
    /// Primary.
    Left,
    /// Wheel.
    Middle,
    /// Secondary.
    Right,
}

/// A modifier key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub enum Modifier {
    /// Alt.
    Alt,
    /// Control.
    Control,
    /// Meta (the Windows key).
    Meta,
    /// Shift.
    Shift,
}

/// `click`.
#[derive(Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClickParams {
    /// `"all"`, a view id or a breakpoint id; the active view when absent.
    #[serde(default, skip_serializing)]
    pub target: Option<TargetArg>,
    /// The element, a CSS selector.
    pub selector: String,
    /// `left` (default), `middle` or `right`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub button: Option<ClickButton>,
    /// 1 (default) to 3.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub click_count: Option<u32>,
    /// Held during the click.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub modifiers: Vec<Modifier>,
}

/// `type`. Its `Debug` leaves the text out.
#[derive(Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TypeParams {
    /// `"all"`, a view id or a breakpoint id; the active view when absent.
    #[serde(default, skip_serializing)]
    pub target: Option<TargetArg>,
    /// The field, clicked first; the focused element when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selector: Option<String>,
    /// What to type.
    pub text: String,
}

impl std::fmt::Debug for TypeParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TypeParams")
            .field("target", &self.target)
            .field("selector", &self.selector)
            .field(
                "text",
                &format_args!("<{} chars>", self.text.chars().count()),
            )
            .finish()
    }
}

/// `hover`.
#[derive(Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HoverParams {
    /// `"all"`, a view id or a breakpoint id; the active view when absent.
    #[serde(default, skip_serializing)]
    pub target: Option<TargetArg>,
    /// The element, a CSS selector.
    pub selector: String,
}

/// `press_key`.
#[derive(Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PressKeyParams {
    /// `"all"`, a view id or a breakpoint id; the active view when absent.
    #[serde(default, skip_serializing)]
    pub target: Option<TargetArg>,
    /// The key's name (Enter, Tab, Escape, ArrowDown, F5, ...) or one letter or digit.
    pub key: String,
    /// Held during the key.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub modifiers: Vec<Modifier>,
}

/// An end of the scroll range.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ScrollEdge {
    /// The start.
    Top,
    /// The end.
    Bottom,
}

/// `scroll`: one destination of `to`, `element` or the percentages.
#[derive(Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScrollParams {
    /// `"all"`, a view id or a breakpoint id; the active view when absent.
    #[serde(default, skip_serializing)]
    pub target: Option<TargetArg>,
    /// The scroll container (CSS selector); the page when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub container: Option<String>,
    /// `top` or `bottom`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<ScrollEdge>,
    /// The element to bring to the top edge (CSS selector).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub element: Option<String>,
    /// Horizontal share of the scroll range, 0 to 100.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub percent_x: Option<f64>,
    /// Vertical share of the scroll range, 0 to 100.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub percent_y: Option<f64>,
}

/// `bus_send`.
#[derive(Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BusSendParams {
    /// `"all"` (default), a view id or a breakpoint id.
    #[serde(default, skip_serializing)]
    pub target: Option<TargetArg>,
    /// The message type: 1 to 64 of A-Z a-z 0-9 _ . : / -.
    #[serde(rename = "type")]
    pub kind: String,
    /// Any JSON value, up to `bus.maxPayloadBytes`; `null` when left out.
    #[serde(default)]
    pub payload: Value,
}

/// `params` as the host tool's arguments (without `target`, which [`RbMcpServer::host_per_target`]
/// sets).
fn args_of(params: &impl Serialize) -> Result<Value, ToolFailure> {
    serde_json::to_value(params).map_err(|error| ToolFailure::new("internal", error.to_string()))
}

impl RbMcpServer {
    async fn host(
        &self,
        name: &str,
        target: Option<&TargetArg>,
        params: &impl Serialize,
        answers: Answers,
    ) -> ToolResult {
        let result = match args_of(params) {
            Ok(args) => self.host_per_target(name, target, args, answers).await,
            Err(failure) => Err(failure),
        };
        respond(result)
    }
}

#[tool_router(router = sync_router, vis = "pub(crate)")]
impl RbMcpServer {
    #[tool(
        description = "Switch the synchronisation between the views: action \"set\" (default) with `enabled` turns sync on or off as a whole (target \"all\"), one kind of it (`feature`: navigation, scroll, click, input, keyboard, hover) or one view (target a view id: out of sync while off; with feature \"navigation\" only its navigation, which then neither leads nor follows); action \"resync\" has a desynced view (or every one, target \"all\") load the active view's URL; action \"skipNextClick\" leaves the next click unsynced. `scrollMode` (\"anchor\" or \"wheel\") switches how scroll sync moves the views until the project's settings change. Answers the sync state: the switches, the excluded and the desynced views with the reason and the selector that failed."
    )]
    pub(crate) async fn sync_set(
        &self,
        Parameters(params): Parameters<SyncSetParams>,
    ) -> ToolResult {
        self.host("sync_set", params.target.as_ref(), &params, Answers::Last)
            .await
    }

    #[tool(
        description = "Click an element (CSS `selector`) as real input in the views of `target` (\"all\", a view id or a breakpoint id; the active view when absent): `button` left (default), middle or right, `clickCount` 1 to 3, `modifiers` (Alt, Control, Meta, Shift). Touch views get a tap. Answers per view done, notFound, notVisible or a failure."
    )]
    pub(crate) async fn click(&self, Parameters(params): Parameters<ClickParams>) -> ToolResult {
        self.host("click", params.target.as_ref(), &params, Answers::Joined)
            .await
    }

    #[tool(
        name = "type",
        description = "Type `text` as real input in the views of `target` (\"all\", a view id or a breakpoint id; the active view when absent): into the element of `selector` after a click on it, else into the focused element. Works with every keyboard layout. Answers per view; the text never appears in the answer."
    )]
    pub(crate) async fn type_text(&self, Parameters(params): Parameters<TypeParams>) -> ToolResult {
        self.host("type", params.target.as_ref(), &params, Answers::Joined)
            .await
    }

    #[tool(
        description = "Move the mouse onto an element (CSS `selector`) in the views of `target` (\"all\", a view id or a breakpoint id; the active view when absent), so :hover styles and hover menus open. Answers per view."
    )]
    pub(crate) async fn hover(&self, Parameters(params): Parameters<HoverParams>) -> ToolResult {
        self.host("hover", params.target.as_ref(), &params, Answers::Joined)
            .await
    }

    #[tool(
        description = "Press a key in the views of `target` (\"all\", a view id or a breakpoint id; the active view when absent): `key` is a key name (Enter, Tab, Escape, Backspace, Delete, ArrowUp/Down/Left/Right, Home, End, PageUp, PageDown, F1-F24, \" \") or one letter or digit, `modifiers` Alt, Control, Meta, Shift. Enter runs the default action (a form submit). Text goes through `type`. Answers per view."
    )]
    pub(crate) async fn press_key(
        &self,
        Parameters(params): Parameters<PressKeyParams>,
    ) -> ToolResult {
        self.host(
            "press_key",
            params.target.as_ref(),
            &params,
            Answers::Joined,
        )
        .await
    }

    #[tool(
        description = "Scroll the views of `target` (\"all\", a view id or a breakpoint id; the active view when absent) the way scroll sync does, so every view shows the same content: `to` \"top\" or \"bottom\", `element` (CSS selector) brought to the top edge, or `percentY` / `percentX` (0 to 100) of the scroll range; the other axis goes to its start. `container` (CSS selector) scrolls that element instead of the page. Answers per view done, notFound, notVisible or a failure."
    )]
    pub(crate) async fn scroll(&self, Parameters(params): Parameters<ScrollParams>) -> ToolResult {
        self.host("scroll", params.target.as_ref(), &params, Answers::Joined)
            .await
    }

    #[tool(
        description = "Send a message on the page API's message bus (window.__rbrowser.bus) to the pages of `target` (\"all\" (default), a view id or a breakpoint id): their bus.on(type) handlers get `payload` with from: null. `type` is 1 to 64 of A-Z a-z 0-9 _ . : / -; `payload` any JSON value up to bus.maxPayloadBytes. Answers the views whose page took it (delivered) and the ones that did not (skipped): a page with the bus or the page API off, or without the sync agent."
    )]
    pub(crate) async fn bus_send(
        &self,
        Parameters(params): Parameters<BusSendParams>,
    ) -> ToolResult {
        let target = params.target.clone().unwrap_or_else(TargetArg::all);
        self.host("bus_send", Some(&target), &params, Answers::Joined)
            .await
    }
}

#[cfg(test)]
#[path = "sync_tests.rs"]
mod tests;
