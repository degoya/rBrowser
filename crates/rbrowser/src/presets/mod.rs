//! Environments, layout presets and focus mode (RB-010-12): switching keeps path, query and
//! hash; presets rebuild the grid without reloading kept views; focus mode with its quick
//! switcher; zoom to fit; the environment's frame (`cef_app/frame.rs`); the device presets for
//! the shell (`rb_config::effective_devices`). Its state is
//! [`crate::host::shell_state::ShellState`] (`environment`, `preset`, `mode`, `focus`).
//!
//! The grid always fits the window (flex BoxLayouts, ADR 0001), so there is nothing to pan. A
//! preset in a layout mode (horizontal, split, canvas) is shown by `crate::layout` (RB-080-04).

pub(crate) mod actions;
mod env;
mod layouts;

use rb_commands::CommandError;
use rb_config::ConfigError;
use rb_ipc::{CoreToShell, IpcErrorCode, ShellToCore};
use serde_json::Value;

use crate::control::Core;
use crate::ipc::Failure;

pub use actions::{ActionError, apply as apply_selection, current_selection, default_zoom};
pub use env::{frame_argb, rewrite_url};
pub use layouts::{PresetError, Selection, resolve};

/// The prefix of the numbered preset commands (`layout.preset.1` ... `9`).
const PRESET_PREFIX: &str = "layout.preset.";
/// The prefix of the numbered environment commands (`environments.switch.1` ... `9`).
const ENVIRONMENT_PREFIX: &str = "environments.switch.";
/// `args.source` of `layout.focus_next` / `focus_prev` when the shell's arrow keys send them.
pub const ARROW_KEY_SOURCE: &str = "arrowKey";

/// Runs the command `id` when it is this module's (`layout.*`, `environments.*`);
/// `None` otherwise. Arguments: `layout.select_preset { id? }`, `environments.select { id? }`
/// (without an id: the next one), `layout.focus_mode { enabled?, focus? }`,
/// `layout.focus_next` / `focus_prev { source? }`.
pub fn run_command(core: &Core, id: &str, args: &Value) -> Option<Result<Value, CommandError>> {
    let text = |name: &str| args.get(name).and_then(Value::as_str);
    let result = match id {
        "layout.select_preset" => actions::select_preset(core, text("id")),
        "environments.select" => actions::select_environment(core, text("id")),
        "layout.focus_mode" => actions::toggle_focus(
            core,
            args.get("enabled").and_then(Value::as_bool),
            text("focus"),
        ),
        "layout.focus_next" | "layout.focus_prev" => actions::step_focus(
            core,
            id == "layout.focus_next",
            text("source") == Some(ARROW_KEY_SOURCE),
        ),
        "layout.zoom_to_fit" => actions::zoom_to_fit(core),
        other => {
            if let Some(number) = numbered(other, PRESET_PREFIX) {
                actions::select_preset_number(core, number)
            } else {
                actions::select_environment_number(core, numbered(other, ENVIRONMENT_PREFIX)?)
            }
        }
    };
    Some(result.map(|()| Value::Null).map_err(CommandError::from))
}

/// Answers `message` when it is this module's (`selectLayoutPreset`, `selectEnvironment`,
/// `getDevices`); `None` otherwise.
pub fn ipc(core: &Core, message: &ShellToCore) -> Option<Result<CoreToShell, Failure>> {
    let answer = match message {
        ShellToCore::SelectLayoutPreset { id } => {
            actions::select_preset(core, Some(id)).map(|()| CoreToShell::Ack)
        }
        ShellToCore::SelectEnvironment { id } => {
            actions::select_environment(core, Some(id)).map(|()| CoreToShell::Ack)
        }
        ShellToCore::GetDevices => {
            return Some(
                rb_config::effective_devices(&core.user_settings().devices)
                    .map(|items| CoreToShell::Devices { items })
                    .map_err(devices_failure),
            );
        }
        _ => return None,
    };
    Some(answer.map_err(failure))
}

/// The project's grid is built: the start environment and its frame, `layouts.default`.
pub fn on_start(core: &Core) {
    if let Err(error) = actions::restore(core, true) {
        tracing::warn!(%error, "the start layout preset was not applied");
    }
}

/// The config was read again and the host rebuilt its grid: the environment and the
/// selection in effect are applied again (a removed one falls back to the start one).
pub fn on_config_changed(core: &Core) {
    if let Err(error) = actions::restore(core, false) {
        tracing::warn!(%error, "the layout preset was not applied again");
    }
}

/// The number after `prefix` in a command id (`layout.preset.3` -> 3).
fn numbered(id: &str, prefix: &str) -> Option<usize> {
    id.strip_prefix(prefix)?.parse().ok()
}

fn failure(error: ActionError) -> Failure {
    let code = match &error {
        ActionError::Preset(
            PresetError::UnknownPreset(_)
            | PresetError::UnknownEnvironment(_)
            | PresetError::UnknownBreakpoint(_),
        ) => IpcErrorCode::NotFound,
        ActionError::Preset(_) | ActionError::Engine(rb_engine::EngineError::Invalid(_)) => {
            IpcErrorCode::BadRequest
        }
        ActionError::Engine(_) => IpcErrorCode::Internal,
    };
    (code, error.to_string())
}

/// The user's device presets are invalid: the keys of the problems, for the shell.
fn devices_failure(error: ConfigError) -> Failure {
    match error {
        ConfigError::Invalid(issues) => (
            IpcErrorCode::BadRequest,
            issues
                .iter()
                .map(|issue| format!("{} {}", issue.path, issue.key))
                .collect::<Vec<_>>()
                .join("; "),
        ),
        other => (IpcErrorCode::Internal, other.key().to_owned()),
    }
}

#[cfg(test)]
#[path = "presets_tests.rs"]
mod tests;
