//! Layout presets and focus mode (RB-010-12), without side effects: which preset, mode and
//! focused breakpoint are selected ([`Selection`], the fields of the shell state), and the grid
//! that selection shows ([`resolve`]).

use rb_config::{EngineKind, Layout, LayoutCell, LayoutMode, ProjectConfig, Zoom};

use crate::host::state::layout_for;

/// Why a preset, a focus step or an environment cannot be applied. The message is the i18n
/// key `presets.error.<kind>` the shell shows.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PresetError {
    /// No preset has this id.
    #[error("presets.error.unknown_preset")]
    UnknownPreset(String),
    /// No environment has this id.
    #[error("presets.error.unknown_environment")]
    UnknownEnvironment(String),
    /// No breakpoint has this id.
    #[error("presets.error.unknown_breakpoint")]
    UnknownBreakpoint(String),
    /// The config has no preset at this place.
    #[error("presets.error.no_preset")]
    NoPreset,
    /// The config has no environment at this place.
    #[error("presets.error.no_environment")]
    NoEnvironment,
    /// Focus mode needs a Chromium breakpoint.
    #[error("presets.error.no_breakpoints")]
    NoBreakpoints,
    /// The focus moves only in focus mode.
    #[error("presets.error.not_focus_mode")]
    NotInFocusMode,
    /// A layout mode's command failed (RB-080-04); the message is its key.
    #[error("{0}")]
    Mode(String),
    /// The environment's URL is no `http`/`https` URL: nothing is loaded.
    #[error("presets.error.environment_url")]
    EnvironmentUrl(String),
}

/// What the grid shows: a preset (or the top-level `layout`), its mode and, in focus mode, the
/// breakpoint shown. A grid preset with `mode: Focus` is focus mode over that preset: leaving
/// it returns to the preset's grid. The default is the top-level layout in the built-in mode,
/// the canvas (RB-099-26).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Selection {
    /// The preset; `None` for the top-level `layout`.
    pub preset: Option<String>,
    /// Grid or focus.
    pub mode: LayoutMode,
    /// Focus mode: the breakpoint shown.
    pub focus: Option<String>,
}

impl Selection {
    /// The preset `id` as configured (its mode and focus).
    pub fn preset(config: &ProjectConfig, id: &str) -> Result<Self, PresetError> {
        let preset = config
            .layouts
            .presets
            .get(id)
            .ok_or_else(|| PresetError::UnknownPreset(id.to_owned()))?;
        Ok(Self {
            preset: Some(id.to_owned()),
            mode: preset.mode,
            focus: preset.focus.clone(),
        })
    }

    /// Focus mode on (on `active`, the active view's breakpoint, when given) or off again: off
    /// returns to the preset when it is a grid preset, else to the top-level `layout` in the
    /// built-in mode.
    pub fn toggled_focus(&self, config: &ProjectConfig, active: Option<&str>) -> Self {
        match self.mode {
            LayoutMode::Focus => {
                // Back to the preset in its own mode, unless it is a focus preset.
                let back = self.preset.as_ref().and_then(|id| {
                    config
                        .layouts
                        .presets
                        .get(id.as_str())
                        .filter(|preset| preset.mode != LayoutMode::Focus)
                        .map(|preset| (id.clone(), preset.mode))
                });
                match back {
                    Some((id, mode)) => Self {
                        preset: Some(id),
                        mode,
                        focus: None,
                    },
                    None => Self::default(),
                }
            }
            _ => Self {
                preset: self.preset.clone(),
                mode: LayoutMode::Focus,
                focus: active.map(str::to_owned),
            },
        }
    }

    /// The next (`forward`) or previous breakpoint in focus mode, wrapping around.
    pub fn stepped(&self, config: &ProjectConfig, forward: bool) -> Result<Self, PresetError> {
        if self.mode != LayoutMode::Focus {
            return Err(PresetError::NotInFocusMode);
        }
        let candidates = focus_candidates(config);
        let count = candidates.len();
        if count == 0 {
            return Err(PresetError::NoBreakpoints);
        }
        let position = self
            .focus
            .as_deref()
            .and_then(|focus| candidates.iter().position(|id| *id == focus));
        let index = match (position, forward) {
            (None, _) => 0,
            (Some(at), true) => (at + 1) % count,
            (Some(at), false) => (at + count - 1) % count,
        };
        Ok(Self {
            focus: Some(candidates[index].to_owned()),
            ..self.clone()
        })
    }
}

/// The preset at `number` (1-based, config order): `layout.preset.<number>`.
pub fn preset_at(config: &ProjectConfig, number: usize) -> Option<&str> {
    number
        .checked_sub(1)
        .and_then(|index| config.layouts.presets.get_index(index))
        .map(|(id, _)| id.as_str())
}

/// The preset after `current` in config order, wrapping (`layout.select_preset` without an id).
pub fn next_preset<'a>(config: &'a ProjectConfig, current: Option<&str>) -> Option<&'a str> {
    let presets = &config.layouts.presets;
    let next = current
        .and_then(|current| presets.get_index_of(current))
        .map_or(0, |index| (index + 1) % presets.len().max(1));
    presets.get_index(next).map(|(id, _)| id.as_str())
}

/// The selection at start: `layouts.default` when it names a preset, else the top-level layout
/// in the built-in mode - the canvas, which places the views itself until they are moved
/// (RB-099-26).
pub fn start_selection(config: &ProjectConfig) -> Selection {
    config
        .layouts
        .default
        .as_deref()
        .and_then(|id| Selection::preset(config, id).ok())
        .unwrap_or_default()
}

/// The grid `selection` shows, and the selection with its focus filled in. Cells rBrowser lays
/// out itself (no `layout.cells` in the config, the focus view) get `default_zoom`; cells of
/// the config keep their own.
pub fn resolve(
    config: &ProjectConfig,
    selection: &Selection,
    default_zoom: Zoom,
) -> Result<(Layout, Selection), PresetError> {
    let preset = match &selection.preset {
        Some(id) => Some(
            config
                .layouts
                .presets
                .get(id)
                .ok_or_else(|| PresetError::UnknownPreset(id.clone()))?,
        ),
        None => None,
    };
    let grid = match preset.and_then(|preset| preset.layout.clone()) {
        Some(layout) => layout,
        None if config.layout.cells.is_empty() => with_zoom(layout_for(config), default_zoom),
        None => config.layout.clone(),
    };
    // Grid, and the base grid of horizontal, split and canvas (RB-080-04).
    if selection.mode != LayoutMode::Focus {
        let normalized = Selection {
            focus: None,
            ..selection.clone()
        };
        return Ok((grid, normalized));
    }
    let candidates = focus_candidates(config);
    let focus = selection
        .focus
        .as_deref()
        .filter(|focus| candidates.contains(focus))
        .or_else(|| candidates.first().copied())
        .ok_or(PresetError::NoBreakpoints)?
        .to_owned();
    let layout = Layout {
        columns: 1,
        rows: 1,
        gap: grid.gap,
        cells: vec![LayoutCell {
            breakpoint: focus.clone(),
            col: 1,
            row: 1,
            col_span: 1,
            row_span: 1,
            zoom: default_zoom,
        }],
        ..Default::default()
    };
    let normalized = Selection {
        focus: Some(focus),
        ..selection.clone()
    };
    Ok((layout, normalized))
}

/// `layout` with every cell zoomed to fit (`layout.zoom_to_fit`).
pub fn fitted(layout: &Layout) -> Layout {
    with_zoom(layout.clone(), Zoom::Fit)
}

/// The breakpoints focus mode switches between: the Chromium ones not hidden (RB-080-41), in
/// config order.
fn focus_candidates(config: &ProjectConfig) -> Vec<&str> {
    let hidden = rb_config::hidden_selection(config);
    config
        .breakpoints
        .iter()
        .filter(|breakpoint| {
            breakpoint.engine == EngineKind::Chromium && !hidden.contains(&breakpoint.id)
        })
        .map(|breakpoint| breakpoint.id.as_str())
        .collect()
}

fn with_zoom(mut layout: Layout, zoom: Zoom) -> Layout {
    for cell in &mut layout.cells {
        cell.zoom = zoom;
    }
    layout
}

#[cfg(test)]
#[path = "layouts_tests.rs"]
mod tests;
