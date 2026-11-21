//! The recording editor's commands and MCP tools (RB-092-01). The Recorder panel edits its
//! draft in the shell and saves it whole (`saveRecording`); its shortcuts and every agent reach
//! the same functions here:
//!
//! | Command | Keys | MCP | Arguments |
//! | --- | --- | --- | --- |
//! | `recorder.step_add` | `Leader F N` | `recorder_step_add` | `recordingId, action, after?, name?, onlyIn?, pauseMs?, disabled?, view?, x?, y?` |
//! | - | - | `recorder_step_update` | `recordingId, stepId, patch` |
//! | - | - | `recorder_step_remove` | `recordingId, stepId` |
//! | `recorder.step_toggle` | `Leader F B` | `recorder_step_toggle` | `recordingId, stepId, disabled?` |
//! | `recorder.step_try` | `Leader F J` | `recorder_step_try` | `recordingId, stepId, view?, draft?` |
//! | `recorder.play_from` | `Leader F H` | `recorder_play` (`fromStep`) | `recordingId, stepId, views?, pace?, draft?` |
//!
//! A command without `recordingId` - a shortcut - goes to the Recorder panel ([`forward`]),
//! which runs it on the step in focus. The edits write the stored recording over its file,
//! checked as a save is ([`rb_sync::recorder::edit::check`], `recording.v1`); an open panel
//! with unsaved edits keeps its draft. No tool deletes a recording (the owner's decision).
//! `recorder_step_add` with `x`/`y` names the element at that point of `view` (the active one)
//! as a recorded click names it ([`super::pick_locator`]) and puts it into the action. An added
//! step's pause is `automation.recorder.insertedStepPauseMs` unless `pauseMs` says otherwise.
//! A secret field's fill is the recorder's own: an agent's add or update neither adds one nor
//! changes one ([`SECRET_CODE`]), and every edit holds the store's write lock.

use rb_commands::CommandError;
use rb_engine::{ViewHost as _, ViewId};
use rb_ipc::CoreToShell;
use rb_mcp::ToolFailure;
use rb_sync::recorder::{Action, Place, Recording, StepPatch, edit};
use serde::Deserialize;
use serde_json::{Value, json};

use super::glue::store_of;
use crate::control::Core;

/// `Leader F N`: add a step.
pub const STEP_ADD: &str = "recorder.step_add";
/// `Leader F B`: switch a step off or on.
pub const STEP_TOGGLE: &str = "recorder.step_toggle";
/// `Leader F J`: try one step in the active view.
pub const STEP_TRY: &str = "recorder.step_try";
/// `Leader F H`: play from a step on.
pub const PLAY_FROM: &str = "recorder.play_from";
/// The MCP code of an agent's edit that adds or changes a secret field's fill (RB-092-01).
pub const SECRET_CODE: &str = "recorder.secret_step";
/// The MCP tools the host runs here.
pub const TOOLS: [&str; 5] = [
    "recorder_step_add",
    "recorder_step_update",
    "recorder_step_remove",
    "recorder_step_toggle",
    "recorder_step_try",
];

/// Why a step edit was refused: a command's refusal, or a secret fill an agent may not add or
/// change ([`edit::check_secret`], its own MCP code [`SECRET_CODE`]).
#[derive(Debug, thiserror::Error)]
pub enum StepError {
    #[error(transparent)]
    Command(#[from] CommandError),
    #[error("{0}")]
    Secret(String),
}

impl From<StepError> for ToolFailure {
    fn from(error: StepError) -> Self {
        match error {
            StepError::Command(error) => error.into(),
            StepError::Secret(message) => ToolFailure::new(SECRET_CODE, message),
        }
    }
}

impl From<StepError> for CommandError {
    fn from(error: StepError) -> Self {
        match error {
            StepError::Command(error) => error,
            StepError::Secret(message) => CommandError::InvalidArgs(message),
        }
    }
}

/// An edit's refusal: a secret fill's is a [`StepError::Secret`].
fn refused(error: edit::EditError) -> StepError {
    match error {
        edit::EditError::Secret(_) => StepError::Secret(error.to_string()),
        other => StepError::Command(invalid(other)),
    }
}

/// Sends `command` to the Recorder panel when `args` name no recording (a shortcut's call):
/// the panel opens and runs it on the step in focus. `true` when it went there.
pub fn forward(core: &Core, command: &str, args: &Value) -> bool {
    if args.get("recordingId").is_some() {
        return false;
    }
    crate::panel_dock::core_panels::open_unfolded(core, crate::panel_dock::core_panels::RECORDER);
    core.send(&CoreToShell::RecorderStepCommand {
        command: command.to_owned(),
    });
    true
}

/// `recorder_step_add`'s and `recorder.step_add`'s arguments.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AddArgs {
    pub recording_id: String,
    /// The step's action; its element from `x`/`y` when they are given.
    pub action: Value,
    /// After this step; `0` before the first; the end when absent.
    #[serde(default)]
    pub after: Option<u32>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub only_in: Option<Vec<String>>,
    #[serde(default)]
    pub pause_ms: Option<u32>,
    #[serde(default)]
    pub disabled: Option<bool>,
    #[serde(default)]
    pub view: Option<u32>,
    #[serde(default)]
    pub x: Option<f64>,
    #[serde(default)]
    pub y: Option<f64>,
}

/// One step of a recording.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StepArgs {
    recording_id: String,
    step_id: u32,
    #[serde(default)]
    disabled: Option<bool>,
    #[serde(default)]
    patch: Option<StepPatch>,
    #[serde(default)]
    view: Option<u32>,
    #[serde(default)]
    views: Option<Value>,
    #[serde(default)]
    pace: Option<Value>,
    #[serde(default)]
    draft: Option<Value>,
}

/// Runs the editor's command `id` when it is one; `None` otherwise.
pub fn run_command(core: &Core, id: &str, args: &Value) -> Option<Result<Value, CommandError>> {
    if ![STEP_ADD, STEP_TOGGLE, STEP_TRY, PLAY_FROM].contains(&id) {
        return None;
    }
    if forward(core, id, args) {
        return Some(Ok(json!({ "forwarded": true })));
    }
    Some(match id {
        STEP_ADD => add(core, args).map_err(CommandError::from),
        STEP_TOGGLE => toggle(core, args).map_err(CommandError::from),
        STEP_TRY => try_args(core, args).and_then(|args| super::play::run(core, &args)),
        _ => play_from_args(args).and_then(|args| super::play::run(core, &args)),
    })
}

/// The MCP tool `name` (one of [`TOOLS`]); its file work off the async workers.
///
/// # Errors
///
/// The tool's refusal: bad arguments, no such recording or step, a check that fails.
pub async fn host_tool(
    core: &std::sync::Arc<Core>,
    name: &str,
    args: Value,
) -> Result<Value, ToolFailure> {
    if name == "recorder_step_try" {
        let args = try_args(core, &args).map_err(ToolFailure::from)?;
        return super::play::tool(core, args).await;
    }
    let blocking = core.clone();
    let name = name.to_owned();
    tokio::task::spawn_blocking(move || match name.as_str() {
        "recorder_step_add" => add(&blocking, &args),
        "recorder_step_update" => update(&blocking, &args),
        "recorder_step_remove" => remove(&blocking, &args),
        _ => toggle(&blocking, &args),
    })
    .await
    .map_err(|_| ToolFailure::new("internal", "the tool did not finish"))?
    .map_err(ToolFailure::from)
}

fn parse<T: serde::de::DeserializeOwned>(args: &Value) -> Result<T, CommandError> {
    serde_json::from_value(args.clone())
        .map_err(|error| CommandError::InvalidArgs(error.to_string()))
}

fn invalid(error: impl std::fmt::Display) -> CommandError {
    CommandError::InvalidArgs(error.to_string())
}

/// Reads the recording `name`, changes it with `change`, checks it and writes it over its file;
/// an open panel asks for the list again.
fn edit_file(
    core: &Core,
    name: &str,
    change: impl FnOnce(&mut Recording) -> Result<Value, StepError>,
) -> Result<Value, StepError> {
    let store = store_of(core.host()).map_err(|error| CommandError::Failed(error.to_string()))?;
    // No other write of the file comes between the read and the write (RB-092-01).
    let _written = rb_sync::recorder::write_lock();
    let mut recording = store.load(name).map_err(super::export::store_failed)?;
    let answer = change(&mut recording)?;
    edit::check(&recording).map_err(invalid)?;
    store
        .replace(&recording)
        .map_err(|error| CommandError::Failed(error.to_string()))?;
    core.send(&CoreToShell::RecordingsChanged);
    Ok(answer)
}

fn add(core: &Core, raw: &Value) -> Result<Value, StepError> {
    let args: AddArgs = parse(raw)?;
    let mut action = args.action.clone();
    let mut only_in = args.only_in.clone();
    if let Some(point) = super::pick_locator::point_of(args.x, args.y, None)? {
        let picked = super::pick_locator::pick_element(core, args.view.map(ViewId), Some(point))?;
        let all = super::pick_locator::breakpoints_of(core);
        let locator = serde_json::to_value(rb_sync::recorder::Locator::from_target(
            &picked.target,
            None,
        ))
        .map_err(invalid)?;
        with_target(&mut action, locator)?;
        if only_in.is_none() {
            only_in = Some(super::pick_locator::only_in(&picked.views, &all));
        }
    }
    let action: Action = serde_json::from_value(action).map_err(invalid)?;
    // An agent adds no secret fill (RB-092-01 security review).
    edit::check_secret(0, None, &action).map_err(refused)?;
    let pause_ms = args.pause_ms.or_else(|| {
        let ms = core
            .host()
            .config()
            .automation
            .recorder
            .inserted_step_pause_ms;
        Some(u32::try_from(ms).unwrap_or(u32::MAX))
    });
    let place = Place::from_after(args.after);
    edit_file(core, &args.recording_id, |recording| {
        let mut step = edit::new_step(recording, place, action, pause_ms).map_err(invalid)?;
        step.name = args.name.filter(|name| !name.trim().is_empty());
        step.only_in = only_in.unwrap_or_default();
        step.disabled = args.disabled.unwrap_or(false);
        let id = step.id;
        let index = edit::insert(recording, place, step).map_err(invalid)?;
        Ok(
            json!({ "recordingId": recording.name, "stepId": id, "index": index,
                   "steps": recording.steps.len() }),
        )
    })
}

/// Puts `locator` into `action` where its kind keeps the element: `target`, or an assertion's.
///
/// # Errors
///
/// An action that is no object, or whose kind has no element (`navigate`, `wait`, an URL check).
pub fn with_target(action: &mut Value, locator: Value) -> Result<(), CommandError> {
    let kind = action
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let holder = if kind == "assert" {
        action.get_mut("assertion")
    } else {
        Some(&mut *action)
    };
    let without = matches!(kind.as_str(), "navigate" | "waitForUrl" | "wait")
        || action_has_no_element(holder.as_deref());
    match holder.and_then(Value::as_object_mut) {
        Some(object) if !without => {
            object.insert("target".to_owned(), locator);
            Ok(())
        }
        _ => Err(CommandError::InvalidArgs(format!(
            "a `{kind}` step has no element to take from `x`/`y`"
        ))),
    }
}

fn action_has_no_element(holder: Option<&Value>) -> bool {
    holder
        .and_then(|holder| holder.get("kind"))
        .and_then(Value::as_str)
        == Some("url")
}

fn update(core: &Core, raw: &Value) -> Result<Value, StepError> {
    let args: StepArgs = parse(raw)?;
    let patch = args
        .patch
        .ok_or_else(|| invalid("`patch` names what changes"))?;
    edit_file(core, &args.recording_id, |recording| {
        if let Some(action) = &patch.action {
            // An agent changes no secret fill and makes none (RB-092-01 security review).
            let before = recording
                .steps
                .iter()
                .find(|step| step.id == args.step_id)
                .map(|step| &step.action);
            edit::check_secret(args.step_id, before, action).map_err(refused)?;
        }
        edit::apply(recording, args.step_id, &patch).map_err(invalid)?;
        Ok(json!({ "recordingId": recording.name, "stepId": args.step_id }))
    })
}

fn remove(core: &Core, raw: &Value) -> Result<Value, StepError> {
    let args: StepArgs = parse(raw)?;
    edit_file(core, &args.recording_id, |recording| {
        edit::remove(recording, args.step_id).map_err(invalid)?;
        Ok(
            json!({ "recordingId": recording.name, "stepId": args.step_id,
                   "steps": recording.steps.len() }),
        )
    })
}

fn toggle(core: &Core, raw: &Value) -> Result<Value, StepError> {
    let args: StepArgs = parse(raw)?;
    edit_file(core, &args.recording_id, |recording| {
        let now = recording
            .steps
            .iter()
            .find(|step| step.id == args.step_id)
            .map(|step| step.disabled)
            .ok_or_else(|| invalid(edit::EditError::NoStep(args.step_id)))?;
        let disabled = args.disabled.unwrap_or(!now);
        edit::set_disabled(recording, args.step_id, disabled).map_err(invalid)?;
        Ok(
            json!({ "recordingId": recording.name, "stepId": args.step_id,
                   "disabled": disabled }),
        )
    })
}

/// "Try": `recorder.play`'s arguments for step `stepId` alone in `view` (the active one), at
/// the pace `fast`, from the draft when one comes.
///
/// # Errors
///
/// Bad arguments, no view.
pub fn try_args(core: &Core, raw: &Value) -> Result<Value, CommandError> {
    let args: StepArgs = parse(raw)?;
    let view = args
        .view
        .map(ViewId)
        .or_else(|| core.host().active_view())
        .ok_or_else(|| CommandError::Failed("there is no view".to_owned()))?;
    let mut play = json!({ "recordingId": args.recording_id, "onlyStep": args.step_id,
                           "views": [view.0], "pace": "fast" });
    if let Some(draft) = args.draft {
        play["draft"] = draft;
    }
    Ok(play)
}

/// "Play from here": `recorder.play`'s arguments from step `stepId` on.
///
/// # Errors
///
/// Bad arguments.
pub fn play_from_args(raw: &Value) -> Result<Value, CommandError> {
    let args: StepArgs = parse(raw)?;
    let mut play = json!({ "recordingId": args.recording_id, "fromStep": args.step_id });
    for (key, value) in [
        ("views", args.views),
        ("pace", args.pace),
        ("draft", args.draft),
    ] {
        if let Some(value) = value {
            play[key] = value;
        }
    }
    Ok(play)
}

#[cfg(test)]
#[path = "steps_tests.rs"]
mod tests;
