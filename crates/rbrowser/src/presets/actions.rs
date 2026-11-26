//! What the commands and messages of environments, presets and focus mode do to the grid
//! (RB-010-12): they resolve a [`Selection`], apply its grid through the host, and write the
//! shell state (`environment`, `preset`, `mode`, `focus`).

use rb_commands::CommandError;
use rb_config::{LayoutMode, ProjectConfig, UserSettings, Zoom};
use rb_engine::{EngineError, Target, ViewHost, ViewId};

use rb_util::url::is_web_url;

use super::env::{frame_argb, initial_environment, next_environment, rewrite_url};
use super::layouts::{
    PresetError, Selection, fitted, next_preset, preset_at, resolve, start_selection,
};
use crate::control::Core;
use crate::host::Host;
use crate::host::state::layout_for;

/// The zoom of the cells rBrowser lays out itself: the user setting `grid.defaultZoom`
/// (Layout > Zoom).
pub fn default_zoom(user: &UserSettings) -> Zoom {
    user.grid.default_zoom
}

/// What a command or message failed with.
#[derive(Debug, thiserror::Error)]
pub enum ActionError {
    /// The selection or environment does not exist or does not apply.
    #[error(transparent)]
    Preset(#[from] PresetError),
    /// The host refused the grid or a URL.
    #[error(transparent)]
    Engine(#[from] EngineError),
}

impl From<ActionError> for CommandError {
    fn from(error: ActionError) -> Self {
        match error {
            ActionError::Preset(
                error @ (PresetError::UnknownPreset(_)
                | PresetError::UnknownEnvironment(_)
                | PresetError::UnknownBreakpoint(_)),
            ) => CommandError::InvalidArgs(error.to_string()),
            other => CommandError::Failed(other.to_string()),
        }
    }
}

type Done = Result<(), ActionError>;

/// The selection the shell state holds now.
pub fn current_selection(host: &Host) -> Selection {
    let shell = host.shell_state();
    Selection {
        preset: shell.preset,
        mode: shell.mode,
        focus: shell.focus,
    }
}

/// Shows `selection`: the grid is rebuilt (views whose cell keeps its breakpoint live on), a
/// newly opened view loads the page the grid showed, and the shell state follows. `fit` zooms
/// every cell to fit; `grid.fitToWindow` returns views zoomed by hand to their cell's zoom.
pub fn apply(core: &Core, selection: &Selection, fit: bool) -> Done {
    apply_with(core, selection, fit, true)
}

/// [`apply`]; without `rebuild` the grid is left alone when it is the one the host built from
/// the config (right after the project opened or its config changed).
fn apply_with(core: &Core, selection: &Selection, fit: bool, rebuild: bool) -> Done {
    let host = core.host();
    let config = host.config();
    let user = core.user_settings();
    let (layout, selection) = resolve(&config, selection, default_zoom(&user))?;
    let layout = if fit { fitted(&layout) } else { layout };
    let before: Vec<ViewId> = host.views().iter().map(|view| view.id).collect();
    let url = shown_url(host);
    if crate::layout::actions::is_mode(selection.mode) {
        // Horizontal, split and canvas place the views of the preset's grid (RB-080-04).
        crate::layout::actions::show(core, &selection, &layout)?;
    } else if rebuild
        || selection != top_level_grid()
        || fit
        || layout != layout_for(&config)
        || host.mode_spec().is_some()
    {
        host.set_layout(&layout)?;
    }
    if user.grid.fit_to_window || fit {
        host.zoom_fit_all();
    }
    if let Some(url) = url {
        for view in host
            .views()
            .iter()
            .filter(|view| !before.contains(&view.id))
        {
            host.navigate(Target::View(view.id), &url)?;
        }
    }
    host.update_shell_state(|shell| {
        if selection.mode != LayoutMode::Focus {
            shell.focus_return = None;
        }
        shell.preset = selection.preset;
        shell.mode = selection.mode;
        shell.focus = selection.focus;
    });
    Ok(())
}

/// The top-level layout in the grid: the selection whose grid the host builds from the config
/// itself, so showing it again rebuilds nothing (no longer the default selection, which is the
/// canvas, RB-099-26).
fn top_level_grid() -> Selection {
    Selection {
        mode: LayoutMode::Grid,
        ..Selection::default()
    }
}

/// `layout.select_preset` (`id`, or the next preset in config order) and the IPC message.
pub fn select_preset(core: &Core, id: Option<&str>) -> Done {
    let config = core.host().config();
    let current = core.host().shell_state().preset;
    let id = match id {
        Some(id) => id,
        None => next_preset(&config, current.as_deref()).ok_or(PresetError::NoPreset)?,
    };
    choose_preset(core, id)
}

/// `layout.preset.<number>`: the preset at that place in config order.
pub fn select_preset_number(core: &Core, number: usize) -> Done {
    let config = core.host().config();
    let id = preset_at(&config, number)
        .ok_or(PresetError::NoPreset)?
        .to_owned();
    choose_preset(core, &id)
}

/// A preset chosen by the person: its hidden selection comes first (RB-080-41, the config is
/// read again when it differs), then the preset is applied once over that config.
fn choose_preset(core: &Core, id: &str) -> Done {
    Selection::preset(&core.host().config(), id)?;
    crate::layout::view_edit::take_preset_selection(core, id)
        .map_err(|error| PresetError::Mode(error.to_string()))?;
    let selection = Selection::preset(&core.host().config(), id)?;
    apply(core, &selection, false)
}

/// `layout.focus_mode`: focus mode on (on `focus`, else the active view's breakpoint) or off.
/// `enabled` set to the current state changes nothing but the focused breakpoint.
pub fn toggle_focus(core: &Core, enabled: Option<bool>, focus: Option<&str>) -> Done {
    let host = core.host();
    let config = host.config();
    if let Some(focus) = focus
        && !config
            .breakpoints
            .iter()
            .any(|breakpoint| breakpoint.id == focus)
    {
        return Err(PresetError::UnknownBreakpoint(focus.to_owned()).into());
    }
    let current = current_selection(host);
    let on = current.mode == LayoutMode::Focus;
    let from = current.mode;
    let next = match enabled {
        Some(wanted) if wanted == on => Selection {
            focus: focus.map(str::to_owned).or(current.focus.clone()),
            ..current
        },
        _ => {
            let active = focus.map(str::to_owned).or_else(|| active_breakpoint(host));
            let mut next = current.toggled_focus(&config, active.as_deref());
            // Off returns to the mode focus mode was switched on from, over the same preset.
            if let Some(mode) = host.shell_state().focus_return
                && on
                && next.preset == current.preset
            {
                next.mode = mode;
            }
            next
        }
    };
    apply(core, &next, false)?;
    if !on && next.mode == LayoutMode::Focus {
        host.update_shell_state(|shell| shell.focus_return = Some(from));
    }
    Ok(())
}

/// `layout.focus_next` / `layout.focus_prev`. From the arrow keys (`from_arrow_key`) it does
/// nothing outside focus mode or when `focusMode.arrowKeys` is off.
pub fn step_focus(core: &Core, forward: bool, from_arrow_key: bool) -> Done {
    let host = core.host();
    let current = current_selection(host);
    if from_arrow_key
        && (current.mode != LayoutMode::Focus || !core.user_settings().focus_mode.arrow_keys)
    {
        return Ok(());
    }
    if current.mode == LayoutMode::Horizontal {
        // In horizontal mode the strip scrolls one view (RB-080-04).
        crate::layout::canvas_ops::scroll(core, forward)
            .map_err(|error| PresetError::Mode(error.to_string()))?;
        return Ok(());
    }
    let next = current.stepped(&host.config(), forward)?;
    apply(core, &next, false)
}

/// `layout.zoom_to_fit`: every cell of the current grid zoomed to fit, hand zoom undone.
pub fn zoom_to_fit(core: &Core) -> Done {
    // The canvas fits its camera to every view instead (RB-080-04).
    if crate::layout::canvas_ops::fit_canvas(core) {
        return Ok(());
    }
    apply(core, &current_selection(core.host()), true)
}

/// `environments.select` (`id`, or the next environment) and the IPC message: every view loads
/// its page in the new environment, and the views are framed in its colour.
pub fn select_environment(core: &Core, id: Option<&str>) -> Done {
    let host = core.host();
    let config = host.config();
    let current = host.shell_state().environment;
    let id = match id {
        Some(id) => id,
        None => next_environment(&config.environments, current.as_deref())
            .ok_or(PresetError::NoEnvironment)?,
    };
    let target = config
        .environments
        .iter()
        .find(|environment| environment.id == id)
        .ok_or_else(|| PresetError::UnknownEnvironment(id.to_owned()))?;
    // The check refuses such a config; this holds for one that reached the host without it.
    if !is_web_url(&target.url) {
        return Err(PresetError::EnvironmentUrl(target.id.clone()).into());
    }
    let from = current
        .as_deref()
        .and_then(|current| config.environments.iter().find(|e| e.id == current))
        .map(|environment| environment.url.as_str());
    for view in host.views() {
        let url = rewrite_url(&view.url, from, &target.url);
        host.navigate(Target::View(view.id), &url)?;
    }
    host.update_shell_state(|shell| shell.environment = Some(target.id.clone()));
    core.ui()
        .set_frame_color(frame_color(&config, Some(&target.id)));
    Ok(())
}

/// `environments.switch.<number>`.
pub fn select_environment_number(core: &Core, number: usize) -> Done {
    let config = core.host().config();
    let environment = number
        .checked_sub(1)
        .and_then(|index| config.environments.get(index))
        .ok_or(PresetError::NoEnvironment)?;
    select_environment(core, Some(&environment.id))
}

/// After the project opened or its config changed: the environment still in effect (else the
/// one the start URL belongs to) without loading anything, its frame, and the selection (else
/// `layouts.default`) applied again.
pub fn restore(core: &Core, opened: bool) -> Done {
    let host = core.host();
    let config = host.config();
    let current = host.shell_state().environment;
    let environment = current
        .as_deref()
        .filter(|id| config.environments.iter().any(|e| e.id == *id))
        .map(str::to_owned)
        .or_else(|| {
            initial_environment(&config.environments, config.start_url.as_deref())
                .map(|environment| environment.id.clone())
        });
    host.update_shell_state(|shell| shell.environment = environment.clone());
    core.ui()
        .set_frame_color(frame_color(&config, environment.as_deref()));
    let current = current_selection(host);
    let selection = match &current.preset {
        _ if opened => start_selection(&config),
        Some(id) if !config.layouts.presets.contains_key(id) => start_selection(&config),
        _ => current,
    };
    apply_with(core, &selection, false, false)
}

/// The frame colour of the environment `id`; `None` without one or for a malformed colour.
fn frame_color(config: &ProjectConfig, id: Option<&str>) -> Option<String> {
    let color = config
        .environments
        .iter()
        .find(|environment| Some(environment.id.as_str()) == id)?
        .color
        .clone()?;
    if frame_argb(&color).is_none() {
        tracing::warn!(%color, "an environment colour is not #rrggbb; the views get no frame");
        return None;
    }
    Some(color)
}

/// The breakpoint of the active view's cell (else the first view's).
fn active_breakpoint(host: &Host) -> Option<String> {
    let views = host.views();
    let active = host.active_view();
    views
        .iter()
        .find(|view| Some(view.id) == active)
        .or_else(|| views.first())
        .and_then(|view| view.cell_breakpoint.clone())
}

/// The web page the grid shows (the active view's, else the first view's).
fn shown_url(host: &Host) -> Option<String> {
    let views = host.views();
    let active = host.active_view();
    views
        .iter()
        .find(|view| Some(view.id) == active)
        .or_else(|| views.first())
        .map(|view| view.url.clone())
        .filter(|url| is_web_url(url))
}
