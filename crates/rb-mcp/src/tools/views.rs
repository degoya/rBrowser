//! Views and navigation: `views_list`, `navigate`, `reload`, `back`, `forward`, a view in a
//! window of its own, `view_pop_out` / `view_pop_in` (RB-080-14), and the engine of a view,
//! `views_set_engine` (RB-080-33, the command `view.set_engine`). The navigation
//! tools answer every view they reached with its final URL ([`crate::settle`]), the views of
//! `"all"` they left alone (`skipped`) and the ones they failed in (`failed`).

use rb_engine::ViewId;
use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::NoArgs;
use super::object;
use crate::RbMcpServer;
use crate::answer::{ToolResult, error_result, json_result, respond};
use crate::backend::ToolFailure;
use crate::settle::{Navigation, settle};
use crate::target::{TargetArg, resolve, resolve_one};

/// `navigate`.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NavigateParams {
    /// The address, http(s) or a file inside the open project (no other scheme, no link);
    /// `example.test` becomes `https://example.test/`.
    pub url: String,
    /// `"all"` (default), a view id or a breakpoint id.
    #[serde(default)]
    pub target: Option<TargetArg>,
}

/// How `reload` reloads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ReloadMode {
    /// The pages (`reload.all`).
    Full,
    /// The stylesheets in place, no reload (`reload.css_only`, RB-030-05).
    Css,
}

/// `reload`.
#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReloadParams {
    /// `"all"` (default), a view id or a breakpoint id.
    #[serde(default)]
    pub target: Option<TargetArg>,
    /// Bypass the cache (a full reload).
    #[serde(default)]
    pub ignore_cache: bool,
    /// `full` (default) reloads the pages; `css` swaps every stylesheet in place, so JS state,
    /// scroll position and form values stay.
    #[serde(default)]
    pub mode: Option<ReloadMode>,
}

/// `back`, `forward`.
#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TargetParams {
    /// `"all"` (default), a view id or a breakpoint id.
    #[serde(default)]
    pub target: Option<TargetArg>,
}

/// `view_pop_out`, `view_pop_in`.
#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PopOutParams {
    /// One view: a view id or a breakpoint id shown once; the active view when absent.
    #[serde(default)]
    pub target: Option<TargetArg>,
}

/// The engines `views_set_engine` takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum EngineArg {
    /// CEF, in the grid.
    Chromium,
    /// The installed Firefox, in an external window.
    Firefox,
    /// Playwright's WebKit, in an external window.
    Webkit,
    /// The real Safari over safaridriver, in an external window; macOS only (RB-095-03).
    Safari,
    /// Mobile Safari on the iOS Simulator or a connected iPhone or iPad; macOS only (RB-095-04).
    Ios,
}

/// `views_set_engine`.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetEngineParams {
    /// `"all"`, a view id or a breakpoint id; the active view when absent.
    #[serde(default)]
    pub target: Option<TargetArg>,
    /// `chromium`, `firefox`, `webkit`, `safari` or `ios` (both macOS only).
    pub engine: EngineArg,
    /// `local` (`config.local.json`, personal, the default) or `project` (`config.json`,
    /// shared with the team).
    #[serde(default)]
    pub file: Option<String>,
    /// With engine `ios`: a simulator model (`iPhone 16`) or a udid from `ios_devices`; absent,
    /// engines.ios.defaultSimulator or the connected device.
    #[serde(default)]
    pub ios_device: Option<String>,
}

#[tool_router(router = views_router, vis = "pub(crate)")]
impl RbMcpServer {
    #[tool(
        description = "List the views of the grid in grid order: id (the target of every view tool), label, breakpoint (null after a free size), cellBreakpoint, engine, width, height, dpr, scale, url, title, loading, session, cell; desync (null while in sync, else reason, event, selector), syncNavigation, detached (in a window of its own), range (from/to: the widths the view stands for), external, externalWindow and embedded for a Firefox or WebKit view, frameworks (Vue and React versions with hook and devtools; {} when none)."
    )]
    pub(crate) async fn views_list(&self, _args: Parameters<NoArgs>) -> ToolResult {
        respond(self.read(|backend| backend.views_list()).await)
    }

    #[tool(
        description = "Load a URL (http or https, or a file: URL inside the open project; other schemes are refused) in the targeted views and wait for the loads (mcp.navigationTimeoutMs). Answers settled, per view its final URL after redirects, skipped (views of \"all\" whose navigation is decoupled) and failed."
    )]
    pub(crate) async fn navigate(
        &self,
        Parameters(params): Parameters<NavigateParams>,
    ) -> ToolResult {
        let extra = json!({ "url": params.url });
        self.navigation("view.navigate", params.target.as_ref(), extra)
            .await
    }

    #[tool(
        description = "Reload the targeted views. mode full (default) reloads the pages (ignoreCache bypasses the cache) and answers their final URLs once loaded; mode css swaps every stylesheet in place without a reload (JS state, scroll and form values stay) and answers per view."
    )]
    pub(crate) async fn reload(&self, Parameters(params): Parameters<ReloadParams>) -> ToolResult {
        if params.mode == Some(ReloadMode::Css) {
            return respond(
                self.per_target("reload.css_only", params.target.as_ref(), json!({}))
                    .await,
            );
        }
        let extra = json!({ "ignoreCache": params.ignore_cache });
        self.navigation("reload.all", params.target.as_ref(), extra)
            .await
    }

    #[tool(
        description = "Go back in the history of the targeted views and answer their final URLs; a view without history stays."
    )]
    pub(crate) async fn back(&self, Parameters(params): Parameters<TargetParams>) -> ToolResult {
        self.navigation("view.back", params.target.as_ref(), Value::Null)
            .await
    }

    #[tool(
        description = "Go forward in the history of the targeted views and answer their final URLs."
    )]
    pub(crate) async fn forward(&self, Parameters(params): Parameters<TargetParams>) -> ToolResult {
        self.navigation("view.forward", params.target.as_ref(), Value::Null)
            .await
    }

    #[tool(
        description = "Pop one view out of the grid into a window of its own at its real size (zoom 1, the breakpoint's width in CSS px), still synced, same view id, session and emulation; popOut.fitToScreen decides for a window larger than the screen. A view out already comes to the front. Answers view, detached and changed."
    )]
    pub(crate) async fn view_pop_out(
        &self,
        Parameters(params): Parameters<PopOutParams>,
    ) -> ToolResult {
        self.pop_command("view.pop_out", params.target.as_ref())
            .await
    }

    #[tool(
        description = "Put one view from its own window back into its grid cell; the window closes. Answers view, detached and changed (false when it was in the grid)."
    )]
    pub(crate) async fn view_pop_in(
        &self,
        Parameters(params): Parameters<PopOutParams>,
    ) -> ToolResult {
        self.pop_command("view.pop_in", params.target.as_ref())
            .await
    }

    #[tool(
        description = "Set the engine of the targeted views: chromium, firefox, webkit, safari (macOS only; follows, never leads) or ios (Mobile Safari on the Simulator or a connected iPhone or iPad, macOS only; iosDevice: a model such as \"iPhone 16\" or a udid from ios_devices). Written as breakpoints[].engine of each view's breakpoint into file local (config.local.json, default) or project (config.json); the view follows. An engine that cannot run here is refused with the reason. Answers per view { view, breakpoint, engine, file, changed, iosDevice? } - changed false when config.local.json shadows a project write - or { view, failed } for a view without a breakpoint of the config."
    )]
    pub(crate) async fn views_set_engine(
        &self,
        Parameters(params): Parameters<SetEngineParams>,
    ) -> ToolResult {
        let result = async {
            let targets: Vec<Option<ViewId>> = match &params.target {
                None => vec![None],
                Some(target) => {
                    let views = self.read(|backend| backend.views()).await?;
                    resolve(Some(target), &views)?
                        .view_ids(&views)
                        .into_iter()
                        .map(Some)
                        .collect()
                }
            };
            let engine = serde_json::to_value(params.engine)
                .map_err(|error| ToolFailure::new("mcp.internal", error.to_string()))?;
            // Several views go in one command: one config write, one reload, an answer per
            // view (`failed` for a view without a breakpoint of the config).
            let args = match targets.as_slice() {
                [one] => object([
                    ("view", one.map(|view| Value::from(view.0))),
                    ("engine", Some(engine)),
                    ("file", params.file.clone().map(Value::String)),
                    ("iosDevice", params.ios_device.clone().map(Value::String)),
                ]),
                many => object([
                    (
                        "views",
                        Some(Value::Array(
                            many.iter()
                                .flatten()
                                .map(|view| Value::from(view.0))
                                .collect(),
                        )),
                    ),
                    ("engine", Some(engine)),
                    ("file", params.file.clone().map(Value::String)),
                    ("iosDevice", params.ios_device.clone().map(Value::String)),
                ]),
            };
            self.command("view.set_engine", args).await
        }
        .await;
        respond(result)
    }
}

impl RbMcpServer {
    /// Runs `view.pop_out` or `view.pop_in` for the one view `target` names.
    async fn pop_command(&self, id: &str, target: Option<&TargetArg>) -> ToolResult {
        let result = async {
            let views = self.read(|backend| backend.views()).await?;
            let view = resolve_one(target, &views)?;
            let args = object([("view", view.map(|view| Value::from(view.0)))]);
            self.command(id, args).await
        }
        .await;
        respond(result)
    }

    /// Runs the navigation command `id` once per resolved target with `extra` and the target,
    /// then waits for the final URLs.
    async fn navigation(&self, id: &str, target: Option<&TargetArg>, extra: Value) -> ToolResult {
        match self.navigate_views(id, target, extra).await {
            Ok(answer) => json_result(&answer),
            Err(failure) => Ok(error_result(&failure)),
        }
    }

    async fn navigate_views(
        &self,
        id: &str,
        target: Option<&TargetArg>,
        extra: Value,
    ) -> Result<Navigation, ToolFailure> {
        let before = self.read(|backend| backend.views()).await?;
        let resolved = resolve(target, &before)?;
        let mut reached = Reached::default();
        for target in resolved.targets() {
            let mut args = match &extra {
                Value::Object(fields) => Value::Object(fields.clone()),
                _ => object([]),
            };
            args["target"] = serde_json::to_value(target)
                .map_err(|error| ToolFailure::new("mcp.internal", error.to_string()))?;
            reached.add(&self.command(id, args).await?);
        }
        // Only the views the command reached load: a skipped one would count as settled.
        let ids = reached.views.unwrap_or_else(|| resolved.view_ids(&before));
        let timeout = self.backend.options().navigation_timeout;
        let mut answer = settle(self.backend.as_ref(), &ids, &before, timeout).await;
        answer.skipped = reached.skipped;
        answer.failed = reached.failed;
        Ok(answer)
    }
}

/// What the navigation commands said they reached (`{ views, skipped, failed }`); `views` is
/// `None` while no command said (an older host: every resolved view).
#[derive(Default)]
struct Reached {
    views: Option<Vec<ViewId>>,
    skipped: Vec<ViewId>,
    failed: Vec<Value>,
}

impl Reached {
    fn add(&mut self, answer: &Value) {
        let ids = |key: &str| -> Option<Vec<ViewId>> {
            serde_json::from_value(answer.get(key)?.clone()).ok()
        };
        if let Some(views) = ids("views") {
            self.views.get_or_insert_with(Vec::new).extend(views);
        }
        self.skipped.extend(ids("skipped").unwrap_or_default());
        if let Some(Value::Array(failed)) = answer.get("failed") {
            self.failed.extend(failed.iter().cloned());
        }
    }
}

#[cfg(test)]
#[path = "views_tests.rs"]
mod tests;
