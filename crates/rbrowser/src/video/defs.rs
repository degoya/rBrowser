//! The commands of video recording for the one registry (RB-080-02): `capture.video.toggle`
//! (`Leader C V`, the active view) and `capture.video.toggle_all` (`Leader C W`, every view)
//! start a recording or stop the running one; `tools.ffmpeg.install` (`Leader X F`) downloads
//! the pinned LGPL ffmpeg - the person's consent, so no MCP tool reaches it - and
//! `tools.ffmpeg.remove` (`Leader X G`) deletes it again; `capture.video.panel` (`Leader C T`)
//! shows the Videos panel (RB-110-12). Every command answers at once; the work runs on the
//! host's runtime or a blocking thread and ends in a notification.

use std::sync::{Arc, Weak};

use rb_commands::{
    Binding, Chord, CommandDef, CommandError, CommandId, CommandRegistry, Group, Handler, Key,
    McpCoverage,
};
use rb_engine::Target;
use rb_ipc::NotificationLevel;
use serde_json::{Value, json};

use super::app::{self, FAILED_KEY, StartRequest, TOOL_NAME};
use super::ffmpeg::{self, NOT_INSTALLED_KEY, REMOVED_KEY, notice};
use super::{VideoError, app::Finished};
use crate::control::Core;

/// Record the active view, or stop.
pub const TOGGLE: &str = "capture.video.toggle";
/// Record every view, or stop.
pub const TOGGLE_ALL: &str = "capture.video.toggle_all";
/// Download the pinned ffmpeg.
pub const INSTALL: &str = "tools.ffmpeg.install";
/// Delete the downloaded ffmpeg.
pub const REMOVE: &str = "tools.ffmpeg.remove";
/// Show the Videos panel (RB-110-12).
pub const PANEL: &str = "capture.video.panel";
/// Why the panel command has no MCP tool (`video_list` lists the recordings).
pub const PANEL_OMITTED: &str = "ui-only: shows the Videos panel; video_list lists the recordings";
/// Why the install has no MCP tool.
pub const INSTALL_OMITTED: &str =
    "protected: downloads a program; only the person, after the consent notice";
/// Why the removal has no MCP tool.
pub const REMOVE_OMITTED: &str = "protected: deletes a program the person downloaded";

/// One row: id, group, the keys after the leader, the MCP tool (`Ok`) or why there is none.
type Row = (
    &'static str,
    Group,
    [char; 2],
    Result<&'static str, &'static str>,
);

/// The commands.
const ROWS: [Row; 5] = [
    (TOGGLE, Group::Capture, ['C', 'V'], Ok(TOOL_NAME)),
    (TOGGLE_ALL, Group::Capture, ['C', 'W'], Ok(TOOL_NAME)),
    (INSTALL, Group::Plugins, ['X', 'F'], Err(INSTALL_OMITTED)),
    (REMOVE, Group::Plugins, ['X', 'G'], Err(REMOVE_OMITTED)),
    (PANEL, Group::Capture, ['C', 'T'], Err(PANEL_OMITTED)),
];

/// The definitions; title key `commands.<id>`.
pub fn definitions() -> Result<Vec<CommandDef>, CommandError> {
    ROWS.iter()
        .map(|(id, group, keys, mcp)| {
            Ok(CommandDef {
                id: CommandId::new(id)?,
                title_key: format!("commands.{id}"),
                group: *group,
                default_bindings: vec![Binding::Leader(Chord {
                    keys: keys.iter().map(|key| Key::Char(*key)).collect(),
                })],
                mcp: match mcp {
                    Ok(tool) => McpCoverage::Covered((*tool).to_owned()),
                    Err(reason) => McpCoverage::Omitted((*reason).to_owned()),
                },
            })
        })
        .collect()
}

/// Registers the commands with `registry`; returns what the registry refused (a test holds it
/// empty).
pub fn register(registry: &mut CommandRegistry, core: &Arc<Core>) -> Vec<CommandError> {
    let defs = match definitions() {
        Ok(defs) => defs,
        Err(error) => return vec![error],
    };
    defs.into_iter()
        .filter_map(|def| {
            let handler = handler(def.id.as_str(), Arc::downgrade(core));
            registry.register(def, handler).err()
        })
        .collect()
}

fn handler(id: &str, core: Weak<Core>) -> Handler {
    let id = id.to_owned();
    Arc::new(move |args: Value| {
        let core = core
            .upgrade()
            .ok_or_else(|| CommandError::Failed("the window is closed".to_owned()))?;
        run(&core, &id, &args)
    })
}

/// The `target` of `capture.video.toggle`'s arguments: `"all"` or a view id (the context
/// menu's clicked view, RB-080-43); `None`, the active view, without one.
fn target_of(args: &Value) -> Result<Option<Target>, CommandError> {
    match args.get("target") {
        None | Some(Value::Null) => Ok(None),
        Some(target) => serde_json::from_value(target.clone())
            .map(Some)
            .map_err(|error| CommandError::InvalidArgs(error.to_string())),
    }
}

/// Runs the command `id`: answers at once, the work goes on in the background.
fn run(core: &Arc<Core>, id: &str, args: &Value) -> Result<Value, CommandError> {
    match id {
        TOGGLE => Ok(toggle(core, target_of(args)?)),
        TOGGLE_ALL => Ok(toggle(core, Some(Target::All))),
        PANEL => {
            crate::panel_dock::core_panels::open(core, crate::panel_dock::core_panels::VIDEOS);
            Ok(Value::Null)
        }
        INSTALL => {
            let notifier = core.clone();
            core.host().runtime().spawn_blocking(move || {
                let _ = ffmpeg::install_default(&|message| notifier.notify(message));
            });
            Ok(json!({ "started": true }))
        }
        REMOVE => {
            if app::is_recording() {
                return Err(CommandError::Failed(VideoError::Busy.to_string()));
            }
            let notifier = core.clone();
            core.host().runtime().spawn_blocking(move || {
                let message = match ffmpeg::remove_default() {
                    Ok(Some(folder)) => notice(
                        NotificationLevel::Info,
                        REMOVED_KEY,
                        &[("folder", folder.display().to_string())],
                    ),
                    Ok(None) => notice(NotificationLevel::Info, NOT_INSTALLED_KEY, &[]),
                    Err(error) => notice(
                        NotificationLevel::Error,
                        FAILED_KEY,
                        &[("error", error.to_string())],
                    ),
                };
                notifier.notify(message);
            });
            Ok(json!({ "started": true }))
        }
        other => Err(CommandError::Failed(format!("no video command {other}"))),
    }
}

/// Stops the running recording, or starts one of `target` (the active view when `None`).
fn toggle(core: &Arc<Core>, target: Option<Target>) -> Value {
    let recording = app::is_recording();
    let runner = core.clone();
    core.host().runtime().spawn(async move {
        let result = if recording {
            app::stop().await.map(|_| ())
        } else {
            let request = StartRequest {
                target,
                views: None,
            };
            app::start(&runner, &request).await.map(|_| ())
        };
        // A missing ffmpeg has shown the consent notice already.
        if let Err(error) = result
            && !matches!(error, VideoError::FfmpegMissing)
        {
            tracing::warn!(%error, "the video command failed");
            runner.notify(notice(
                NotificationLevel::Error,
                FAILED_KEY,
                &[("error", error.to_string())],
            ));
        }
    });
    if recording {
        json!({ "stopping": true })
    } else {
        json!({ "starting": true })
    }
}

/// The MCP tool `record_video`: `action` `start` (with `target` or `views`), `stop` (answers
/// the files) or `status`. Never downloads ffmpeg.
///
/// # Errors
///
/// Arguments that do not read, or the recording's refusal with its code.
pub async fn host_tool(core: &Arc<Core>, args: Value) -> Result<Value, rb_mcp::ToolFailure> {
    let mut args = match args {
        Value::Null => serde_json::Map::new(),
        Value::Object(map) => map,
        _ => return Err(invalid("the arguments are an object".to_owned())),
    };
    let action = args
        .remove("action")
        .and_then(|action| action.as_str().map(str::to_owned))
        .ok_or_else(|| invalid("action is start, stop or status".to_owned()))?;
    let failure = |error: VideoError| rb_mcp::ToolFailure::new(error.code(), error.to_string());
    match action.as_str() {
        "start" => {
            let request: StartRequest = serde_json::from_value(Value::Object(args))
                .map_err(|error| invalid(error.to_string()))?;
            let run = core.clone();
            let started = core
                .host()
                .runtime()
                .spawn(async move { app::start(&run, &request).await })
                .await
                .map_err(|error| rb_mcp::ToolFailure::new("internal", error.to_string()))?
                .map_err(failure)?;
            answer(&started)
        }
        "stop" => {
            let finished: Arc<Finished> = core
                .host()
                .runtime()
                .spawn(app::stop())
                .await
                .map_err(|error| rb_mcp::ToolFailure::new("internal", error.to_string()))?
                .map_err(failure)?;
            answer(finished.as_ref())
        }
        "status" => answer(&app::status(core)),
        other => Err(invalid(format!(
            "action {other} is not start, stop or status"
        ))),
    }
}

fn invalid(message: String) -> rb_mcp::ToolFailure {
    rb_mcp::ToolFailure::new("invalid_arguments", message)
}

fn answer(value: &impl serde::Serialize) -> Result<Value, rb_mcp::ToolFailure> {
    serde_json::to_value(value)
        .map_err(|error| rb_mcp::ToolFailure::new("internal", error.to_string()))
}

#[cfg(test)]
#[path = "defs_tests.rs"]
mod tests;
