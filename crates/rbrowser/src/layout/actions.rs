//! What the commands of the layout modes do (RB-080-04): switch the mode of the current preset,
//! show a mode for a selection (called by the presets when they apply one), move and swap the
//! split's divider, the minimap switch. The canvas's and the strip's own commands are
//! [`super::canvas_ops`]. The session's side of a mode lives in the shell state
//! ([`ModeSession`]); what is kept across a restart is written into the config ([`super::store`]).

use rb_commands::CommandError;
use rb_config::{Layout, LayoutMode, ModeFields, ProjectConfig};
use rb_engine::{ViewHost, ViewId};
use serde_json::{Map, Value, json};

use super::defs::{
    CANVAS_PLACE, CANVAS_ZOOM_IN, CANVAS_ZOOM_OUT, CANVAS_ZOOM_RESET, CENTER, MINIMAP_TOGGLE,
    MODE_CANVAS, MODE_GRID, MODE_HORIZONTAL, MODE_SPLIT, SAVE_PRESET, SPLIT_GROW, SPLIT_SHRINK,
    SPLIT_SWAP,
};
use super::spec::{ModeInputs, ModeKind, ModeSession, resolve_mode};
use super::split::step_ratio;
use super::{canvas_ops, store};
use crate::control::Core;
use crate::host::Host;
use crate::presets::{ActionError, PresetError, Selection, default_zoom};

/// Runs the layout-mode command `id` with `args`.
pub fn run(core: &Core, id: &str, args: &Value) -> Result<Value, CommandError> {
    match id {
        MODE_GRID => switch(core, LayoutMode::Grid, args),
        MODE_HORIZONTAL => switch(core, LayoutMode::Horizontal, args),
        MODE_SPLIT => switch(core, LayoutMode::Split, args),
        MODE_CANVAS => switch(core, LayoutMode::Canvas, args),
        MINIMAP_TOGGLE => Ok(toggle_minimap(core)),
        SPLIT_SWAP => swap(core),
        SPLIT_GROW | SPLIT_SHRINK => step(core, id == SPLIT_GROW),
        CENTER => canvas_ops::center(core, args),
        CANVAS_PLACE => canvas_ops::place(core, args),
        SAVE_PRESET => super::save_preset::run(core, args),
        CANVAS_ZOOM_RESET | CANVAS_ZOOM_IN | CANVAS_ZOOM_OUT => {
            super::canvas_level::run(core, id, args)
        }
        other => Err(CommandError::InvalidArgs(format!(
            "unknown command {other}"
        ))),
    }
}

/// Whether `mode` is one of this module's (placed views) rather than grid or focus.
pub fn is_mode(mode: LayoutMode) -> bool {
    matches!(
        mode,
        LayoutMode::Horizontal | LayoutMode::Split | LayoutMode::Canvas
    )
}

/// Shows `selection` (a mode of [`is_mode`]) over its base grid `grid`: the session takes the
/// preset's fields over, the spec is resolved and the host places the views.
pub fn show(core: &Core, selection: &Selection, grid: &Layout) -> Result<(), ActionError> {
    let host = core.host();
    let config = host.config();
    let stored = stored_fields(&config, selection.preset.as_deref(), selection.mode);
    let settings = core.user_settings().layout_modes.clamped();
    host.update_shell_state(|shell| {
        shell
            .modes
            .enter(selection.preset.as_deref(), selection.mode, &stored);
        shell.modes.settings = settings;
    });
    let session = host.shell_state().modes;
    let spec = spec_for(core, &config, selection.mode, &session, grid)
        .ok_or(PresetError::NoBreakpoints)?;
    host.set_mode(spec)?;
    Ok(())
}

/// The fields of `mode` kept for `preset`: the preset's own when it has `mode`, else the
/// top-level layout's `layout.modes` ([`store::stored`]).
pub fn stored_fields(config: &ProjectConfig, preset: Option<&str>, mode: LayoutMode) -> ModeFields {
    store::stored(config, preset, mode)
}

/// The spec of `mode` for `session` over `grid`.
fn spec_for(
    core: &Core,
    config: &ProjectConfig,
    mode: LayoutMode,
    session: &ModeSession,
    grid: &Layout,
) -> Option<super::ModeSpec> {
    let host = core.host();
    let user = core.user_settings();
    let settings = user.layout_modes.clamped();
    let active = active_breakpoint(host);
    // Every breakpoint of the config, on any engine: a view on Firefox or WebKit keeps its
    // Chromium stand-in here as in the grid (RB-080-39) - a canvas of items only and the split's
    // sides lost it (RB-090-74).
    let known = |id: &str| {
        config
            .breakpoints
            .iter()
            .any(|breakpoint| breakpoint.id == id)
    };
    // The hidden selection holds in every mode (RB-080-41).
    let hidden = rb_config::hidden_selection(config);
    resolve_mode(
        mode,
        session,
        &ModeInputs {
            grid,
            settings: &settings,
            active: active.as_deref(),
            default_zoom: default_zoom(&user),
            known: &known,
            hidden: &hidden,
        },
    )
}

/// `layout.mode.<mode>`: the current preset in `mode`; `args` set the mode's fields (the
/// session's, and the preset's when given).
fn switch(core: &Core, mode: LayoutMode, args: &Value) -> Result<Value, CommandError> {
    let host = core.host();
    let current = crate::presets::current_selection(host);
    let patch: ModeFields = if args.is_null() {
        ModeFields::default()
    } else {
        serde_json::from_value(args.clone())
            .map_err(|error| CommandError::InvalidArgs(error.to_string()))?
    };
    let selection = Selection {
        preset: current.preset,
        mode,
        focus: None,
    };
    let changed = patch != ModeFields::default();
    // The toolbar's divider in split mode: the views move, nothing is rebuilt.
    if let Some(ratio) = only_ratio(&patch)
        && mode == LayoutMode::Split
        && current.mode == LayoutMode::Split
        && host.mode_spec().is_some()
    {
        let answer = set_ratio(core, ratio)?;
        return Ok(json!({ "mode": store::mode_name(mode), "persisted": answer["persisted"] }));
    }
    if is_mode(mode) {
        let stored = stored_fields(&host.config(), selection.preset.as_deref(), mode);
        host.update_shell_state(|shell| {
            shell
                .modes
                .enter(selection.preset.as_deref(), mode, &stored);
            merge(&mut shell.modes.fields, &patch);
        });
    }
    crate::presets::apply_selection(core, &selection, false)?;
    let persisted = if changed && is_mode(mode) {
        persist_session(core)
    } else {
        None
    };
    Ok(json!({ "mode": store::mode_name(mode), "persisted": persisted }))
}

/// The ratio, when it is the only field `patch` sets.
fn only_ratio(patch: &ModeFields) -> Option<f64> {
    let ratio = patch.ratio?;
    let rest = ModeFields {
        ratio: None,
        ..patch.clone()
    };
    (rest == ModeFields::default()).then_some(ratio)
}

/// The fields `patch` sets, over `fields`.
pub fn merge(fields: &mut ModeFields, patch: &ModeFields) {
    let ModeFields {
        height,
        gap,
        orientation,
        ratio,
        primary,
        secondary,
        snap,
        items,
    } = patch;
    fields.height = height.or(fields.height);
    fields.gap = gap.or(fields.gap);
    fields.orientation = orientation.or(fields.orientation);
    fields.ratio = ratio.or(fields.ratio);
    fields.snap = snap.or(fields.snap);
    if !primary.is_empty() {
        fields.primary.clone_from(primary);
    }
    if !secondary.is_empty() {
        fields.secondary.clone_from(secondary);
    }
    if !items.is_empty() {
        fields.items.clone_from(items);
    }
}

/// `layout.split.grow` / `shrink`: the divider one `layoutModes.splitStep` further.
fn step(core: &Core, grow: bool) -> Result<Value, CommandError> {
    let host = core.host();
    let Some(ModeKind::Split { ratio, .. }) = host.mode_spec().map(|spec| spec.kind) else {
        return Err(not_in(LayoutMode::Split));
    };
    let step = core.user_settings().layout_modes.clamped().split_step;
    set_ratio(core, step_ratio(ratio, step, grow))
}

/// The split's ratio set to `ratio` (the toolbar's divider control passes it to
/// `layout.mode.split`; the keyboard steps come here).
pub fn set_ratio(core: &Core, ratio: f64) -> Result<Value, CommandError> {
    let host = core.host();
    let ratio = super::split::clamp_ratio(ratio);
    let placed = host.update_mode(|spec| {
        if let ModeKind::Split { ratio: current, .. } = &mut spec.kind {
            *current = ratio;
        }
    });
    if !placed {
        return Err(not_in(LayoutMode::Split));
    }
    host.update_shell_state(|shell| shell.modes.fields.ratio = Some(ratio));
    let persisted = persist_session(core);
    Ok(json!({ "ratio": ratio, "persisted": persisted }))
}

/// `layout.split.swap`: the primary and secondary sides trade places.
fn swap(core: &Core) -> Result<Value, CommandError> {
    let host = core.host();
    let Some(spec) = host.mode_spec() else {
        return Err(not_in(LayoutMode::Split));
    };
    let ModeKind::Split { primary, .. } = spec.kind else {
        return Err(not_in(LayoutMode::Split));
    };
    let count = primary.min(spec.breakpoints.len());
    let (first, second) = spec.breakpoints.split_at(count);
    let (first, second) = (first.to_vec(), second.to_vec());
    host.update_shell_state(|shell| {
        shell.modes.fields.primary.clone_from(&second);
        shell.modes.fields.secondary.clone_from(&first);
    });
    reshow(core)?;
    let persisted = persist_session(core);
    Ok(json!({ "primary": second, "secondary": first, "persisted": persisted }))
}

/// `layout.minimap.toggle`: the minimap off or on for this session.
fn toggle_minimap(core: &Core) -> Value {
    let shown = core.user_settings().layout_modes.minimap;
    let mut now = shown;
    core.host().update_shell_state(|shell| {
        now = !shell.modes.minimap.unwrap_or(shown);
        shell.modes.minimap = Some(now);
    });
    json!({ "minimap": now })
}

/// Shows the current selection again with the session's fields, over its base grid (a preset
/// the config no longer has counts as the top-level layout's grid).
pub fn reshow(core: &Core) -> Result<(), CommandError> {
    let host = core.host();
    let config = host.config();
    let selection = crate::presets::current_selection(host);
    let known = Selection {
        preset: selection
            .preset
            .clone()
            .filter(|id| config.layouts.presets.contains_key(id)),
        mode: LayoutMode::Grid,
        focus: None,
    };
    let user = core.user_settings();
    let (grid, _) =
        crate::presets::resolve(&config, &known, default_zoom(&user)).map_err(ActionError::from)?;
    let session = host.shell_state().modes;
    let spec = spec_for(core, &config, selection.mode, &session, &grid)
        .ok_or(ActionError::Preset(PresetError::NoBreakpoints))?;
    host.set_mode(spec).map_err(ActionError::from)?;
    Ok(())
}

/// Writes the session's fields into their place (the preset of the mode, else the top-level
/// layout's `layout.modes`, see [`super::store`]); answers where, `None` (logged) when they
/// could not be written - the session keeps them. The current preset stays.
pub fn persist_session(core: &Core) -> Option<String> {
    let host = core.host();
    let shell = host.shell_state();
    let mode = shell.modes.mode.filter(|mode| is_mode(*mode))?;
    let fields = shell.modes.fields.clone();
    let target = store::target(&host.config(), shell.preset.as_deref(), mode);
    match store::persist(core, &target, mode, &field_map(&fields)) {
        Ok(label) => {
            host.update_shell_state(|shell| shell.modes.persisted = fields);
            Some(label)
        }
        Err(error) => {
            tracing::warn!(%error, "the layout was not written into the config");
            None
        }
    }
}

/// Every mode field as a JSON key; an unset one is `null`, so the preset loses a stale value.
pub(super) fn field_map(fields: &ModeFields) -> Map<String, Value> {
    let mut map = Map::new();
    for key in [
        "height",
        "gap",
        "orientation",
        "ratio",
        "primary",
        "secondary",
        "snap",
        "items",
    ] {
        map.insert(key.to_owned(), Value::Null);
    }
    if let Ok(Value::Object(set)) = serde_json::to_value(fields) {
        map.extend(set);
    }
    map
}

/// The breakpoint of the active view's cell (else the first view's).
pub fn active_breakpoint(host: &Host) -> Option<String> {
    let views = host.views();
    let active: Option<ViewId> = host.active_view();
    views
        .iter()
        .find(|view| Some(view.id) == active)
        .or_else(|| views.first())
        .and_then(|view| view.cell_breakpoint.clone())
}

/// The error of a command that needs `mode`.
pub fn not_in(mode: LayoutMode) -> CommandError {
    let key = match mode {
        LayoutMode::Horizontal => "layout.error.not_horizontal",
        LayoutMode::Canvas => "layout.error.not_canvas",
        _ => "layout.error.not_split",
    };
    CommandError::Failed(key.to_owned())
}

#[cfg(test)]
#[path = "actions_tests.rs"]
mod tests;
