//! The host's half of the layout modes (RB-080-04, `crate::layout`): the spec in effect, the
//! views rebuilt for its breakpoints (a kept view keeps its browser, as for a preset), and their
//! places - computed from the grid area by the mode's pure function and sent to the window,
//! which puts the views there as overlay views. A view outside the area is marked off screen,
//! which the throttling treats like a view without area (RB-020-09).

use rb_config::Zoom;
use rb_engine::{CellRect, EngineError, GridCell, ViewEvent, ViewId};

use super::state::HostState;
use super::ui::PlacedView;
use super::{Host, LABEL_HEIGHT, rebuild};
use crate::layout::{Item, ModeSpec, Slot};

impl Host {
    /// Shows `spec`: the views of its breakpoints (kept views live on, the others close or
    /// open) at the places its mode gives them. Answers the view of each breakpoint, in order.
    pub fn set_mode(&self, spec: ModeSpec) -> Result<Vec<ViewId>, EngineError> {
        let layout = spec.grid();
        let mut state = self.state();
        let mut next = state.clone();
        let rebuilt = rebuild::rebuild_state(&mut next, &layout)?;
        next.mode = Some(spec);
        *state = next;
        drop(state);
        Ok(self.finish_rebuild(&layout, rebuilt))
    }

    /// The mode spec in effect; `None` in grid and focus mode.
    pub fn mode_spec(&self) -> Option<ModeSpec> {
        self.state().mode.clone()
    }

    /// Changes the spec in effect without rebuilding (scroll, pan, the divider) and places the
    /// views again. `false` without a mode.
    pub fn update_mode(&self, change: impl FnOnce(&mut ModeSpec)) -> bool {
        let placement = {
            let mut state = self.state();
            let Some(spec) = state.mode.as_mut() else {
                return false;
            };
            change(spec);
            place(&mut state)
        };
        if let Some(placement) = placement {
            self.ui.place_views(placement.area, placement.placed);
            for view in placement.moved {
                self.spawn_apply(view, false);
            }
        }
        self.rethrottle();
        self.emit(ViewEvent::LayoutChanged);
        true
    }

    /// A gesture shows the views at `placed` for now (a drag, a pan, a zoom, RB-080-50): only
    /// the window moves them, nothing is kept. A view that comes into the area runs at once -
    /// it is paused again only when the gesture ends and the views are placed for good
    /// (RB-080-55).
    pub fn place_live(&self, area: CellRect, placed: Vec<PlacedView>) {
        let woken = {
            let mut state = self.state();
            let mut woken = false;
            for place in placed.iter().filter(|place| place.visible) {
                if let Some(entry) = state.views.get_mut(&place.view)
                    && entry.detached.is_none()
                    && entry.off_screen
                {
                    entry.off_screen = false;
                    woken = true;
                }
            }
            woken
        };
        self.ui.place_views(area, placed);
        if woken {
            self.rethrottle();
        }
    }

    /// The grid area as the window laid it out; `None` before it did.
    pub fn grid_area(&self) -> Option<CellRect> {
        self.state().grid_area
    }

    /// The breakpoint size of each view of the spec in effect, in its order.
    pub fn mode_sizes(&self) -> Vec<Item> {
        sizes(&self.state())
    }

    /// `view` gets new metrics: when a layout mode is in effect and its emulated size is not
    /// the one it was placed at - whichever path wrote it (a breakpoint, a free size, a device,
    /// the orientation) - the views are placed again first: the canvas board's frame, the
    /// overlay and the metrics follow at once (RB-080-54). Answers the views whose rectangle
    /// changed; none without a mode or a new size.
    pub(super) fn place_if_resized(&self, view: ViewId) -> Vec<ViewId> {
        let placement = {
            let mut state = self.state();
            if state.mode.is_none() {
                return Vec::new();
            }
            let Some(entry) = state.views.get(&view) else {
                return Vec::new();
            };
            let size = (entry.emulation.width, entry.emulation.height);
            if entry.placed_size.is_none_or(|placed| placed == size) {
                return Vec::new();
            }
            tracing::info!(
                %view,
                from = ?entry.placed_size,
                to = ?size,
                area = ?state.grid_area,
                "a view of the layout mode changed its size: placed again"
            );
            place(&mut state)
        };
        let Some(placement) = placement else {
            return Vec::new();
        };
        self.ui.place_views(placement.area, placement.placed);
        self.rethrottle();
        self.emit(ViewEvent::LayoutChanged);
        placement.moved
    }

    /// The places of the views for the spec in effect and the area they were computed for;
    /// `None` without a mode or before the window laid the area out. Updates the cells and the
    /// off-screen marks.
    pub(super) fn placed_views(&self) -> Option<(CellRect, Vec<PlacedView>)> {
        place(&mut self.state()).map(|placement| (placement.area, placement.placed))
    }
}

/// What [`place`] computed: the area, where the window puts each view, and the views whose
/// rectangle changed.
pub(super) struct Placement {
    /// The grid area the places were computed for.
    pub area: CellRect,
    /// Where each view goes.
    pub placed: Vec<PlacedView>,
    /// The views whose rectangle changed.
    pub moved: Vec<ViewId>,
}

/// Computes the slots of the spec in `state` for its grid area, writes the cells, the off-screen
/// marks and the shown views' rectangles (the window puts them exactly there; its own report
/// follows), and returns what the window places and the views whose rectangle changed.
pub(super) fn place(state: &mut HostState) -> Option<Placement> {
    let spec = state.mode.clone()?;
    let area = state.grid_area?;
    let sizes = sizes(state);
    let slots = spec.arrange(&sizes, area, LABEL_HEIGHT);
    // A view gives its footer the bottom of its rectangle (RB-080-44); the place stays.
    let mut placed = Vec::with_capacity(slots.len());
    let mut cells = Vec::with_capacity(slots.len());
    let mut moved = Vec::new();
    for (index, (view, slot)) in state.order.clone().into_iter().zip(&slots).enumerate() {
        let outer = slot.outer();
        let slot = &Slot {
            rect: crate::view_footer::above_footer(slot.rect, state.footer.height_of(view)),
            ..*slot
        };
        // A view in a window of its own keeps the window's rectangle (RB-080-14).
        if let Some(entry) = state.views.get_mut(&view) {
            entry.placed_size = Some((entry.emulation.width, entry.emulation.height));
        }
        if let Some(entry) = state.views.get_mut(&view)
            && entry.detached.is_none()
        {
            if entry.off_screen == slot.visible {
                // One line per change, for the owner's diagnostics (RB-080-55).
                tracing::info!(
                    %view,
                    shown = slot.visible,
                    ?outer,
                    ?area,
                    "a view of the layout mode was shown or hidden"
                );
            }
            entry.off_screen = !slot.visible;
            if slot.visible && entry.measured != Some(slot.rect) {
                entry.measured = Some(slot.rect);
                moved.push(view);
            }
        }
        cells.push(cell(&spec, index, slot));
        placed.push(PlacedView {
            view,
            outer,
            visible: slot.visible,
        });
    }
    state.cells = cells;
    Some(Placement {
        area,
        placed,
        moved,
    })
}

/// The size of each view in grid order: its emulated viewport.
pub(super) fn sizes(state: &HostState) -> Vec<Item> {
    state
        .order
        .iter()
        .filter_map(|id| state.views.get(id))
        .map(|entry| Item {
            breakpoint: entry.cell_breakpoint.clone().unwrap_or_default(),
            width: entry.emulation.width,
            height: entry.emulation.height,
        })
        .collect()
}

fn cell(spec: &ModeSpec, index: usize, slot: &Slot) -> GridCell {
    GridCell {
        breakpoint: spec.breakpoints.get(index).cloned().unwrap_or_default(),
        col: index as u32 + 1,
        row: 1,
        col_span: 1,
        row_span: 1,
        zoom: Zoom::Fit,
        rect: slot.rect,
        label: slot.label,
    }
}

#[cfg(test)]
#[path = "placed_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "placed_resize_tests.rs"]
mod resize_tests;
