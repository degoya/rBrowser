//! The selection against the grid in effect (RB-080-41): which hidden breakpoints have a cell
//! to open ([`addable`]), the grid that decides it ([`base_grid`]: the preset's own grid, else
//! the config's) and a chosen preset's `hidden` taken over ([`take_preset_selection`]).
//! `view_edit` re-exports the public ones.

use rb_commands::CommandError;
use rb_config::{Layout, ProjectConfig};

use super::view_edit::write_hidden;
use crate::control::Core;
use crate::host::Host;

/// A preset is being chosen (RB-080-41): its `hidden`, when it has one, becomes the selection -
/// written into `config.local.json` and read again (the caller applies the preset over that
/// config) - so the views it was saved with come back; a preset without, or with the selection
/// in effect (as a set), leaves it alone. `true` when the config was written.
pub fn take_preset_selection(core: &Core, preset: &str) -> Result<bool, CommandError> {
    let config = core.host().config();
    let Some(chosen) = config.layouts.presets.get(preset) else {
        return Ok(false);
    };
    let Some(wanted) = chosen.hidden.as_deref() else {
        return Ok(false);
    };
    let wanted = rb_config::known_hidden(&config, wanted);
    let current = rb_config::hidden_selection(&config);
    let same = wanted.len() == current.len() && wanted.iter().all(|id| current.contains(id));
    if same {
        return Ok(false);
    }
    let grid = chosen
        .layout
        .clone()
        .unwrap_or_else(|| config.layout.clone());
    if !rb_config::keeps_a_chromium_view_in(&config, &grid, &wanted) {
        return Err(CommandError::Failed(format!(
            "preset `{preset}` would hide every view"
        )));
    }
    write_hidden(core, &wanted)?;
    Ok(true)
}

/// The grid in effect's base: the grid of the preset shown when it has one of its own, else
/// the config's (explicit `layout.cells`, or computed when there are none).
pub fn base_grid(host: &Host) -> Layout {
    own_preset_grid(host).unwrap_or_else(|| host.config().layout)
}

/// [`base_grid`] of `config` with `preset` shown.
fn base_grid_in(config: &ProjectConfig, preset: Option<&str>) -> Layout {
    preset
        .and_then(|preset| config.layouts.presets.get(preset))
        .and_then(|chosen| chosen.layout.clone())
        .unwrap_or_else(|| config.layout.clone())
}

/// The grid of the preset shown, when it has one of its own.
pub(super) fn own_preset_grid(host: &Host) -> Option<Layout> {
    let preset = host.shell_state().preset?;
    host.config().layouts.presets.get(&preset)?.layout.clone()
}

/// Whether `grid` has a cell for `breakpoint`: a computed grid (no cells) has one for every
/// breakpoint.
pub(super) fn has_cell(grid: &Layout, breakpoint: &str) -> bool {
    grid.cells.is_empty() || grid.cells.iter().any(|cell| cell.breakpoint == breakpoint)
}

/// The selection in effect: `layout.hidden` against the grid in effect ([`base_grid`]), none
/// when it would leave that grid without a view.
pub fn selection(host: &Host) -> Vec<String> {
    rb_config::hidden_selection_in(&host.config(), &base_grid(host))
}

/// The hidden breakpoints with a cell in the grid in effect: what the add menus offer and
/// `view.add` opens.
pub fn addable(host: &Host) -> Vec<String> {
    addable_in(&host.config(), host.shell_state().preset.as_deref())
}

/// [`addable`] of `config` with `preset` shown, for a caller that read both already (the state
/// push, RB-094-10).
pub fn addable_in(config: &ProjectConfig, preset: Option<&str>) -> Vec<String> {
    let grid = base_grid_in(config, preset);
    rb_config::hidden_selection_in(config, &grid)
        .into_iter()
        .filter(|id| has_cell(&grid, id))
        .collect()
}
