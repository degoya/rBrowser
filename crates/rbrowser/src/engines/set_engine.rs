//! `view.set_engine` (RB-080-33, `Leader B E`, MCP `views_set_engine`): the engine of one view -
//! Chromium, Firefox or WebKit - chosen from the engine badge at the right end of its label
//! strip, the canvas board's badge, the command palette, the chord or MCP. Without an `engine`
//! the engine menu opens for the view ([`crate::breakpoint_keys::engine_pick`]).
//!
//! The engine of a view is the engine of its breakpoint, so the choice is written as
//! `breakpoints[<id>].engine` into `config.local.json` (`file: "local"`, the default: personal,
//! not checked in) or `config.json` (`file: "project"`, shared with the team), the config is
//! read again and the view follows - Firefox and WebKit as external windows, as a breakpoint
//! set to them in the config does. A project without `layout.cells` gets the computed grid
//! written beside it, so the view keeps its cell (the computed layout holds Chromium views
//! only). An engine that cannot run (Firefox not found, WebKit not installed) is refused with
//! the reason; the view stays. The user setting `ui.engineBadge` shows or hides the badge.
//! With `engine: "ios"` an `iosDevice` (RB-095-04) is written too - always into
//! `config.local.json`, as a device (a UDID above all) is the person's own, also when the engine
//! goes into `config.json` (RB-095-05); a view on iOS already starts again on the new device.

use std::sync::{Arc, Weak};

use rb_commands::{
    Binding, Chord, CommandDef, CommandError, CommandId, CommandRegistry, Group, Handler, Key,
    McpCoverage,
};
use rb_config::{ConfigFile, EngineKind, Layout, PresetFile, ProjectConfig};
use rb_engine::{ViewHost as _, ViewId, ViewInfo};
use rb_ipc::{CoreToShell, Notification, NotificationLevel};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::breakpoint_keys::engine_pick::{config_name, unavailable_reason};
use crate::control::Core;

/// The command.
pub const SET_ENGINE: &str = "view.set_engine";
/// The MCP tool that covers it.
pub const MCP_TOOL: &str = "views_set_engine";
/// The i18n key of the notice after the write; `{breakpoint}`, `{engine}`, `{file}`.
pub const WRITTEN_KEY: &str = "engines.badge.written";
/// The i18n key of the warning when the view did not follow the write: an entry of the same
/// breakpoint in `config.local.json` shadows `config.json`; the same arguments.
pub const SHADOWED_KEY: &str = "engines.badge.shadowed";

/// The arguments of `view.set_engine`.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Args {
    /// The view; the active view when absent.
    #[serde(default, alias = "viewId")]
    pub view: Option<ViewId>,
    /// Several views at once (MCP `target: "all"`): one write, one reload; answers a list.
    #[serde(default)]
    pub views: Vec<ViewId>,
    /// The engine; absent opens the engine menu for the view.
    #[serde(default)]
    pub engine: Option<EngineKind>,
    /// `local` (the default) or `project`: the config file the engine is written to.
    #[serde(default)]
    pub file: Option<PresetFile>,
    /// With `engine: "ios"`: the model, simulator or device (`breakpoints[].iosDevice`,
    /// RB-095-04).
    #[serde(default)]
    pub ios_device: Option<String>,
}

/// Runs the command `id` when it is `view.set_engine`; `None` otherwise.
pub fn run_command(core: &Core, id: &str, args: &Value) -> Option<Result<Value, CommandError>> {
    (id == SET_ENGINE).then(|| run(core, args))
}

/// `view.set_engine`: answers `{ view, breakpoint, engine, file, changed }` - a list of them
/// for `views`, an entry `{ view, failed }` for a view that has no breakpoint of the config -
/// or `null` when the engine menu was opened instead.
pub fn run(core: &Core, args: &Value) -> Result<Value, CommandError> {
    let args: Args = if args.is_null() {
        Args::default()
    } else {
        serde_json::from_value(args.clone())
            .map_err(|error| CommandError::InvalidArgs(error.to_string()))?
    };
    let host = core.host();
    let device = args.ios_device.as_deref();
    if let Some(device) = device
        && (args.engine != Some(EngineKind::Ios) || !crate::engines::ios::valid_device(device))
    {
        return Err(CommandError::InvalidArgs(
            "iosDevice goes with engine ios: a model or a UDID of at most 80 characters".to_owned(),
        ));
    }
    if !args.views.is_empty() {
        let engine = args
            .engine
            .ok_or_else(|| CommandError::InvalidArgs("`views` needs an `engine`".to_owned()))?;
        return Ok(Value::Array(run_many_with(
            core,
            &args.views,
            engine,
            args.file.unwrap_or_default(),
            device,
        )?));
    }
    // The menus' engine pick (no engine) takes the first view while none is active, as the other
    // view commands do (RB-090-82); a switch to an engine names its view or the active one.
    let view = match (args.view, args.engine) {
        (Some(view), _) => Some(view),
        (None, None) => host.active_or_first().ok(),
        (None, Some(_)) => host.active_view(),
    };
    let view = view.ok_or_else(|| CommandError::InvalidArgs("no view is active".to_owned()))?;
    if host.view(view).is_none() {
        return Err(CommandError::InvalidArgs(format!("unknown view {view}")));
    }
    let Some(engine) = args.engine else {
        // The canvas board draws its own strips: its menu hangs from the board's label.
        if crate::layout::canvas_board::engine_pick(core, view).is_err() {
            core.ui().show_engine_pick(view, None);
        }
        return Ok(Value::Null);
    };
    let mut answers = run_many_with(core, &[view], engine, args.file.unwrap_or_default(), device)?;
    let answer = answers.pop().unwrap_or(Value::Null);
    match answer.get("failed").and_then(Value::as_str) {
        Some(reason) => Err(CommandError::InvalidArgs(reason.to_owned())),
        None => Ok(answer),
    }
}

/// A view with its breakpoint of the config, or why it has none.
type Planned = (ViewId, Result<(ViewInfo, String), String>);

/// Sets `engine` for `views` with one write and one reload; one answer per view, in order.
///
/// # Errors
///
/// The engine cannot run, or the write or the reload failed.
pub fn run_many(
    core: &Core,
    views: &[ViewId],
    engine: EngineKind,
    file: PresetFile,
) -> Result<Vec<Value>, CommandError> {
    run_many_with(core, views, engine, file, None)
}

/// [`run_many`] with the `iosDevice` written beside `engine: "ios"` (RB-095-04): it writes even
/// when the view is on iOS already, and such a view starts again on the new device.
///
/// # Errors
///
/// The engine cannot run, or the write or the reload failed.
pub fn run_many_with(
    core: &Core,
    views: &[ViewId],
    engine: EngineKind,
    file: PresetFile,
    ios_device: Option<&str>,
) -> Result<Vec<Value>, CommandError> {
    if engine == EngineKind::Android {
        return Err(CommandError::InvalidArgs(
            "an Android device is not a breakpoint's engine: add it with view.add_android"
                .to_owned(),
        ));
    }
    let host = core.host();
    let config = host.config();
    // What each view needs: its breakpoint, or why it has none.
    let mut plan: Vec<Planned> = Vec::new();
    for &view in views {
        let Some(info) = host.view(view) else {
            plan.push((view, Err(format!("unknown view {view}"))));
            continue;
        };
        match breakpoint_for(&config, &info) {
            Some(breakpoint) => plan.push((view, Ok((info, breakpoint)))),
            None => plan.push((
                view,
                Err(format!(
                    "view {view} shows no breakpoint of the config; set it to one first"
                )),
            )),
        }
    }
    let to_write: Vec<&String> = plan
        .iter()
        .filter_map(|(_, entry)| entry.as_ref().ok())
        .filter(|(info, _)| info.engine != engine || ios_device.is_some())
        .map(|(_, breakpoint)| breakpoint)
        .collect();
    if !to_write.is_empty() {
        if let Some(reason) = unavailable_reason(core, engine) {
            return Err(CommandError::Failed(reason));
        }
        // The computed layout holds Chromium views only: a project without cells gets them.
        let cells = config
            .layout
            .cells
            .is_empty()
            .then(|| crate::host::state::layout_for(&config));
        let engine_file = file.config_file();
        let local = engine_file == ConfigFile::Local;
        crate::layout::store::write_into(core, Some(engine_file), &[], |document| {
            for breakpoint in &to_write {
                patch_document(document, breakpoint, engine, cells.as_ref());
                if let (true, Some(device)) = (local, ios_device) {
                    patch_ios_device(document, breakpoint, device);
                }
            }
        })?;
        // The device is the person's own: never into the shared file.
        if let (false, Some(device)) = (local, ios_device) {
            crate::layout::store::write_into(core, Some(ConfigFile::Local), &[], |document| {
                for breakpoint in &to_write {
                    patch_ios_device(document, breakpoint, device);
                }
            })?;
        }
        crate::commands::reload_config(host).map_err(CommandError::Failed)?;
        // The read config shows the grid; the preset or mode in effect comes back, as after the
        // watcher's reload.
        crate::presets::on_config_changed(core);
    }
    // The config as read now: what each breakpoint runs on, `config.local.json` merged.
    let read = host.config();
    let mut answers = Vec::with_capacity(plan.len());
    for (view, entry) in plan {
        let (info, breakpoint) = match entry {
            Ok(entry) => entry,
            Err(reason) => {
                answers.push(json!({ "view": view, "failed": reason }));
                continue;
            }
        };
        if info.engine == engine && ios_device.is_none() {
            answers.push(answer(view, &breakpoint, engine, file, false));
            continue;
        }
        if info.engine == engine {
            // On iOS already: the view starts again on the new device.
            restart_ios(core, view);
        }
        // A view set to another breakpoint of the config by the quick pick follows that
        // breakpoint's new definition only when it is applied again.
        if info.cell_breakpoint.as_deref() != Some(breakpoint.as_str())
            && let Err(error) = crate::breakpoint_keys::run(
                core,
                crate::breakpoint_keys::SET,
                &json!({ "view": view, "breakpoint": breakpoint }),
            )
        {
            tracing::warn!(%view, %error, "the view did not take its breakpoint again");
        }
        // `changed` is what the read config gives the breakpoint, not what the old view id
        // shows: a view whose breakpoint has no cell of the grid opens anew in a layout mode
        // (another id). A `config.local.json` entry of the breakpoint shadows a write into
        // `config.json`.
        let changed = effective_engine(&read, &breakpoint) == Some(engine);
        let Some((level, key)) = notice_for(changed, file) else {
            tracing::warn!(
                %view,
                %breakpoint,
                "the engine written to config.local.json was not read back"
            );
            answers.push(with_device(
                answer(view, &breakpoint, engine, file, changed),
                ios_device,
            ));
            continue;
        };
        core.notify(CoreToShell::Notification(Notification {
            level,
            message_key: key.to_owned(),
            args: [
                ("breakpoint".to_owned(), breakpoint.clone()),
                (
                    "engine".to_owned(),
                    rb_i18n::t(&crate::breakpoint_keys::engine_pick::name_key(engine)),
                ),
                ("file".to_owned(), file.config_file().file_name().to_owned()),
            ]
            .into(),
            settings_key: None,
        }));
        answers.push(with_device(
            answer(view, &breakpoint, engine, file, changed),
            ios_device,
        ));
    }
    Ok(answers)
}

/// Restarts the iOS view `view`, which reads its breakpoint's new `iosDevice` (RB-095-04).
fn restart_ios(core: &Core, view: ViewId) {
    // The external windows take the breakpoint as read now, then the view starts on it.
    let restarted = crate::engines::for_host(core.host()).map(|engines| {
        engines.views_changed();
        engines.windows().restart(EngineKind::Ios, Some(view))
    });
    if let Some(Err(error)) = restarted {
        tracing::debug!(%view, %error, "the iOS view did not start again");
    }
}

/// `answer` with the `iosDevice` written, when one was.
fn with_device(mut answer: Value, ios_device: Option<&str>) -> Value {
    if let (Some(device), Some(object)) = (ios_device, answer.as_object_mut()) {
        object.insert("iosDevice".to_owned(), Value::from(device));
    }
    answer
}

/// Writes `device` as the `iosDevice` of `breakpoint` into a config file's JSON `document`;
/// a file without the breakpoint's entry gets one with only `id` and `iosDevice` (the merge
/// joins it with the other file's entry by id).
pub fn patch_ios_device(document: &mut Value, breakpoint: &str, device: &str) {
    if !document.is_object() {
        *document = json!({});
    }
    let Some(object) = document.as_object_mut() else {
        return;
    };
    let breakpoints = object
        .entry("breakpoints".to_owned())
        .or_insert_with(|| json!([]));
    if !breakpoints.is_array() {
        *breakpoints = json!([]);
    }
    let Some(list) = breakpoints.as_array_mut() else {
        return;
    };
    match list
        .iter_mut()
        .find(|item| item.get("id").and_then(Value::as_str) == Some(breakpoint))
    {
        Some(Value::Object(entry)) => {
            entry.insert("iosDevice".to_owned(), Value::from(device));
        }
        Some(_) => {}
        None => list.push(json!({ "id": breakpoint, "iosDevice": device })),
    }
}

/// The breakpoint of the config that carries the engine of a view: the one it shows, else its
/// cell's; `None` when neither is in the config (a device preset in a cell-less view).
pub fn breakpoint_for(config: &ProjectConfig, info: &ViewInfo) -> Option<String> {
    [info.breakpoint.as_deref(), info.cell_breakpoint.as_deref()]
        .into_iter()
        .flatten()
        .find(|id| {
            config
                .breakpoints
                .iter()
                .any(|breakpoint| breakpoint.id == *id)
        })
        .map(str::to_owned)
}

/// The engine `config` gives `breakpoint` (`config.local.json` merged over `config.json`);
/// `None` when it has no such breakpoint.
pub fn effective_engine(config: &ProjectConfig, breakpoint: &str) -> Option<EngineKind> {
    config
        .breakpoints
        .iter()
        .find(|known| known.id == breakpoint)
        .map(|known| known.engine)
}

/// The status bar's notice after a write into `file`: "runs on ... from now on" when the read
/// config `changed` the engine, the warning when it did not - only a `config.local.json` entry
/// can shadow a write into `config.json`; `None` for a local write that did not take (nothing
/// shadows that file, so the warning would be false).
pub fn notice_for(changed: bool, file: PresetFile) -> Option<(NotificationLevel, &'static str)> {
    match (changed, file) {
        (true, _) => Some((NotificationLevel::Info, WRITTEN_KEY)),
        (false, PresetFile::Project) => Some((NotificationLevel::Warning, SHADOWED_KEY)),
        (false, PresetFile::Local) => None,
    }
}

/// Writes `engine` as the engine of `breakpoint` into a config file's JSON `document`: the
/// breakpoint's entry gets `engine` (or is added with only `id` and `engine` - the merge joins
/// it with the project file's entry by id), and `cells` - the computed grid - goes into `layout`
/// when the document has no cells of its own.
pub fn patch_document(
    document: &mut Value,
    breakpoint: &str,
    engine: EngineKind,
    cells: Option<&Layout>,
) {
    if !document.is_object() {
        *document = json!({});
    }
    let engine = Value::from(config_name(engine));
    let Some(object) = document.as_object_mut() else {
        return;
    };
    let breakpoints = object
        .entry("breakpoints".to_owned())
        .or_insert_with(|| json!([]));
    if !breakpoints.is_array() {
        *breakpoints = json!([]);
    }
    if let Some(list) = breakpoints.as_array_mut() {
        match list
            .iter_mut()
            .find(|item| item.get("id").and_then(Value::as_str) == Some(breakpoint))
        {
            Some(Value::Object(entry)) => {
                entry.insert("engine".to_owned(), engine);
            }
            Some(_) => {}
            None => list.push(json!({ "id": breakpoint, "engine": engine })),
        }
    }
    let Some(cells) = cells else {
        return;
    };
    let layout = object
        .entry("layout".to_owned())
        .or_insert_with(|| json!({}));
    if !layout.is_object() {
        *layout = json!({});
    }
    let has_cells = layout
        .get("cells")
        .and_then(Value::as_array)
        .is_some_and(|cells| !cells.is_empty());
    if has_cells {
        return;
    }
    if let (Some(layout), Ok(Value::Object(computed))) =
        (layout.as_object_mut(), serde_json::to_value(cells))
    {
        for key in ["columns", "rows", "cells"] {
            if let Some(value) = computed.get(key) {
                layout.insert(key.to_owned(), value.clone());
            }
        }
    }
}

fn answer(
    view: ViewId,
    breakpoint: &str,
    engine: EngineKind,
    file: PresetFile,
    changed: bool,
) -> Value {
    json!({
        "view": view,
        "breakpoint": breakpoint,
        "engine": config_name(engine),
        "file": file.name(),
        "changed": changed,
    })
}

/// The command's definition: Breakpoints group, `Leader B E`, MCP `views_set_engine`.
pub fn definition() -> Result<CommandDef, CommandError> {
    Ok(CommandDef {
        id: CommandId::new(SET_ENGINE)?,
        title_key: format!("commands.{SET_ENGINE}"),
        group: Group::Breakpoints,
        default_bindings: vec![Binding::Leader(Chord {
            keys: vec![Key::Char('B'), Key::Char('E')],
        })],
        mcp: McpCoverage::Covered(MCP_TOOL.to_owned()),
    })
}

/// Registers the command with `registry`; returns what the registry refused.
pub fn register(registry: &mut CommandRegistry, core: &Arc<Core>) -> Vec<CommandError> {
    let def = match definition() {
        Ok(def) => def,
        Err(error) => return vec![error],
    };
    let weak: Weak<Core> = Arc::downgrade(core);
    let handler: Handler = Arc::new(move |args: Value| {
        let core = weak
            .upgrade()
            .ok_or_else(|| CommandError::Failed("the window is closed".to_owned()))?;
        run(&core, &args)
    });
    registry.register(def, handler).err().into_iter().collect()
}

/// The core is up: the window learns whether the badges show.
pub fn on_start(core: &Core) {
    on_user_settings_changed(core);
}

/// The user settings changed: the window follows `ui.engineBadge`.
pub fn on_user_settings_changed(core: &Core) {
    core.ui()
        .set_engine_badges(core.user_settings().ui.engine_badge);
}

#[cfg(test)]
#[path = "set_engine_tests.rs"]
mod tests;
