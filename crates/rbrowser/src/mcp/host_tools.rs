//! The tools the host runs itself (`rb_mcp::Backend::host_tool`) with the function each job
//! delivered: `sync_set` (RB-020-02), `click`, `type`, `hover`, `press_key` (RB-020-03),
//! `scroll` (RB-020-04), `bus_send` (RB-020-08), `screenshot` (RB-050-03), `recorder_export`
//! (RB-050-02), `recorder_play` (RB-090-66), `visual_compare` and `visual_baseline_update`
//! (RB-050-04), `update_status`, `update_check` and `update_install` (RB-090-14). The async
//! ones run on the host's runtime; `sync_set` and the export off the async workers. And what
//! `views_list` adds per view: its desync (RB-020-02), whether its navigation is synced
//! (RB-020-06) and its engine state (RB-040-01).

use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use rb_commands::CommandError;
use rb_engine::{ViewHost as _, ViewId};
use rb_mcp::ToolFailure;
use serde::Serialize;
use serde_json::{Value, json};

use crate::capture::{self, ScreenshotArgs};
use crate::control::Core;
use crate::recorder::export::RECORDER_EXPORT_TOOL;
use crate::recorder::play::RECORDER_PLAY_TOOL;
use crate::replay::mcp::{CLICK_TOOL, HOVER_TOOL, PRESS_KEY_TOOL, TYPE_TOOL};
use crate::scroll_sync::mcp::SCROLL_TOOL;
use crate::sync_agent::bus::{BUS_SEND_TOOL, BusSendArgs};
use crate::sync_hub::mcp::SYNC_SET_TOOL;
use crate::visual::VisualRunError;
use crate::visual::tool::{COMPARE_TOOL, CompareArgs, UPDATE_TOOL, UpdateArgs};

/// Runs the host tool `name` with `args` in `core`'s window.
///
/// # Errors
///
/// The tool's refusal or failure; `unknown_tool` for a name the host does not run.
pub async fn call(core: &Arc<Core>, name: &str, args: Value) -> Result<Value, ToolFailure> {
    let host = core.host().clone();
    // How long MCP waits for a view: the input tools, `sync_set`'s work.
    let timeout = Duration::from_millis(u64::from(core.user_settings().mcp.navigation_timeout_ms));
    match name {
        SYNC_SET_TOOL => {
            let blocking = core.clone();
            tokio::task::spawn_blocking(move || crate::sync_hub::mcp::sync_set(&blocking, &args))
                .await
                .map_err(|_| unfinished())??;
            // Answered once the agents have the new switches and resynced views have loaded.
            if let Some(sync) = crate::sync_hub::for_host(&host) {
                host.runtime()
                    .spawn(async move { sync.settled(timeout).await })
                    .await
                    .map_err(|_| unfinished())?;
            }
            let state = crate::sync_hub::mcp::current_state(core).map_err(ToolFailure::from)?;
            json(&state)
        }
        CLICK_TOOL => on_host(core, async move {
            crate::replay::mcp::click(&host, &args, timeout).await
        })
        .await
        .and_then(|answer| json(&answer)),
        TYPE_TOOL => on_host(core, async move {
            crate::replay::mcp::type_text(&host, &args, timeout).await
        })
        .await
        .and_then(|answer| json(&answer)),
        HOVER_TOOL => on_host(core, async move {
            crate::replay::mcp::hover(&host, &args, timeout).await
        })
        .await
        .and_then(|answer| json(&answer)),
        PRESS_KEY_TOOL => on_host(core, async move {
            crate::replay::mcp::press_key(&host, &args, timeout).await
        })
        .await
        .and_then(|answer| json(&answer)),
        SCROLL_TOOL => on_host(core, async move {
            crate::scroll_sync::mcp::scroll(&host, &args).await
        })
        .await
        .and_then(|answer| json(&answer)),
        BUS_SEND_TOOL => {
            let args: BusSendArgs = parse(args)?;
            let answer = on_host(core, async move {
                crate::sync_agent::bus::bus_send(&host, args)
                    .await
                    .map_err(|error| CommandError::Failed(error.to_string()))
            })
            .await?;
            json(&answer)
        }
        capture::tool::TOOL_NAME => screenshot(core, args).await,
        RECORDER_EXPORT_TOOL => {
            let blocking = core.clone();
            tokio::task::spawn_blocking(move || {
                crate::recorder::export::recorder_export(&blocking, &args)
            })
            .await
            .map_err(|_| unfinished())?
            .map_err(ToolFailure::from)
            .and_then(|answer| json(&answer))
        }
        RECORDER_PLAY_TOOL => crate::recorder::play::tool(core, args).await,
        // RB-092-01: the recording editor's steps; no tool deletes a recording.
        step if crate::recorder::steps::TOOLS.contains(&step) => {
            crate::recorder::steps::host_tool(core, step, args).await
        }
        COMPARE_TOOL => {
            let args = CompareArgs::parse(&args)
                .map_err(|message| ToolFailure::new("invalid_arguments", message))?;
            let run = core.clone();
            visual(
                core,
                async move { crate::visual::compare(&run, &args).await },
            )
            .await
        }
        UPDATE_TOOL => {
            let args = UpdateArgs::parse(&args)
                .map_err(|message| ToolFailure::new("invalid_arguments", message))?;
            let run = core.clone();
            visual(
                core,
                async move { crate::visual::update(&run, &args).await },
            )
            .await
        }
        // RB-060-10: the Web Vitals and resources per view.
        crate::vitals::VIEW_METRICS_TOOL => crate::vitals::mcp::view_metrics(core, &args),
        // RB-050-05: sitemap capture.
        sitemap if crate::sitemap::app::TOOLS.contains(&sitemap) => {
            crate::sitemap::app::host_tool(core, sitemap, args).await
        }
        // RB-080-03: the environment comparison.
        compare if crate::compare::app::TOOLS.contains(&compare) => {
            crate::compare::app::host_tool(core, compare, args).await
        }
        // RB-094-03: the engine comparison.
        engines if crate::engine_compare::app::TOOLS.contains(&engines) => {
            crate::engine_compare::app::host_tool(core, engines, args).await
        }
        // RB-080-15: the status bar's messages.
        crate::status::STATUS_MESSAGES_TOOL => crate::status::mcp::status_messages(core, &args),
        // RB-080-32: the installed Firefox editions; a read, it starts nothing.
        crate::engines::firefox::installed::LIST_TOOL => {
            crate::engines::firefox::installed::host_tool(core, &args)
        }
        // RB-095-02: Android devices over ADB; neither tool downloads adb.
        crate::engines::android::commands::ADD_TOOL => {
            crate::engines::android::commands::add_tool(core, args).await
        }
        crate::engines::android::commands::DEVICES_TOOL => {
            crate::engines::android::commands::devices_tool(core).await
        }
        // RB-095-04: the Mac's iOS simulators and devices; a read, it boots nothing.
        crate::engines::ios::commands::DEVICES_TOOL => {
            crate::engines::ios::commands::devices_tool(core).await
        }
        // RB-080-02: video recording; it never downloads ffmpeg.
        crate::video::app::TOOL_NAME => crate::video::defs::host_tool(core, args).await,
        // RB-110-12: the recordings of the Videos panel; a read, it deletes nothing.
        crate::video::panel::LIST_TOOL => crate::video::panel::host_tool(core, &args),
        // RB-080-06: the extensions' state; it installs nothing.
        crate::extensions::defs::TOOL_NAME => crate::extensions::defs::host_tool(core, args).await,
        // RB-080-23: a Web Store install, only after the person's word in the window.
        crate::extensions::web::INSTALL_TOOL => crate::extensions::web::host_tool(core, args).await,
        // RB-080-30: a Web Store removal, only after the person's word in the window.
        crate::extensions::web::REMOVE_TOOL => {
            crate::extensions::remove::host_tool(core, args).await
        }
        // RB-080-37: the HTTP debugger; edits and sends behind their gates, answers masked.
        network if super::network_tools::TOOLS.contains(&network) => {
            super::network_tools::host_tool(core, network, args).await
        }
        // RB-080-48: the bookmarks, each with `relative` and the URL it leads to now; a read.
        crate::bookmarks::mcp::LIST_TOOL => crate::bookmarks::mcp::bookmarks_list(core, &args),
        // RB-080-05: the portal; no answer carries a pairing token or a device session.
        portal if crate::portal::mcp::TOOLS.contains(&portal) => {
            crate::portal::mcp::host_tool(core, portal, &args)
        }
        // RB-090-14: the self-update; `update_install` asks the person and installs nothing.
        update if crate::updates::mcp::is_tool(update) => {
            crate::updates::mcp::call(core, update).await
        }
        // A loaded plugin's tool (RB-060-12).
        other => match crate::plugins::mcp::call(core, other, args).await {
            Some(result) => result,
            None => Err(ToolFailure::new(
                "unknown_tool",
                format!("the host runs no tool `{other}`"),
            )),
        },
    }
}

/// `screenshot`: the arguments checked as the commands check them, without the delay cycle and
/// without picking (nobody clicks for MCP), captured and waited for.
async fn screenshot(core: &Arc<Core>, args: Value) -> Result<Value, ToolFailure> {
    let args: ScreenshotArgs = parse(args)?;
    let padding = capture::settings(core).element_padding;
    let request = args
        .request(padding, Duration::ZERO, false)
        .map_err(|message| ToolFailure::new("invalid_arguments", message))?;
    let core = core.clone();
    let runtime = core.host().runtime().clone();
    let answer = runtime
        .spawn(async move { capture::screenshot(&core, &request).await })
        .await
        .map_err(|_| unfinished())?
        .map_err(|error| ToolFailure::new("capture_failed", error.to_string()))?;
    json(&answer)
}

/// A visual run on the host's runtime, waited for.
async fn visual<T: Serialize + Send + 'static>(
    core: &Arc<Core>,
    run: impl Future<Output = Result<T, VisualRunError>> + Send + 'static,
) -> Result<Value, ToolFailure> {
    let answer = core
        .host()
        .runtime()
        .spawn(run)
        .await
        .map_err(|_| unfinished())?
        .map_err(|error| visual_failure(&error))?;
    json(&answer)
}

/// A visual run's error as a tool failure with a stable code.
fn visual_failure(error: &VisualRunError) -> ToolFailure {
    let code = match error {
        VisualRunError::NoProject => "visual.no_project",
        VisualRunError::NoViews => "view_not_found",
        VisualRunError::Busy => "visual.busy",
        _ => "visual.failed",
    };
    ToolFailure::new(code, error.to_string())
}

/// Runs `work` on the host's runtime and waits for it.
async fn on_host<T: Send + 'static>(
    core: &Arc<Core>,
    work: impl Future<Output = Result<T, CommandError>> + Send + 'static,
) -> Result<T, ToolFailure> {
    core.host()
        .runtime()
        .spawn(work)
        .await
        .map_err(|_| unfinished())?
        .map_err(ToolFailure::from)
}

fn parse<T: serde::de::DeserializeOwned>(args: Value) -> Result<T, ToolFailure> {
    serde_json::from_value(args)
        .map_err(|error| ToolFailure::new("invalid_arguments", error.to_string()))
}

fn json(answer: &impl Serialize) -> Result<Value, ToolFailure> {
    serde_json::to_value(answer).map_err(|error| ToolFailure::new("internal", error.to_string()))
}

fn unfinished() -> ToolFailure {
    ToolFailure::new("internal", "the tool did not finish")
}

/// What `views_list` answers: every view with its desync (`null` while in sync),
/// `syncNavigation`, the `range` of widths it stands for (RB-080-44) and the frameworks its
/// page runs (`frameworks`, RB-080-31: `vue` and `react` as the probe reported them while their
/// DevTools are not off, else `{}`).
pub fn views_list(core: &Core) -> Value {
    let host = core.host();
    let frameworks: HashMap<ViewId, Value> = host
        .framework_hooks()
        .frameworks()
        .into_iter()
        .map(|entry| {
            let view = entry.view;
            let mut value = serde_json::to_value(entry).unwrap_or(Value::Null);
            if let Value::Object(fields) = &mut value {
                fields.remove("view");
            }
            (view, value)
        })
        .collect();
    let views = host
        .views()
        .into_iter()
        .map(|info| {
            let view = info.id;
            let mut entry = serde_json::to_value(&info).unwrap_or(Value::Null);
            if let Value::Object(fields) = &mut entry {
                fields.insert(
                    "frameworks".to_owned(),
                    frameworks.get(&view).cloned().unwrap_or_else(|| json!({})),
                );
                let desync = crate::sync_hub::mcp::view_desync(host, view)
                    .and_then(|desync| serde_json::to_value(desync).ok())
                    .unwrap_or(Value::Null);
                fields.insert("desync".to_owned(), desync);
                let navigation = crate::nav_sync::mcp::view_navigation(host, view);
                fields.insert(
                    "syncNavigation".to_owned(),
                    Value::Bool(navigation.sync_navigation),
                );
                // In a window of its own (RB-080-14).
                fields.insert("detached".to_owned(), Value::Bool(host.is_detached(view)));
                // The widths the view stands for among the config's breakpoints (RB-080-44).
                let range = host
                    .footer_range(view)
                    .and_then(|range| serde_json::to_value(range).ok())
                    .unwrap_or(Value::Null);
                fields.insert("range".to_owned(), range);
                if let Some(Value::Object(engine)) =
                    crate::engines::mcp::view_engine_state(host, view)
                        .and_then(|state| serde_json::to_value(state).ok())
                {
                    fields.extend(engine);
                }
            }
            entry
        })
        .collect();
    Value::Array(views)
}

#[cfg(test)]
#[path = "host_tools_tests.rs"]
mod tests;
