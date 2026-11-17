//! The views of a layout mode (RB-080-04, `crate::layout`) as overlay views of the main window
//! (ADR 0001, Decision 2: free placement through `Window::add_overlay_view`, docking mode
//! CUSTOM). Each overlay holds a cell panel - label strip over the view, as in the grid - at the
//! rectangle the core computed, cut to the grid area, so an overlay never covers the toolbar
//! strip or the DevTools dock; the core hides the views that do not fit (in the canvas only
//! those wholly outside). A browser view moves between the grid's boxes and an overlay without
//! reloading.
//!
//! In canvas mode the canvas board (`canvas_board.rs`) fills the grid panel behind the overlays
//! and draws the label strips, which get mouse events there: each overlay then holds only the
//! view, below its label's band - and its footer (RB-080-44), which needs no mouse.
//!
//! A canvas view that reaches past the grid area is shown cut to it (RB-080-55): the overlay
//! covers only the part inside the area ([`crate::layout::canvas::clip`]) and holds the cell
//! panel in a clip panel whose box layout reaches out of the overlay by what is cut off
//! (negative insets), so the page keeps its size and the parts outside are not drawn - the
//! overlay never covers the toolbar strip, the status bar or the DevTools dock. UI thread.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use cef::*;
use rb_engine::{CellRect, ViewId};

use super::grid::fill_cell;
use super::views::{UiState, sized_panel};
use crate::host::ui::{PlacedView, UiGrid};
use crate::layout::canvas::overlay_cut;

/// One placed view's overlay; `trim` DIP at the top of its place are the board's label strip.
struct Overlay {
    controller: OverlayController,
    trim: u32,
    /// The overlay's content: the cell panel, filling it within its insets.
    clip: Panel,
    /// The cut-off DIP (top, left, bottom, right) the clip panel's insets reach out by now.
    cut: Cell<Option<[u32; 4]>>,
}

thread_local! {
    /// The overlay of each placed view.
    static OVERLAYS: RefCell<HashMap<ViewId, Overlay>> = RefCell::new(HashMap::new());
}

/// Removes every overlay. The grid detached the browser views from them first, so only the
/// overlays' panels and label strips go.
pub fn clear() {
    OVERLAYS.with(|overlays| {
        for (_, overlay) in overlays.borrow_mut().drain() {
            if overlay.controller.is_valid() != 0 {
                overlay.controller.destroy();
            }
        }
    });
}

/// Puts the cells of `grid` into overlays at `placed`.
pub fn build(grid: &UiGrid, placed: &[PlacedView], ui: &mut UiState) {
    let Some(window) = ui.window.clone() else {
        tracing::error!("the main window does not exist");
        return;
    };
    // The canvas board behind the overlays draws the labels (RB-080-04).
    let board = is_canvas();
    if board && let Some(grid_panel) = &ui.grid {
        super::canvas_board::attach(grid_panel);
    }
    for place in placed {
        let Some(index) = grid.cells.iter().position(|cell| cell.view == place.view) else {
            continue;
        };
        let Some(panel) = sized_panel(0, 0, false) else {
            continue;
        };
        if fill_cell(&panel, index, grid, ui).is_none() {
            continue;
        }
        let trim = if board {
            // A placeholder's cell has no footer: only what `fill_cell` built counts.
            let footer = crate::host::footer::cell_has_strip(grid, index);
            without_label(&panel, grid.label_height, footer)
        } else {
            0
        };
        let Some(clip) = sized_panel(0, 0, false) else {
            continue;
        };
        // One layout for the overlay's life; a cut changes only its insets (`reach_out`).
        clip.set_to_fill_layout();
        let mut cell = View::from(&panel);
        clip.add_child_view(Some(&mut cell));
        let mut view = View::from(&clip);
        // Activatable: the page in it takes the keyboard.
        match window.add_overlay_view(Some(&mut view), DockingMode::CUSTOM, 1) {
            Some(controller) => {
                let overlay = Overlay {
                    controller,
                    trim,
                    clip,
                    cut: Cell::new(None),
                };
                // The area the places were computed for (RB-080-55).
                apply(&overlay, place, grid.area);
                OVERLAYS.with(|overlays| overlays.borrow_mut().insert(place.view, overlay));
            }
            None => tracing::warn!(view = %place.view, "the view's overlay was not created"),
        }
    }
}

/// Moves the overlays to `placed` (scroll, pan, a new grid area); the external views' windows
/// follow (RB-080-39).
pub fn place(area: CellRect, placed: &[PlacedView]) {
    OVERLAYS.with(|overlays| {
        let overlays = overlays.borrow();
        for place in placed {
            if let Some(overlay) = overlays.get(&place.view) {
                apply(overlay, place, Some(area));
            }
        }
    });
    super::embed::place_all();
}

fn apply(overlay: &Overlay, place: &PlacedView, area: Option<CellRect>) {
    let controller = &overlay.controller;
    if controller.is_valid() == 0 {
        return;
    }
    // Cut to the grid area, below a label the board holds at its top (RB-080-55).
    let cut = overlay_cut(place.outer, overlay.trim, area).filter(|_| place.visible);
    // A hidden overlay is neither moved nor laid out: nothing to do per frame of a gesture.
    if let Some(cut) = cut {
        let bounds = cut.bounds;
        controller.set_bounds(Some(&Rect {
            x: bounds.x,
            y: bounds.y,
            width: i32::try_from(bounds.width).unwrap_or(0),
            height: i32::try_from(bounds.height).unwrap_or(0),
        }));
        reach_out(overlay, [cut.top, cut.left, cut.bottom, cut.right]);
    }
    // Shown only when it was hidden: showing an overlay that can take the focus activates it,
    // which took the focus from the canvas board in the middle of a drag (the board cancelled it).
    let shown = cut.is_some();
    if (controller.is_visible() != 0) != shown {
        controller.set_visible(i32::from(shown));
    }
}

/// Lets the cell panel reach `cut` DIP (top, left, bottom, right) past the clip panel's edges,
/// where it is cut off: the clip panel's fill layout within negative insets. Only when the cut
/// changed; the cell keeps its size during a pan, so only its position changes.
fn reach_out(overlay: &Overlay, cut: [u32; 4]) {
    if overlay.cut.get() == Some(cut) {
        return;
    }
    overlay.cut.set(Some(cut));
    let out = |dip: u32| -i32::try_from(dip).unwrap_or(i32::MAX);
    overlay.clip.set_insets(Some(&Insets {
        top: out(cut[0]),
        left: out(cut[1]),
        bottom: out(cut[2]),
        right: out(cut[3]),
    }));
    overlay.clip.layout();
}

/// Whether the mode in effect is the canvas.
fn is_canvas() -> bool {
    super::views::host()
        .and_then(|host| host.mode_spec())
        .is_some_and(|spec| spec.mode() == rb_config::LayoutMode::Canvas)
}

/// Takes the label strip out of a cell panel `fill_cell` filled (strip first, then the view or
/// its placeholder, then its footer if any): the board draws it. Answers the DIP the overlay
/// leaves to the board.
fn without_label(panel: &Panel, label_height: u32, footer: bool) -> u32 {
    // Without its label strip the panel's first child is the view: that one stays.
    if panel.child_view_count() < 2 + usize::from(footer) {
        return 0;
    }
    match panel.child_view_at(0) {
        Some(mut strip) => {
            panel.remove_child_view(Some(&mut strip));
            label_height
        }
        None => 0,
    }
}
