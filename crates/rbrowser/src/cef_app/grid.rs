//! Builds the grid from its plan as nested BoxLayouts (ADR 0001, Decision 2); in a layout mode
//! (RB-080-04) the cells go to `placed.rs` instead. A view in a window of its own (RB-080-14,
//! `pop_out.rs`) stays there: its cell shows a placeholder, or is hidden so the others close the
//! gap. The cell of a hidden breakpoint (RB-080-41) stays empty under a label strip that opens
//! the add menu. With `ui.viewFooter` a cell ends in its view's footer (RB-080-44,
//! `cell_footers.rs`); an empty cell and a placeholder have none. UI thread.

use std::collections::HashSet;

use cef::*;
use rb_engine::ViewId;

use super::views::{UI, UiState, browser_view, cell_label, sized_panel};
use crate::host::ui::UiGrid;
use crate::layout_plan::{NodeKind, PlanNode};

/// Replaces the grid: kept views are moved into their new cells (their browsers live on), new
/// ones are created, dropped ones are closed.
pub fn apply(grid: &UiGrid) {
    UI.with(|ui| {
        let mut ui = ui.borrow_mut();
        let Some(panel) = ui.grid.clone() else {
            tracing::error!("the grid panel does not exist");
            return;
        };
        let keep: HashSet<ViewId> = grid.cells.iter().map(|cell| cell.view).collect();
        for (id, view) in &ui.views {
            // A view in a window of its own stays in it.
            if grid.detached(*id).is_none() {
                detach(&View::from(view));
            }
        }
        let dropped: Vec<ViewId> = ui
            .views
            .keys()
            .copied()
            .filter(|id| !keep.contains(id))
            .collect();
        for id in dropped {
            let Some(view) = ui.views.remove(&id) else {
                continue;
            };
            // A popped-out view's window closes without it (RB-080-14).
            super::pop_out::close_without(id, &view);
            // The view is out of the hierarchy now; releasing its last reference destroys the
            // browser (`~CefBrowserViewImpl` calls `WindowDestroyed()`). No `close_browser`:
            // that would ask DoClose, and a pending unload handler would leave the browser
            // half closed once the view is gone (RB-010-12, focus mode quit the app).
            release(view);
        }
        panel.remove_all_child_views();
        // The strips are built anew below: the badges of the old ones go (RB-080-33).
        super::engine_badge::clear();
        super::page_zoom::clear();
        super::cell_info::clear();
        super::cell_labels::clear_hidden();
        super::cell_footers::clear();
        // A layout mode places its views as overlays over the empty grid panel (RB-080-04).
        super::placed::clear();
        if let Some(placed) = &grid.placed {
            super::placed::build(grid, placed, &mut ui);
            panel.layout();
            return;
        }
        let inset = i32::try_from(grid.gap).unwrap_or(0);
        let root_box = panel.set_to_box_layout(Some(&BoxLayoutSettings {
            horizontal: 1,
            inside_border_insets: Insets {
                top: inset,
                left: inset,
                bottom: inset,
                right: inset,
            },
            cross_axis_alignment: AxisAlignment::STRETCH,
            ..Default::default()
        }));
        if let Some(mut root) = build(&grid.plan, grid, &mut ui) {
            panel.add_child_view(Some(&mut root));
            if let Some(root_box) = &root_box {
                root_box.set_flex_for_view(Some(&mut root), 1);
            }
        }
        panel.layout();
    });
    // The external views' windows over their new cells (RB-080-39), after the borrow of `UI`
    // ends: placing reads it.
    super::embed::place_all();
}

/// Drops a browser view taken out of the hierarchy: with its last reference CEF destroys the
/// browser. Shared with the DevTools dock (`devtools.rs`).
pub(super) fn release(view: BrowserView) {
    if View::from(&view).parent_view().is_some() {
        tracing::warn!("a released browser view is still in the hierarchy");
    }
    drop(view);
}

/// Removes `view` from its parent panel (or window), so it can be added to a new one.
pub(super) fn detach(view: &View) {
    if let Some(parent) = view.parent_view().and_then(|parent| parent.as_panel()) {
        let mut view = view.clone();
        parent.remove_child_view(Some(&mut view));
    }
}

/// Fills `panel` with cell `index`: its label strip over its view (kept, or created in its
/// session) over its footer, when the grid has footers. Shared with the overlays of the layout
/// modes (`placed.rs`).
pub(super) fn fill_cell(
    panel: &Panel,
    index: usize,
    grid: &UiGrid,
    ui: &mut UiState,
) -> Option<()> {
    let cell = grid.cells.get(index)?;
    let layout = panel.set_to_box_layout(Some(&BoxLayoutSettings {
        horizontal: 0,
        cross_axis_alignment: AxisAlignment::STRETCH,
        ..Default::default()
    }));
    if let Some(label) = cell_label(cell.view, &cell.label, grid.label_height) {
        super::cell_labels::remember(cell.view, &label);
        // The session badge beside the label, for a named session (RB-020-07).
        let label =
            crate::sessions::cef::label_strip(&label, cell.badge.as_ref(), grid.label_height);
        // The address, the Web Vitals and the external engine's window (RB-090-27).
        let label = super::cell_info::with_info(label, cell.view, grid.label_height);
        // The page zoom's badge, shown while the page is zoomed (RB-090-20).
        let label = super::page_zoom::with_badge(label, cell.view, grid.label_height);
        // The engine badge at the strip's right end (RB-080-33).
        let mut label = super::engine_badge::with_badge(label, cell.view, grid.label_height);
        panel.add_child_view(Some(&mut label));
    }
    // A view in a window of its own (RB-080-14): the placeholder in its place.
    if grid.detached(cell.view).is_some() {
        if let Some(mut placeholder) = super::pop_out::placeholder(cell.view) {
            panel.add_child_view(Some(&mut placeholder));
            if let Some(layout) = &layout {
                layout.set_flex_for_view(Some(&mut placeholder), 1);
            }
        }
        return Some(());
    }
    let browser = match ui.views.get(&cell.view) {
        Some(existing) => Some(existing.clone()),
        None => {
            // A new view lives in its session's request context (RB-020-07).
            let context = crate::sessions::cef::request_context(&cell.session);
            let created = browser_view(Some(cell.view), &cell.initial_url, context);
            if let Some(created) = &created {
                ui.views.insert(cell.view, created.clone());
            }
            created
        }
    };
    if let Some(browser) = browser {
        let mut browser = View::from(&browser);
        panel.add_child_view(Some(&mut browser));
        if let Some(layout) = &layout {
            layout.set_flex_for_view(Some(&mut browser), 1);
        }
    }
    // The footer takes its height from the view above it (RB-080-44).
    if crate::host::footer::cell_has_strip(grid, index)
        && let Some(mut footer) = footer_of(cell, grid)
    {
        panel.add_child_view(Some(&mut footer));
    }
    Some(())
}

/// The footer strip of `cell`; `None` for a view without a footer.
fn footer_of(cell: &crate::host::ui::UiCell, grid: &UiGrid) -> Option<View> {
    super::cell_footers::grid_strip(cell.view, cell.footer.as_ref()?, grid.footer_height)
}

fn build(node: &PlanNode, grid: &UiGrid, ui: &mut UiState) -> Option<View> {
    let (width, height) = node.preferred(grid.gap, grid.label_height);
    let panel = sized_panel(width, height, false)?;
    match &node.kind {
        NodeKind::Spacer => {}
        // The slot of a hidden breakpoint (RB-080-41): its label strip offers to open it again.
        NodeKind::Hidden(breakpoint) => {
            panel.set_to_box_layout(Some(&BoxLayoutSettings {
                horizontal: 0,
                cross_axis_alignment: AxisAlignment::STRETCH,
                ..Default::default()
            }));
            if let Some(label) = super::cell_labels::hidden_label(breakpoint, grid.label_height) {
                let mut label = View::from(&label);
                panel.add_child_view(Some(&mut label));
            }
        }
        NodeKind::Cell(index) => {
            fill_cell(&panel, *index, grid, ui)?;
            // Without a placeholder the cell is hidden: the box layout skips a hidden child, so
            // the other views close the gap (RB-080-14).
            let hidden = grid
                .cells
                .get(*index)
                .and_then(|cell| grid.detached(cell.view))
                .is_some_and(|detached| !detached.placeholder);
            if hidden {
                panel.set_visible(0);
            }
        }
        NodeKind::Split {
            horizontal,
            children,
        } => {
            let layout = panel.set_to_box_layout(Some(&BoxLayoutSettings {
                horizontal: i32::from(*horizontal),
                between_child_spacing: i32::try_from(grid.gap).unwrap_or(0),
                cross_axis_alignment: AxisAlignment::STRETCH,
                ..Default::default()
            }));
            for child in children {
                if let Some(mut built) = build(child, grid, ui) {
                    panel.add_child_view(Some(&mut built));
                    if let Some(layout) = &layout {
                        let flex = i32::try_from(child.flex(*horizontal)).unwrap_or(1);
                        layout.set_flex_for_view(Some(&mut built), flex);
                    }
                }
            }
        }
    }
    Some(View::from(&panel))
}
