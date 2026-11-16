//! Breakpoint shortcuts for the active view (RB-010-16): a key, the size quick pick or MCP sets
//! the active view (focused or last clicked) to a breakpoint - its size and full emulation, live,
//! without a reload, sync on - or to a free size, +/-1 px, back to its cell's breakpoint, or a
//! CSS breakpoint the analyser found. With the new-view modifier the action opens a new view
//! instead. The label strip of the cell shows the result for a moment.
//!
//! | Command | Default | |
//! | --- | --- | --- |
//! | `view.set_breakpoint.1...9` | `Ctrl+Shift+1...9`, `Leader V 1...9` | breakpoint n in config order |
//! | `view.next_breakpoint` / `prev` | `Leader ]` / `Leader [` | by width, around at the ends |
//! | `view.nudge_width.plus` / `minus` | `Leader .` / `Leader -` | +/-1 px |
//! | `view.reset_size` | `Leader 0` | the breakpoint of the view's cell |
//! | `breakpoints.detected.1...9` | `Leader B 1...9` | the analyser's CSS breakpoints by width |
//! | `view.set_size` | `Leader V S` | arguments, else the free input of the shell |
//!
//! Every command takes `{ view?, newView? }`; `view.set_size` also `breakpoint` (an id of
//! the config or a device preset) or `width` / `height` (`dpr`). MCP: `view_set_breakpoint`.
//! The project's own keys (`breakpoints[].shortcut`) are [`project_keys`]; the engine badge at
//! the strip's right end and its menu are [`engine_pick`] (RB-080-33).

mod actions;
mod defs;
pub mod engine_pick;
mod project_keys;
mod quick_pick;
mod sizes;

use rb_commands::{
    Binding, CommandError, CommandId, Modifiers, Platform, Shortcut, builtin_reserved,
};
use rb_config::NewViewModifier;
use rb_engine::{ViewHost, ViewId};
use rb_ipc::{
    CoreToShell, IpcErrorCode, Notification, NotificationLevel, ProjectShortcutList, ShellToCore,
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::control::Core;
use crate::emulation::device_list;
use crate::ipc::Failure;

pub use actions::{Change, Goal, OVERLAY_KEY, apply, format_dpr, overlay_text};
pub use defs::{
    DETECTED_PREFIX, MCP_TOOL, NARROWER, NEXT, PREV, RESET, SET, SET_PREFIX, WIDER, definitions,
    is_own, register,
};
pub use engine_pick::{Badge, EngineItem, badge_of, engine_pick, pick_engine};
pub use project_keys::{CONFLICT_KEY, Conflict, ProjectKey, REJECTED_CHORD};
pub use quick_pick::{Current, PickItem, Section, command_args};
pub use sizes::{FreeSize, MAX_SIDE, parse_free_size};

/// The size arithmetic the add menu of RB-080-41 shares with the quick pick.
pub mod sizes_public {
    pub use super::sizes::{by_width, size_of};
}

/// The arguments every command of the module takes.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Args {
    /// The view; the active view when absent.
    #[serde(default, alias = "viewId")]
    view: Option<ViewId>,
    /// Open a new view instead.
    #[serde(default)]
    new_view: bool,
    /// `view.set_size`: a breakpoint or device preset id.
    #[serde(default)]
    breakpoint: Option<String>,
    /// `view.set_size`: a free width (the height stays when absent).
    #[serde(default)]
    width: Option<u32>,
    /// `view.set_size`: a free height (the width stays when absent).
    #[serde(default)]
    height: Option<u32>,
    /// `view.set_size`: a free DPR.
    #[serde(default)]
    dpr: Option<f64>,
}

/// Runs the command `id` of this module.
pub fn run(core: &Core, id: &str, args: &Value) -> Result<Value, CommandError> {
    let args: Args = if args.is_null() {
        Args::default()
    } else {
        serde_json::from_value(args.clone())
            .map_err(|error| CommandError::InvalidArgs(error.to_string()))?
    };
    let goal = Goal {
        view: args.view,
        new_view: args.new_view,
    };
    let host = core.host();
    let config = host.config();
    let view = actions::view_of(core, goal)?;
    let emulation = host.emulation(view).unwrap_or_default();
    let info = host
        .view(view)
        .ok_or_else(|| CommandError::InvalidArgs(format!("unknown view {view}")))?;
    let goal = Goal {
        view: Some(view),
        ..goal
    };
    match id {
        SET => set(core, goal, &args, emulation.width, emulation.height),
        NEXT | PREV => {
            let breakpoint = sizes::step(
                &config.breakpoints,
                &device_list(),
                info.breakpoint.as_deref(),
                emulation.width,
                id == NEXT,
            )
            .ok_or_else(|| CommandError::Failed("the project has no breakpoint".to_owned()))?;
            apply(core, goal, Change::Breakpoint(breakpoint))
        }
        WIDER | NARROWER => {
            let delta = if id == WIDER { 1 } else { -1 };
            let width = sizes::nudged(emulation.width, delta).ok_or_else(|| {
                CommandError::InvalidArgs(format!("{} px is the limit", emulation.width))
            })?;
            apply(core, goal, free(width, emulation.height, None)?)
        }
        RESET => {
            let cell = info
                .cell_breakpoint
                .ok_or_else(|| CommandError::Failed(format!("view {view} has no cell")))?;
            let breakpoint = actions::find(core, &cell)?;
            apply(core, goal, Change::Breakpoint(&breakpoint))
        }
        other => {
            if let Some(number) = defs::numbered(other, SET_PREFIX) {
                let breakpoint = config.breakpoints.get(number - 1).ok_or_else(|| {
                    CommandError::InvalidArgs(format!("the project has no breakpoint {number}"))
                })?;
                apply(core, goal, Change::Breakpoint(breakpoint))
            } else if let Some(number) = defs::numbered(other, DETECTED_PREFIX) {
                let width = sizes::detected_width(&host.shell_state().detected, number)
                    .ok_or_else(|| {
                        CommandError::InvalidArgs(format!("no detected breakpoint {number}"))
                    })?;
                apply(core, goal, free(width, emulation.height, None)?)
            } else {
                Err(CommandError::UnknownCommand(other.to_owned()))
            }
        }
    }
}

/// `view.set_size`: a breakpoint or device preset, a free size, or - without either -
/// the free input of the shell.
fn set(
    core: &Core,
    goal: Goal,
    args: &Args,
    width: u32,
    height: u32,
) -> Result<Value, CommandError> {
    if let Some(id) = &args.breakpoint {
        if args.width.is_some() || args.height.is_some() || args.dpr.is_some() {
            return Err(CommandError::InvalidArgs(
                "either `breakpoint` or `width`/`height`".to_owned(),
            ));
        }
        let breakpoint = actions::find(core, id)?;
        return apply(core, goal, Change::Breakpoint(&breakpoint));
    }
    if args.width.is_none() && args.height.is_none() && args.dpr.is_none() {
        core.send(&CoreToShell::UiCommand { id: SET.to_owned() });
        return Ok(Value::Null);
    }
    let (width, height) = (args.width.unwrap_or(width), args.height.unwrap_or(height));
    apply(core, goal, free(width, height, args.dpr)?)
}

/// A free size as a change; an invalid one is refused.
fn free(width: u32, height: u32, dpr: Option<f64>) -> Result<Change<'static>, CommandError> {
    FreeSize::new(width, height, dpr)
        .map(Change::Size)
        .ok_or_else(|| {
            CommandError::InvalidArgs(format!("{width}x{height} at DPR {dpr:?} is not a size"))
        })
}

/// Answers the shell's `setBreakpoint` (the quick pick) and `listProjectShortcuts` (the
/// shortcut editor's conflict lines); `None` for every other message.
pub fn ipc(core: &Core, message: &ShellToCore) -> Option<Result<CoreToShell, Failure>> {
    let (view, breakpoint, width, height, dpr) = match message {
        ShellToCore::SetBreakpoint {
            view,
            breakpoint,
            width,
            height,
            dpr,
        } => (view, breakpoint, width, height, dpr),
        ShellToCore::ListProjectShortcuts => {
            let (keys, conflicts) = project_shortcuts(core);
            let items = project_keys::listing(&core.host().config(), &keys, &conflicts);
            return Some(Ok(CoreToShell::ProjectShortcuts(ProjectShortcutList {
                items,
            })));
        }
        _ => return None,
    };
    let args = json!({
        "view": view,
        "breakpoint": breakpoint,
        "width": width,
        "height": height,
        "dpr": dpr,
    });
    let args: Value = args
        .as_object()
        .map(|map| {
            map.iter()
                .filter(|(_, value)| !value.is_null())
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect()
        })
        .unwrap_or_default();
    Some(match run(core, SET, &args) {
        Ok(_) => Ok(CoreToShell::Ack),
        Err(CommandError::InvalidArgs(message)) => Err((IpcErrorCode::BadRequest, message)),
        Err(error) => Err((IpcErrorCode::Internal, error.to_string())),
    })
}

/// The core is up: the project's shortcuts are checked and their conflicts reported.
pub fn on_start(core: &Core) {
    report_conflicts(core);
}

/// The config was read again: its shortcuts are checked again.
pub fn on_config_changed(core: &Core) {
    report_conflicts(core);
}

/// The project shortcuts of the current config and their conflicts, against the registry's
/// bindings in effect.
pub fn project_shortcuts(core: &Core) -> (Vec<ProjectKey>, Vec<Conflict>) {
    let CoreToShell::Commands { commands, .. } = core.commands() else {
        return (Vec::new(), Vec::new());
    };
    let held: Vec<(String, Binding)> = commands
        .iter()
        .flat_map(|command| {
            command
                .bindings
                .iter()
                .filter_map(|text| text.parse().ok())
                .map(|binding| (command.id.clone(), binding))
        })
        .collect();
    project_keys::project_keys(
        &core.host().config(),
        &held,
        builtin_reserved(),
        Platform::current(),
    )
}

fn report_conflicts(core: &Core) {
    for conflict in project_shortcuts(core).1 {
        tracing::warn!(
            breakpoint = %conflict.breakpoint,
            binding = %conflict.binding,
            reason = %conflict.reason_key,
            "a project breakpoint shortcut is not in force"
        );
        let mut args = std::collections::BTreeMap::from([
            ("breakpoint".to_owned(), conflict.breakpoint),
            ("binding".to_owned(), conflict.binding),
            ("reason".to_owned(), conflict.reason_key),
        ]);
        args.insert("other".to_owned(), conflict.other.unwrap_or_default());
        core.notify(CoreToShell::Notification(Notification {
            level: NotificationLevel::Warning,
            message_key: CONFLICT_KEY.to_owned(),
            args,
            settings_key: Some("shortcuts.bindings".to_owned()),
        }));
    }
}

/// A key no command took (`KeyOutcome::PassThrough`): a project shortcut sets its breakpoint,
/// and with `breakpointShortcuts.newViewModifier` held a breakpoint shortcut (the registry's,
/// via `resolve`, or the project's) opens a new view. A reserved combination as pressed (the
/// modifier included: `Ctrl` with a project's `Alt+F9` is AltGr) is never taken. `true` when the
/// key was taken; the command runs off the calling (UI) thread.
pub fn on_unbound_key(
    core: &Core,
    shortcut: Shortcut,
    resolve: impl Fn(&Binding) -> Option<CommandId>,
) -> bool {
    if !project_keys::may_be_project_key(&shortcut)
        || builtin_reserved()
            .find(&shortcut, Platform::current())
            .is_some()
    {
        return false;
    }
    let (plain, new_view) = match strip_modifier(core, shortcut) {
        Some(plain) => (plain, true),
        None => (shortcut, false),
    };
    if new_view && let Some(id) = resolve(&Binding::Direct(plain)).filter(|id| is_own(id.as_str()))
    {
        spawn(core, id.as_str(), json!({ "newView": true }));
        return true;
    }
    let (keys, _) = project_shortcuts(core);
    let Some(breakpoint) = project_keys::breakpoint_for(&keys, &plain) else {
        return false;
    };
    spawn(
        core,
        SET,
        json!({ "breakpoint": breakpoint, "newView": new_view }),
    );
    true
}

/// `shortcut` without the new-view modifier, when the setting names one and it is held.
fn strip_modifier(core: &Core, shortcut: Shortcut) -> Option<Shortcut> {
    let mut plain = shortcut;
    let held = match modifier_setting(core)? {
        NewViewModifier::Shift => std::mem::replace(&mut plain.modifiers.shift, false),
        NewViewModifier::Ctrl => std::mem::replace(&mut plain.modifiers.ctrl, false),
    };
    held.then_some(plain)
}

/// Whether `modifiers` hold the new-view modifier of the settings.
pub fn wants_new_view(core: &Core, modifiers: Modifiers) -> bool {
    match modifier_setting(core) {
        Some(NewViewModifier::Shift) => modifiers.shift,
        Some(NewViewModifier::Ctrl) => modifiers.ctrl,
        None => false,
    }
}

/// `breakpointShortcuts.newViewModifier`.
fn modifier_setting(core: &Core) -> Option<NewViewModifier> {
    core.user_settings().breakpoint_shortcuts.new_view_modifier
}

/// A row of the quick pick was chosen for `view` with `modifiers` held: it runs
/// `view.set_size` off the calling (UI) thread; the free input row opens the shell's.
pub fn pick(core: &Core, view: ViewId, item: &PickItem, modifiers: Modifiers) {
    let new_view = wants_new_view(core, modifiers);
    let args = command_args(item, view, new_view).unwrap_or(Value::Null);
    spawn(core, SET, args);
}

fn spawn(core: &Core, id: &str, args: Value) {
    let command = id.to_owned();
    core.run_command_async(id, args, move |result| {
        if let Err(error) = result {
            tracing::warn!(%command, %error, "a breakpoint shortcut failed");
        }
    });
}

/// The rows of the size quick pick for `view` (the label strip's native menu, RB-010-16).
pub fn quick_pick(core: &Core, view: ViewId) -> Vec<PickItem> {
    let host = core.host();
    let info = host.view(view);
    let width = host.emulation(view).map_or(0, |emulation| emulation.width);
    quick_pick::items(
        &host.config(),
        &device_list(),
        &host.shell_state().detected,
        Current {
            breakpoint: info.as_ref().and_then(|info| info.breakpoint.as_deref()),
            width,
        },
    )
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
