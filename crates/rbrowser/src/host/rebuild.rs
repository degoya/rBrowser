//! The state half of a grid rebuild: computes the new order, cells and views on a
//! [`HostState`] without side effects, so a caller can run it on a copy and commit the copy only
//! when it succeeds (RB-020-07 review: a failed move leaves host and grid as they were). The
//! breakpoints of the hidden selection (`layout.hidden`, RB-080-41) get no view: an explicit
//! grid keeps their slots as empty cells, a computed one flows without them (`layout_for`).

use rb_config::{Layout, ProjectConfig};
use rb_engine::{CellRect, EngineError, ViewId, compute_grid};

use super::nav::marker_url;
use super::state::{HostState, ViewState, emulation_for, match_views};
use super::ui::UiCell;
use super::{LABEL_HEIGHT, cell_label};
use crate::layout_plan::{self, PlanNode};

/// What a rebuild changed, for the effects once the state is committed.
pub(super) struct Rebuilt {
    /// The view of each cell.
    pub order: Vec<ViewId>,
    pub plan: PlanNode,
    pub cells: Vec<UiCell>,
    /// Kept views with a CDP session: their metrics may have changed.
    pub kept_views: Vec<ViewId>,
    pub closed: Vec<ViewId>,
    pub opened: Vec<ViewId>,
}

/// Rebuilds `state` for `layout`. On an error `state` may be half changed: run it on a copy.
pub(super) fn rebuild_state(state: &mut HostState, full: &Layout) -> Result<Rebuilt, EngineError> {
    // Validates the cells against the grid before anything changes.
    compute_grid(full, CellRect::default(), LABEL_HEIGHT)?;
    for (index, cell) in full.cells.iter().enumerate() {
        if state.breakpoint(&cell.breakpoint).is_none() {
            return Err(EngineError::Invalid(format!(
                "layout.cells[{index}] names the unknown breakpoint {}",
                cell.breakpoint
            )));
        }
    }
    // The hidden breakpoints (RB-080-41) keep their slots as empty cells; only the others get
    // a view.
    let hidden = rb_config::hidden_selection_in(&state.config, full);
    let plan = layout_plan::plan_hiding(full, &hidden)?;
    let shown = layout_plan::visible_cells(full, &hidden);
    let layout = &shown;
    let existing: Vec<(ViewId, Option<String>)> = state
        .order
        .iter()
        .filter_map(|id| state.views.get(id))
        .map(|entry| (entry.id, entry.cell_breakpoint.clone()))
        .collect();
    let wanted: Vec<&str> = layout
        .cells
        .iter()
        .map(|cell| cell.breakpoint.as_str())
        .collect();
    let matched = match_views(&existing, &wanted);
    let start_url = state
        .config
        .start_url
        .clone()
        .unwrap_or_else(|| "about:blank".to_owned());
    let defaults = state.config.emulation.clone();
    let (mut order, mut opened, mut cells) = (Vec::new(), Vec::new(), Vec::new());
    let mut kept_views = Vec::new();
    for (index, (cell, kept)) in layout.cells.iter().zip(matched).enumerate() {
        let id = match kept {
            Some(id) => {
                // A kept view that shows its cell's breakpoint follows a changed definition.
                let fresh = state.breakpoint(&cell.breakpoint).cloned();
                if let (Some(fresh), Some(entry)) = (fresh, state.views.get_mut(&id))
                    && entry
                        .breakpoint
                        .as_ref()
                        .is_some_and(|shown| shown.id == fresh.id && *shown != fresh)
                {
                    entry.emulation = emulation_for(&fresh, &defaults);
                    entry.breakpoint = Some(fresh);
                }
                // A view not attached yet gets its metrics from the attach.
                if state.views.get(&id).is_some_and(|e| e.session.is_some()) {
                    kept_views.push(id);
                }
                id
            }
            None => {
                let id = state.allocate_id();
                let breakpoint = state
                    .breakpoint(&cell.breakpoint)
                    .cloned()
                    .ok_or_else(|| EngineError::Invalid(cell.breakpoint.clone()))?;
                let mut entry =
                    ViewState::new(id, &breakpoint, emulation_for(&breakpoint, &defaults));
                entry.pending_url = Some(start_url.clone());
                state.views.insert(id, entry);
                opened.push(id);
                id
            }
        };
        if let Some(entry) = state.views.get_mut(&id) {
            entry.cell_index = Some(index);
            entry.zoom = cell.zoom;
            entry.cell_breakpoint = Some(cell.breakpoint.clone());
        }
        order.push(id);
        cells.push(match state.views.get(&id) {
            Some(entry) => ui_cell(&state.config, entry),
            None => UiCell {
                view: id,
                label: String::new(),
                initial_url: marker_url(id),
                badge: crate::sessions::badge(&state.config, rb_config::DEFAULT_SESSION),
                session: rb_config::DEFAULT_SESSION.to_owned(),
                footer: None,
            },
        });
    }
    let closed: Vec<ViewId> = state
        .order
        .iter()
        .copied()
        .filter(|id| !order.contains(id))
        .collect();
    for id in &closed {
        state.views.remove(id);
    }
    if state.active.is_some_and(|active| closed.contains(&active)) {
        state.active = None;
    }
    state.order = order.clone();
    state.layout = full.clone();
    state.hidden = hidden;
    if let Some(area) = state.grid_area {
        state.cells = super::footer::grid_cells(state, layout, area);
    }
    Ok(Rebuilt {
        order,
        plan,
        cells,
        kept_views,
        closed,
        opened,
    })
}

/// The window's cell of `entry`: its label strip, its marker URL and session (also used when the
/// grid is sent again without a rebuild, RB-080-14).
pub(super) fn ui_cell(config: &ProjectConfig, entry: &ViewState) -> UiCell {
    let session = entry.context_session().to_owned();
    UiCell {
        view: entry.id,
        label: cell_label(entry),
        initial_url: marker_url(entry.id),
        badge: crate::sessions::badge(config, &session),
        session,
        // The host adds it when it sends the grid (`Host::with_footers`).
        footer: None,
    }
}

#[cfg(test)]
#[path = "rebuild_tests.rs"]
mod tests;
