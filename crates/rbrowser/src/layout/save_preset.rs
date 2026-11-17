//! "Save layout as preset" (RB-080-04, `layout.save_preset { id, file? }`, MCP
//! `layout_save_preset`): the layout in effect written as the preset `id` - after arranging the
//! views on the canvas, say - and selected, so the toolbar shows it.
//!
//! The preset keeps the mode shown and what that mode needs: the grid in effect (grid), the
//! breakpoint shown (focus), the session's fields (horizontal, split) or every view's place on the
//! canvas, the ones never moved included (canvas; the camera stays in the session), and the views
//! open now as `hidden` (RB-080-41). A preset that has a grid of its own lends it to the new one. It goes to `config.local.json` or `config.json`
//! (`file`, else the user setting `layoutModes.savePresetTo`), replacing a preset of that id
//! there. Without an `id` the shell asks for the name (`uiCommand`, `SavePresetBar.vue`).

use rb_commands::CommandError;
use rb_config::{Layout, LayoutMode, LayoutModes, LayoutPreset, PresetFile};
use rb_ipc::CoreToShell;
use serde::Deserialize;
use serde_json::{Value, json};

use super::canvas_ops::{canvas_state, mode_and_area, set_camera};
use super::defs::SAVE_PRESET;
use super::store::{self, mode_name};
use crate::control::Core;
use crate::presets::{ActionError, Selection, default_zoom};

/// The longest preset name, in characters.
pub const MAX_ID_CHARS: usize = 40;

/// `layout.save_preset`'s arguments.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct SaveArgs {
    /// The preset's name; the shell asks for one when absent.
    id: Option<String>,
    /// `local` or `project`; the user setting when absent.
    file: Option<PresetFile>,
}

/// `layout.save_preset`: answers `{ id, file, mode }`, or `null` when the shell was asked for
/// the name.
pub fn run(core: &Core, args: &Value) -> Result<Value, CommandError> {
    let args: SaveArgs = if args.is_null() {
        SaveArgs::default()
    } else {
        serde_json::from_value(args.clone())
            .map_err(|error| CommandError::InvalidArgs(error.to_string()))?
    };
    let Some(raw) = args.id else {
        // The name field sits in the toolbar strip: the shell takes the keyboard.
        core.ui().focus_shell();
        core.send(&CoreToShell::UiCommand {
            id: SAVE_PRESET.to_owned(),
            args: None,
        });
        return Ok(Value::Null);
    };
    let id = preset_id(&raw)?;
    let file = args
        .file
        .unwrap_or(core.user_settings().layout_modes.save_preset_to);
    let host = core.host();
    let preset = preset_in_effect(core)?;
    let mode = preset.mode;
    let camera = host.shell_state().modes.camera;
    let value =
        serde_json::to_value(&preset).map_err(|error| CommandError::Failed(error.to_string()))?;
    store::save_preset(core, &id, file.config_file(), &value)?;
    // The watcher reads the file again too; the preset is selected now, from the file.
    crate::commands::reload_config(host).map_err(CommandError::Failed)?;
    crate::presets::actions::select_preset(core, Some(&id))?;
    if mode == LayoutMode::Canvas
        && let Some(camera) = camera
    {
        set_camera(core, camera);
    }
    Ok(json!({ "id": id, "file": file.name(), "mode": mode_name(mode) }))
}

/// `raw` trimmed, when it is 1 to [`MAX_ID_CHARS`] letters, digits, spaces, `-`, `_` or `.`.
pub fn preset_id(raw: &str) -> Result<String, CommandError> {
    let id = raw.trim();
    let count = id.chars().count();
    let allowed = |c: char| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.');
    if count == 0 || count > MAX_ID_CHARS || !id.chars().all(allowed) {
        return Err(CommandError::InvalidArgs(format!(
            "a preset name is 1 to {MAX_ID_CHARS} letters, digits, spaces, '-', '_' or '.'"
        )));
    }
    Ok(id.to_owned())
}

/// The layout shown now as a preset.
fn preset_in_effect(core: &Core) -> Result<LayoutPreset, CommandError> {
    let host = core.host();
    let config = host.config();
    let shell = host.shell_state();
    let current = shell
        .preset
        .clone()
        .filter(|id| config.layouts.presets.contains_key(id));
    let own_grid = current
        .as_deref()
        .and_then(|id| config.layouts.presets.get(id))
        .and_then(|preset| preset.layout.clone())
        .map(without_modes);
    let mode = shell.mode;
    // The views open now go with the preset (RB-080-41): `[]` when every one is.
    let hidden = Some(rb_config::hidden_selection(&config));
    let preset = match mode {
        LayoutMode::Grid => {
            let selection = Selection {
                preset: current,
                mode,
                focus: None,
            };
            // The grid goes in whole: a hidden breakpoint keeps its cell and `hidden` names
            // it, so a view added later has its slot (RB-080-41).
            let mut whole = config.clone();
            whole.layout.hidden.clear();
            let (grid, _) =
                crate::presets::resolve(&whole, &selection, default_zoom(&core.user_settings()))
                    .map_err(ActionError::from)?;
            LayoutPreset {
                mode,
                layout: Some(without_modes(grid)),
                hidden,
                ..LayoutPreset::default()
            }
        }
        LayoutMode::Focus => LayoutPreset {
            mode,
            layout: own_grid,
            focus: shell.focus.clone(),
            hidden,
            ..LayoutPreset::default()
        },
        _ => {
            let mut fields = shell.modes.fields.clone();
            if mode == LayoutMode::Canvas
                && let Ok((spec, area)) = mode_and_area(host, LayoutMode::Canvas)
            {
                fields.items = canvas_state(&spec, &host.mode_sizes(), area).0;
            }
            LayoutPreset {
                mode,
                layout: own_grid,
                focus: None,
                modes: fields,
                hidden,
            }
        }
    };
    Ok(preset)
}

/// `layout` without the top-level layout's `modes` and `hidden`, which a preset's grid
/// ignores (the preset's own `hidden` carries the selection).
fn without_modes(layout: Layout) -> Layout {
    Layout {
        modes: LayoutModes::default(),
        hidden: Vec::new(),
        ..layout
    }
}

#[cfg(test)]
#[path = "save_preset_tests.rs"]
mod tests;
