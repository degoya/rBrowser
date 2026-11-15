//! Views added and removed in every layout mode (RB-080-41): `view_add`, `view_remove` and
//! `view_add_size`, each the command of `layout::view_edit` with its arguments. Which
//! breakpoints are open is the selection `layout.hidden` of `config.local.json`; every answer
//! names it as `hidden`.

use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};
use serde::Deserialize;
use serde_json::{Value, json};

use super::object;
use crate::RbMcpServer;
use crate::answer::{ToolResult, respond};
use crate::backend::ToolFailure;
use crate::target::TargetArg;

/// `view_add`.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AddParams {
    /// A hidden breakpoint id, or `"all"` for every hidden one.
    pub breakpoint: String,
    /// Canvas: where the view goes, in canvas DIP (both or neither); elsewhere ignored.
    #[serde(default)]
    pub x: Option<f64>,
    #[serde(default)]
    pub y: Option<f64>,
}

/// `view_remove`.
#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RemoveParams {
    /// One view: a view id or a breakpoint id shown once; the active view when absent.
    #[serde(default)]
    pub target: Option<TargetArg>,
}

/// `view_add_size`.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AddSizeParams {
    /// Width in CSS px, 1 to 16384; with `height`, unless `device` is given.
    #[serde(default)]
    pub width: Option<u32>,
    /// Height in CSS px, 1 to 16384.
    #[serde(default)]
    pub height: Option<u32>,
    /// A device preset's id (`devices_list`) instead of `width` and `height`: the view gets
    /// the preset's size, pixel ratio, user agent and touch.
    #[serde(default)]
    pub device: Option<String>,
    /// The breakpoint's id (letters, digits, `-`, `_`, `.`); `size-<width>x<height>` or the
    /// device's id when absent.
    #[serde(default)]
    pub name: Option<String>,
    /// A device pixel ratio, above 0 and at most 10; not with `device`.
    #[serde(default)]
    pub dpr: Option<f64>,
    /// Canvas: where the view goes, in canvas DIP (both or neither).
    #[serde(default)]
    pub x: Option<f64>,
    #[serde(default)]
    pub y: Option<f64>,
}

#[tool_router(router = views_edit_router, vis = "pub(crate)")]
impl RbMcpServer {
    #[tool(
        description = "Open the view of a hidden breakpoint again (breakpoint: its id, or \"all\" for every hidden one): the breakpoint leaves layout.hidden of config.local.json, the config is read again and the view opens in the layout in effect - in canvas mode at x/y (canvas DIP) when given, else after the others. Answers added (the breakpoints opened), hidden (the ones still hidden) and changed."
    )]
    pub(crate) async fn view_add(&self, Parameters(params): Parameters<AddParams>) -> ToolResult {
        let args = object([
            ("breakpoint", Some(Value::String(params.breakpoint))),
            ("x", params.x.map(Value::from)),
            ("y", params.y.map(Value::from)),
        ]);
        respond(self.command("view.add", args).await)
    }

    #[tool(
        description = "Remove one view (target: a view id or breakpoint id, default the active view) from every layout mode: its breakpoint goes into layout.hidden of config.local.json and the view closes (a window of its own included). The last view stays and the call is refused. Answers removed (the breakpoint), view, hidden and changed (false when it was hidden already). The breakpoint stays in the config; view_add opens it again."
    )]
    pub(crate) async fn view_remove(
        &self,
        Parameters(params): Parameters<RemoveParams>,
    ) -> ToolResult {
        // A view id goes through as `view`, a breakpoint id as `breakpoint` (one hidden
        // already answers changed: false rather than "not found"); none is the active view.
        let args = match params.target {
            None => object([]),
            Some(TargetArg::Id(id)) => object([("view", Some(Value::from(id)))]),
            Some(TargetArg::Name(name)) if name == "all" => {
                return respond::<Value>(Err(ToolFailure::new(
                    "target_not_single",
                    "view_remove acts on one view; pass a view id or a breakpoint id".to_owned(),
                )));
            }
            Some(TargetArg::Name(name)) => object([("breakpoint", Some(Value::String(name)))]),
        };
        respond(self.command("view.remove", args).await)
    }

    #[tool(
        description = "Add a view of a new size or of a device preset: pass width and height (dpr optional), or device (a device preset id: its size, pixel ratio, user agent and touch apply). A breakpoint (name, else size-<width>x<height> or an id made from the device id) is written into config.local.json, the config is read again and the view opens - an explicit layout.cells grid gets a column for it. A breakpoint of that id that exists with the same size or device is opened instead; one with another size or device is refused - choose another name. Answers what is open in the end: breakpoint, width, height, added (false when it existed), changed (false when its view was open already) and hidden; for a device breakpoint also device and dpr."
    )]
    pub(crate) async fn view_add_size(
        &self,
        Parameters(params): Parameters<AddSizeParams>,
    ) -> ToolResult {
        // Without a size or a device the command opens the picker in the window: not a tool's.
        let valid = matches!(
            (&params.device, params.width, params.height),
            (Some(_), None, None) | (None, Some(_), Some(_))
        );
        if !valid {
            return respond::<Value>(Err(ToolFailure::new(
                "invalid_arguments",
                "view_add_size takes width and height, or device".to_owned(),
            )));
        }
        let args = object([
            ("width", params.width.map(|width| json!(width))),
            ("height", params.height.map(|height| json!(height))),
            ("device", params.device.map(Value::String)),
            ("name", params.name.map(Value::String)),
            ("dpr", params.dpr.map(Value::from)),
            ("x", params.x.map(Value::from)),
            ("y", params.y.map(Value::from)),
        ]);
        respond(self.command("view.add_size", args).await)
    }
}

#[cfg(test)]
#[path = "views_edit_tests.rs"]
mod tests;
