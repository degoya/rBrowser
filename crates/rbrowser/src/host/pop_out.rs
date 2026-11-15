//! The host's half of popping a view out of the grid (RB-080-14): the view keeps its id, browser,
//! session, emulation and sync and moves into a window of its own at its real size; its cell
//! shows a placeholder or closes the gap. The state says which views are out
//! ([`super::state::Detached`]); every grid the window builds carries them
//! ([`UiGrid::detached`]), so a rebuild (a config reload, a preset) leaves them in their windows.
//! Nothing of it is kept across a restart: every view starts in the grid.
//!
//! The device metrics follow the window's client area at zoom 1 (`crate::pop_out::metrics`); the
//! cell's zoom holds again once the view is back.

use rb_config::EngineKind;
use rb_engine::{CellRect, EngineError, ViewEvent, ViewId};

use super::state::Detached;
use super::ui::{DetachedCell, PopOutWindow, ScreenRect, UiGrid};
use super::{Host, LABEL_HEIGHT, rebuild};

/// How a view pops out: the settings `popOut.*` and the remembered bounds of its breakpoint.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PopOutOptions {
    /// `popOut.keepPlaceholder`.
    pub keep_placeholder: bool,
    /// `popOut.fitToScreen`.
    pub fit_to_screen: bool,
    /// The client area of the breakpoint's window last time (`popOut.rememberBounds`).
    pub remembered: Option<ScreenRect>,
}

impl Host {
    /// Pops `view` out into a window of its own; `false` when it is out already, and its window
    /// comes to the front instead. Firefox and WebKit views are windows of their own already.
    pub fn pop_out(&self, view: ViewId, options: PopOutOptions) -> Result<bool, EngineError> {
        let window = {
            let mut state = self.state();
            let entry = state
                .views
                .get(&view)
                .ok_or(EngineError::UnknownView(view))?;
            let engine = entry
                .breakpoint
                .as_ref()
                .map_or(EngineKind::Chromium, |breakpoint| breakpoint.engine);
            if engine != EngineKind::Chromium {
                return Err(EngineError::Invalid(format!(
                    "view {view} is a window of its own already"
                )));
            }
            if entry.detached.is_some() {
                drop(state);
                self.ui.raise_pop_out(view);
                return Ok(false);
            }
            let label = state.info(entry).label;
            let (width, height) = (entry.emulation.width.max(1), entry.emulation.height.max(1));
            if let Some(entry) = state.views.get_mut(&view) {
                entry.detached = Some(Detached {
                    placeholder: options.keep_placeholder,
                });
                // The real size until the window reports its client area.
                entry.measured = Some(CellRect {
                    x: 0,
                    y: 0,
                    width,
                    height,
                });
            }
            PopOutWindow {
                view,
                label,
                width,
                height,
                remembered: options.remembered,
                fit_to_screen: options.fit_to_screen,
                footer: state
                    .footer
                    .sent
                    .get(&view)
                    .filter(|footer| !footer.folded)
                    .cloned(),
            }
        };
        // The window takes the view first, so the grid built next leaves it there.
        self.ui.pop_out(window);
        self.moved_out_or_in(view);
        Ok(true)
    }

    /// Takes `view` back into its cell and closes its window; `false` when it was in the grid.
    pub fn pop_in(&self, view: ViewId) -> Result<bool, EngineError> {
        {
            let mut state = self.state();
            let entry = state
                .views
                .get_mut(&view)
                .ok_or(EngineError::UnknownView(view))?;
            if entry.detached.take().is_none() {
                return Ok(false);
            }
            // The cell's rectangle until the grid reports the view's.
            entry.measured = None;
        }
        // The window lets the view go first, so the grid built next can take it.
        self.ui.pop_in(view);
        self.moved_out_or_in(view);
        Ok(true)
    }

    /// Whether `view` is in a window of its own.
    pub fn is_detached(&self, view: ViewId) -> bool {
        self.state()
            .views
            .get(&view)
            .is_some_and(|entry| entry.detached.is_some())
    }

    /// The views in a window of their own, in grid order.
    pub(super) fn detached_cells(&self) -> Vec<DetachedCell> {
        let state = self.state();
        state
            .order
            .iter()
            .filter_map(|id| state.views.get(id))
            .filter_map(|entry| {
                entry.detached.map(|detached| DetachedCell {
                    view: entry.id,
                    placeholder: detached.placeholder,
                })
            })
            .collect()
    }

    /// The effects of a view leaving or entering the grid: the grid again (every other view kept,
    /// only re-parented), new metrics, throttling and events.
    fn moved_out_or_in(&self, view: ViewId) {
        self.regrid();
        self.spawn_apply(view, false);
        self.rethrottle();
        self.emit(ViewEvent::Updated(view));
        self.emit(ViewEvent::LayoutChanged);
    }

    /// Sends the grid of the state in effect again, without a rebuild.
    pub(super) fn regrid(&self) {
        let (plan, gap, mut cells) = {
            let state = self.state();
            let plan = match crate::layout_plan::plan_hiding(&state.layout, &state.hidden) {
                Ok(plan) => plan,
                Err(error) => {
                    tracing::warn!(%error, "the grid was not sent again");
                    return;
                }
            };
            let cells: Vec<_> = state
                .order
                .iter()
                .filter_map(|id| state.views.get(id))
                .map(|entry| rebuild::ui_cell(&state.config, entry))
                .collect();
            (plan, state.layout.gap, cells)
        };
        let footer_height = self.with_footers(&mut cells);
        let (area, placed) = self.placed_views().unzip();
        self.ui.apply_grid(UiGrid {
            plan,
            gap,
            label_height: LABEL_HEIGHT,
            footer_height,
            cells,
            placed,
            area,
            detached: self.detached_cells(),
        });
    }
}

#[cfg(test)]
#[path = "pop_out_tests.rs"]
mod tests;
