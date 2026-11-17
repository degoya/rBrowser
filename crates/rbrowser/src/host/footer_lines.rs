//! The lines of a footer (RB-080-53, `crate::view_footer::wrap`): one, or two in a view too
//! narrow for one, and the height they take from the view. The width is the view's in the
//! layout in effect - its cell, its place in a layout mode, a popped-out view's window - which
//! in a zoomed-out cell is far below the page's. The footers of a row end flush: a footer takes
//! its height from its own view, so every cell keeps its bottom and only a narrow view's page
//! is lower.
//!
//! What it costs: a new grid fits its footers to its own cells before it is sent, so it is
//! built once. A layout change (a pan, a scroll, the divider, a new grid area) compares each
//! view's width with the one its footer was fitted to and fits again only where it differs,
//! from the text already written; a footer that gets or loses a line goes the way of one that
//! appears (`Host::footer_reshaped`): the grid again and new metrics for the views.
//!
//! The height counts too (RB-080-78): a footer takes at most half of its view's rectangle, so
//! in a low view - a canvas zoomed far out - the second line goes first, then the strip
//! (`CellFooter::folded`), and it comes back when the view is high enough again. The vitals bar
//! (RB-090-71) and the Lighthouse bar (RB-090-73) are lines of the same strip and share that
//! room; the Lighthouse bar's gauges name their categories as fully as its width holds.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};

use rb_config::{FooterAlign, ViewFooterSettings};
use rb_engine::{CellRect, ViewId, compute_grid};

use super::super::state::HostState;
use super::super::{Host, label_height};
use crate::view_footer::CellFooter;
use crate::view_footer::lighthouse::{Form, form_for};
use crate::view_footer::wrap::{Fitted, fit_lines, room_lines, share, text_width};

/// The first footer of two lines this run was logged.
static LOGGED: AtomicBool = AtomicBool::new(false);

/// The room of a view's footer: the strip's width and the view's height with the footer, in
/// DIP; no height where the footer takes none of the view's (a view's own window grows by it).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Room {
    /// The strip's width.
    pub width: u32,
    /// The view's rectangle with its footer; `None` for a view in a window of its own.
    pub height: Option<u32>,
}

/// What a footer was fitted to: the room, and its tooltip before a cut text led it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Fit {
    /// The room; `None` before the view was laid out.
    pub room: Option<Room>,
    /// The tooltip as the footer's text wrote it.
    pub tooltip: String,
}

/// Gives `footer` its lines for `fit` and its alignment; what the lines cannot show leads the
/// tooltip. The view's room is shared with the vitals bar (RB-090-71) and the Lighthouse bar
/// (RB-090-73, `wrap::share`): the breakpoint bar's first line, the vitals bar, the Lighthouse
/// bar, its second line. The first footer of two lines in a run is logged.
pub(super) fn shape(footer: &mut CellFooter, fit_to: &Fit, settings: &ViewFooterSettings) {
    let width = fit_to.room.map(|room| room.width);
    let height = fit_to.room.and_then(|room| room.height);
    let has_text = !footer.text.is_empty();
    let shared = share(
        room_lines(height),
        has_text,
        footer.vitals.is_some(),
        footer.lighthouse.is_some(),
    );
    let fitted = if has_text {
        fit_lines(&footer.text, width, shared.bar, settings.wrap)
    } else {
        Fitted::default()
    };
    footer.tooltip = if !fitted.cut {
        fit_to.tooltip.clone()
    } else if fit_to.tooltip.is_empty() {
        footer.text.clone()
    } else {
        format!("{}\n{}", footer.text, fit_to.tooltip)
    };
    if fitted.lines.len() > 1 && !LOGGED.swap(true, Ordering::Relaxed) {
        tracing::info!(
            chars = footer.text.chars().count(),
            estimated = text_width(&footer.text),
            available = ?width,
            cut = fitted.cut,
            "a view footer took two lines (the first this run)"
        );
    }
    footer.folded = fitted.lines.is_empty() && !shared.vitals && !shared.lighthouse;
    footer.vitals_shown = shared.vitals;
    footer.lighthouse_shown = shared.lighthouse;
    footer.lighthouse_form = footer
        .lighthouse
        .as_ref()
        .map_or(Form::default(), |bar| form_for(&bar.gauges, width));
    footer.lines = fitted.lines;
    footer.centered = settings.align == FooterAlign::Center;
}

/// The room of each view in `rects` (the cells in grid order, `footed`: the footer's height
/// already taken from them); a popped-out view's width is its window's, without a height.
fn rooms(state: &HostState, rects: &[CellRect], footed: bool) -> BTreeMap<ViewId, Room> {
    state
        .order
        .iter()
        .enumerate()
        .filter_map(|(index, view)| {
            let entry = state.views.get(view)?;
            let room = match entry.detached {
                Some(_) => entry.measured.map(|rect| Room {
                    width: rect.width,
                    height: None,
                }),
                None => rects.get(index).map(|rect| Room {
                    width: rect.width,
                    height: Some(if footed {
                        rect.height.saturating_add(state.footer.height_of(*view))
                    } else {
                        rect.height
                    }),
                }),
            };
            room.map(|room| (*view, room))
        })
        .collect()
}

/// The room of the views as they are laid out now: their computed cells or places, each with
/// the height its footer takes now.
pub(super) fn cell_widths(state: &HostState) -> BTreeMap<ViewId, Room> {
    let rects: Vec<CellRect> = state.cells.iter().map(|cell| cell.rect).collect();
    rooms(state, &rects, true)
}

/// The room of the views in the layout in effect, computed afresh: a new grid or mode is
/// fitted to its own cells before it is sent, not to the ones of the grid before it.
pub(super) fn layout_widths(state: &HostState) -> BTreeMap<ViewId, Room> {
    let rects: Vec<CellRect> = match (state.grid_area, &state.mode) {
        (None, _) => Vec::new(),
        (Some(area), None) => compute_grid(&state.visible_layout(), area, label_height())
            .unwrap_or_default()
            .into_iter()
            .map(|cell| cell.rect)
            .collect(),
        (Some(area), Some(spec)) => spec
            .arrange(&super::super::placed::sizes(state), area, label_height())
            .into_iter()
            .map(|slot| slot.rect)
            .collect(),
    };
    rooms(state, &rects, false)
}

/// Whether `before` and `after` take another height: a footer appeared, went, or got or lost a
/// line. Logged at debug level per view.
pub(super) fn reshapes(
    view: ViewId,
    before: Option<&CellFooter>,
    after: Option<&CellFooter>,
) -> bool {
    let (was, now) = (
        before.map(CellFooter::height),
        after.map(CellFooter::height),
    );
    if was.is_some() && now.is_some() && was != now {
        tracing::debug!(
            %view,
            lines = after.map_or(0, |footer| footer.lines.len()),
            "a view footer changed its number of lines"
        );
    }
    was != now
}

impl Host {
    /// The views were laid out again (a new grid area, a pan, a zoom, a scroll, a popped-out
    /// window resized): the footers of the views whose room changed are fitted again, from the
    /// text they show. Nothing is written for a view whose room stayed.
    pub(in crate::host) fn refit_footers(&self) {
        let changed = {
            let mut state = self.state();
            let widths = cell_widths(&state);
            let super::FooterState {
                settings,
                fits,
                sent,
                ..
            } = &mut state.footer;
            let Some(settings) = settings.as_ref() else {
                return;
            };
            let mut changed = Vec::new();
            let mut reshaped = false;
            for (view, fitted) in fits.iter_mut() {
                let room = widths.get(view).copied();
                if room == fitted.room {
                    continue;
                }
                fitted.room = room;
                let Some(sent) = sent.get_mut(view) else {
                    continue;
                };
                let mut next = sent.clone();
                shape(&mut next, fitted, settings);
                if next != *sent {
                    reshaped |= reshapes(*view, Some(sent), Some(&next));
                    *sent = next.clone();
                    changed.push((*view, next));
                }
            }
            if reshaped { None } else { Some(changed) }
        };
        match changed {
            Some(changed) => {
                for (view, footer) in changed {
                    self.ui.set_cell_footer(view, footer.shown().cloned());
                }
            }
            None => self.footer_reshaped(),
        }
    }
}
