//! Breakpoints, emulation and layout: `breakpoints_detect`, `breakpoints_apply`,
//! `view_set_breakpoint`, `emulation_set`, `layout_set`, `canvas_place`, `canvas_zoom`
//! (RB-080-94) and `layout_save_preset` (RB-080-04), `environment_set` - each a command of the registry with the arguments its module reads.

use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};
use serde::Deserialize;
use serde_json::{Map, Value, json};

use super::object;
use crate::RbMcpServer;
use crate::answer::{ToolResult, error_result, json_result, respond};
use crate::backend::ToolFailure;
use crate::target::{TargetArg, resolve, resolve_one};

/// `breakpoints_detect`.
#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DetectParams {
    /// The pages to read; the project's start URL when empty.
    #[serde(default)]
    pub urls: Vec<String>,
    /// `all` (a view per breakpoint), `simplified` (five groups) or `edges` (a view at every
    /// edge +/- 1 px); the project's mode when absent.
    #[serde(default)]
    pub mode: Option<String>,
}

/// `breakpoints_apply`.
#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApplyParams {
    /// The proposal of a `breakpoints_detect` call; the last one when absent.
    #[serde(default)]
    pub proposal_id: Option<u32>,
    /// The proposed breakpoint ids to write; every new one when empty.
    #[serde(default)]
    pub selected: Vec<String>,
    /// Replace the config's breakpoints instead of adding to them.
    #[serde(default)]
    pub replace: bool,
}

/// `view_set_breakpoint`.
#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetSizeParams {
    /// One view (id or breakpoint id); the active view when absent.
    #[serde(default)]
    pub target: Option<TargetArg>,
    /// A breakpoint or device preset id.
    #[serde(default)]
    pub breakpoint: Option<String>,
    /// A free width in CSS px (instead of a breakpoint).
    #[serde(default)]
    pub width: Option<u32>,
    /// A free height in CSS px.
    #[serde(default)]
    pub height: Option<u32>,
    /// A free device pixel ratio.
    #[serde(default)]
    pub dpr: Option<f64>,
    /// Open a new view with it instead.
    #[serde(default)]
    pub new_view: bool,
}

/// `emulation_set`.
#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EmulationParams {
    /// `"all"`, a view id or a breakpoint id; the active view when absent.
    #[serde(default)]
    pub target: Option<TargetArg>,
    /// The emulation fields to set (a breakpoint's `emulation` in the config: colorScheme,
    /// reducedMotion, userAgent, locale, timezone, network, dir, lang, ...); absent fields
    /// stay; `null` for colorScheme, dir or lang drops the live override.
    #[serde(default)]
    pub fields: Map<String, Value>,
}

/// `layout_set`.
#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LayoutParams {
    /// A layout preset id of the config.
    #[serde(default)]
    pub preset: Option<String>,
    /// Focus mode on (one breakpoint large) or off (the grid).
    #[serde(default)]
    pub focus_mode: Option<bool>,
    /// The breakpoint focus mode shows.
    #[serde(default)]
    pub focus: Option<String>,
    /// The layout mode: `grid`, `focus`, `horizontal`, `split` or `canvas` (RB-080-04); the
    /// fields below set the mode's parameters (and are written into its preset).
    #[serde(default)]
    pub mode: Option<String>,
    /// Horizontal: the views' height, `"fit"` or DIP.
    #[serde(default)]
    pub height: Option<Value>,
    /// Horizontal: the gap between the views in DIP.
    #[serde(default)]
    pub gap: Option<u32>,
    /// Split: `vertical` (primary left) or `horizontal` (primary on top).
    #[serde(default)]
    pub orientation: Option<String>,
    /// Split: the primary side's share, 0.1 to 0.9.
    #[serde(default)]
    pub ratio: Option<f64>,
    /// Split: the breakpoints of the primary side.
    #[serde(default)]
    pub primary: Option<Vec<String>>,
    /// Split: the breakpoints of the other side.
    #[serde(default)]
    pub secondary: Option<Vec<String>>,
    /// Canvas: the snap grid in DIP (0 for none).
    #[serde(default)]
    pub snap: Option<u32>,
}

impl LayoutParams {
    /// The mode's arguments; `None` when no mode field is set.
    fn mode_args(&self) -> Option<Value> {
        let args = object([
            ("height", self.height.clone()),
            ("gap", self.gap.map(Value::from)),
            ("orientation", self.orientation.clone().map(Value::String)),
            ("ratio", self.ratio.map(Value::from)),
            ("primary", self.primary.clone().map(|ids| json!(ids))),
            ("secondary", self.secondary.clone().map(|ids| json!(ids))),
            ("snap", self.snap.map(Value::from)),
        ]);
        args.as_object()
            .is_some_and(|fields| !fields.is_empty())
            .then_some(args)
    }
}

/// `canvas_place`.
#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CanvasPlaceParams {
    /// The view: a view id or a breakpoint id; the active view when absent.
    #[serde(default)]
    pub target: Option<TargetArg>,
    /// The view's left edge on the canvas, in canvas DIP (snapped).
    pub x: f64,
    /// The view's top edge on the canvas, in canvas DIP (snapped; its label sits above).
    pub y: f64,
    /// The view's zoom, 0.1 to 1; unchanged when absent.
    #[serde(default)]
    pub zoom: Option<f64>,
}

/// `canvas_zoom`: a page zoom (`1` = 100 %) or `"fit"`.
#[derive(Debug, Clone, PartialEq, Deserialize, schemars::JsonSchema)]
#[serde(untagged)]
pub enum CanvasZoomLevel {
    /// The active view's page zoom: 1 for 100 %, up to 2.
    Level(f64),
    /// `"fit"`: every view in sight.
    Named(String),
}

/// `canvas_zoom`.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CanvasZoomParams {
    /// The page zoom of the active view (1 = 100 %, 0.5 = 50 %, at most 2), or `"fit"` for
    /// every view in sight; 100 % when absent.
    #[serde(default)]
    pub level: Option<CanvasZoomLevel>,
}

/// `layout_save_preset`.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SavePresetParams {
    /// The preset's name: 1 to 40 letters, digits, spaces, `-`, `_` or `.` (trimmed). A preset
    /// of that name in the same file is replaced.
    pub id: String,
    /// `local` (`config.local.json`, personal) or `project` (`config.json`, shared with the
    /// team); the user setting `layoutModes.savePresetTo` when absent.
    #[serde(default)]
    pub file: Option<String>,
}

/// `environment_set`.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EnvironmentParams {
    /// An environment id of the config.
    pub id: String,
}

#[tool_router(router = layout_router, vis = "pub(crate)")]
impl RbMcpServer {
    #[tool(
        description = "Find the breakpoints a site's CSS uses (media queries, container queries) and propose views for them; writes nothing. urls: the pages (default the start URL), mode: all, simplified or edges. The answer carries a proposalId for breakpoints_apply."
    )]
    pub(crate) async fn breakpoints_detect(
        &self,
        Parameters(params): Parameters<DetectParams>,
    ) -> ToolResult {
        let args = object([
            ("urls", Some(json!(params.urls))),
            ("mode", params.mode.map(Value::String)),
        ]);
        respond(self.command("breakpoints.analyze", args).await)
    }

    #[tool(
        description = "Write proposed breakpoints into .rbrowser/config.json: proposalId (default the last), selected ids (default every new one), replace instead of add."
    )]
    pub(crate) async fn breakpoints_apply(
        &self,
        Parameters(params): Parameters<ApplyParams>,
    ) -> ToolResult {
        let args = object([
            ("proposalId", params.proposal_id.map(Value::from)),
            ("selected", Some(json!(params.selected))),
            ("replace", Some(Value::Bool(params.replace))),
        ]);
        respond(self.command("breakpoints.append_detected", args).await)
    }

    #[tool(
        description = "Set one view (target: a view id or breakpoint id, default the active view) to a breakpoint or device preset, or to a free width/height/dpr; newView opens a new view instead."
    )]
    pub(crate) async fn view_set_breakpoint(
        &self,
        Parameters(params): Parameters<SetSizeParams>,
    ) -> ToolResult {
        let result = async {
            let views = self.read(|backend| backend.views()).await?;
            let view = resolve_one(params.target.as_ref(), &views)?;
            let args = object([
                ("view", view.map(|view| Value::from(view.0))),
                ("breakpoint", params.breakpoint.map(Value::String)),
                ("width", params.width.map(Value::from)),
                ("height", params.height.map(Value::from)),
                ("dpr", params.dpr.map(Value::from)),
                ("newView", params.new_view.then_some(Value::Bool(true))),
            ]);
            self.command("view.set_size", args).await
        }
        .await;
        respond(result)
    }

    #[tool(
        description = "Change the emulation of the targeted views live (\"all\", a view id or a breakpoint id; default the active view): fields as in a breakpoint's emulation - dir (ltr, rtl, auto) and lang (BCP 47) set <html dir> and <html lang> of the page without a reload (Chromium views); null for colorScheme, dir or lang drops the live override. Answers the views changed and whether a reload is needed."
    )]
    pub(crate) async fn emulation_set(
        &self,
        Parameters(params): Parameters<EmulationParams>,
    ) -> ToolResult {
        let result = async {
            let mut changed = Vec::new();
            let targets = match &params.target {
                None => vec![None],
                Some(target) => {
                    let views = self.read(|backend| backend.views()).await?;
                    resolve(Some(target), &views)?
                        .targets()
                        .into_iter()
                        .map(Some)
                        .collect()
                }
            };
            for target in targets {
                let args = object([
                    ("target", target.map(|target| json!(target))),
                    ("fields", Some(Value::Object(params.fields.clone()))),
                ]);
                changed.push(self.command("emulation.set", args).await?);
            }
            Ok::<_, ToolFailure>(match <[Value; 1]>::try_from(changed) {
                Ok([one]) => one,
                Err(many) => Value::Array(many),
            })
        }
        .await;
        respond(result)
    }

    #[tool(
        description = "Change the layout: preset selects a layout preset of the config; focusMode true shows one breakpoint large (focus names it), false the grid again; mode switches the current preset to grid, focus, horizontal (height, gap), split (orientation, ratio, primary, secondary) or canvas (snap) - the fields are written into the current preset when it has that mode, else into the top-level layout's `layout.modes`."
    )]
    pub(crate) async fn layout_set(
        &self,
        Parameters(params): Parameters<LayoutParams>,
    ) -> ToolResult {
        let mode_args = params.mode_args();
        if params.preset.is_none()
            && params.focus_mode.is_none()
            && params.focus.is_none()
            && params.mode.is_none()
        {
            return Ok(error_result(&ToolFailure::new(
                "invalid_arguments",
                "pass preset, focusMode, focus or mode",
            )));
        }
        let mode_command = match params.mode.as_deref() {
            None if mode_args.is_some() => {
                return Ok(error_result(&ToolFailure::new(
                    "invalid_arguments",
                    "the mode's fields need mode",
                )));
            }
            None => None,
            Some("focus") => Some(("layout.focus_mode", json!({ "enabled": true }))),
            Some(mode @ ("grid" | "horizontal" | "split" | "canvas")) => Some((
                match mode {
                    "grid" => "layout.mode.grid",
                    "horizontal" => "layout.mode.horizontal",
                    "split" => "layout.mode.split",
                    _ => "layout.mode.canvas",
                },
                mode_args.unwrap_or(Value::Null),
            )),
            Some(other) => {
                return Ok(error_result(&ToolFailure::new(
                    "invalid_arguments",
                    format!("unknown mode {other}: grid, focus, horizontal, split or canvas"),
                )));
            }
        };
        let result = async {
            if let Some(preset) = params.preset {
                self.command("layout.select_preset", json!({ "id": preset }))
                    .await?;
            }
            if params.focus_mode.is_some() || params.focus.is_some() {
                let args = object([
                    ("enabled", params.focus_mode.map(Value::Bool)),
                    ("focus", params.focus.map(Value::String)),
                ]);
                self.command("layout.focus_mode", args).await?;
            }
            if let Some((command, args)) = mode_command {
                self.command(command, args).await?;
            }
            Ok::<_, ToolFailure>(())
        }
        .await;
        match result {
            Ok(()) => json_result(&json!({ "ok": true })),
            Err(failure) => Ok(error_result(&failure)),
        }
    }

    #[tool(
        description = "Canvas layout: move one view (target: a view id or breakpoint id, default the active view) to x/y on the canvas in DIP, optionally with its zoom (0.1 to 1). The place snaps to the preset's grid and to neighbouring views' edges and is written into the current canvas preset, else into the top-level layout's `layout.modes.canvas`; the answer has the snapped x/y and whether the view is in sight."
    )]
    pub(crate) async fn canvas_place(
        &self,
        Parameters(params): Parameters<CanvasPlaceParams>,
    ) -> ToolResult {
        let result = async {
            let views = self.read(|backend| backend.views()).await?;
            let view = resolve_one(params.target.as_ref(), &views)?;
            let args = object([
                ("view", view.map(|view| Value::from(view.0))),
                ("x", Some(Value::from(params.x))),
                ("y", Some(Value::from(params.y))),
                ("zoom", params.zoom.map(Value::from)),
            ]);
            self.command("layout.canvas.place", args).await
        }
        .await;
        respond(result)
    }

    #[tool(
        description = "Canvas layout: zoom the canvas. level is the active view's page zoom - 1 shows its page at its real size (100 %), 0.5 at half, at most 2 (the largest view stays at 200 % at most) - or \"fit\" for every view in sight; 100 % when absent. The active view stays in the middle of the area. Answers { level }, the level set. Fails outside the canvas layout."
    )]
    pub(crate) async fn canvas_zoom(
        &self,
        Parameters(params): Parameters<CanvasZoomParams>,
    ) -> ToolResult {
        let (command, args) = match params.level {
            None => ("canvas.zoom.reset", Value::Null),
            Some(CanvasZoomLevel::Level(level)) if level.is_finite() && level > 0.0 => {
                ("canvas.zoom.reset", json!({ "level": level }))
            }
            // Every view in sight: the one zoom-to-fit command, held to the canvas (RB-090-63).
            Some(CanvasZoomLevel::Named(name)) if name == "fit" => {
                ("layout.zoom_to_fit", json!({ "canvas": true }))
            }
            Some(_) => {
                return Ok(error_result(&ToolFailure::new(
                    "invalid_arguments",
                    "level is a zoom above 0 (1 = 100 %) or \"fit\"",
                )));
            }
        };
        respond(self.command(command, args).await)
    }

    #[tool(
        description = "Save the layout shown now as a named layout preset and select it: the mode with its fields - the canvas with every view's place (not the camera), the split's ratio and sides, the strip's height, the grid in effect, focus mode's breakpoint. file is local (config.local.json, the default unless the user setting layoutModes.savePresetTo says project) or project (config.json, shared with the team); a preset of that id in that file is replaced. Answers { id, file, mode }."
    )]
    pub(crate) async fn layout_save_preset(
        &self,
        Parameters(params): Parameters<SavePresetParams>,
    ) -> ToolResult {
        let args = object([
            ("id", Some(Value::from(params.id))),
            ("file", params.file.map(Value::from)),
        ]);
        respond(self.command("layout.save_preset", args).await)
    }

    #[tool(description = "Switch to an environment of the config (its base URL in every view).")]
    pub(crate) async fn environment_set(
        &self,
        Parameters(params): Parameters<EnvironmentParams>,
    ) -> ToolResult {
        respond(
            self.command("environments.select", json!({ "id": params.id }))
                .await,
        )
    }
}

#[cfg(test)]
#[path = "layout_tests.rs"]
mod tests;
