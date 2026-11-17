//! The scale of each external view into its grid cell (RB-080-40): the window reports every
//! cell's size in pixels ([`HostEngines::cell_sized`]), [`plan`] turns that into one
//! [`CellScale`] per view - a WebKit view its own factor, every Firefox view the one factor of
//! the profile (`layout.css.devPixelsPerPx`, the smallest so every page fits) - and
//! [`HostEngines::apply_cells`] sends what changed: `ViewHandle::set_cell` per view, and for a
//! changed Firefox factor a rebuild of the engine (the pref is read at start). A cell counts
//! once it settled ([`SETTLE`]): a drag of the divider changes nothing until it rests, so no
//! intermediate factor restarts Firefox or builds a WebKit window anew.

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use rb_config::EngineKind;
use rb_engine::external::{
    CellPixels, CellScale, ExternalStatus, cell_factor, cell_scale, quantize, shared_factor,
};
use rb_engine::{ViewHost, ViewId};

use super::{Embedding, HostEngines};
use crate::cdp::lock;

/// How long a cell's size must hold before it is sent to the views.
pub const SETTLE: Duration = Duration::from_millis(300);

/// One external view as the plan sees it.
#[derive(Clone, Debug, PartialEq)]
pub struct CellInput {
    /// The view.
    pub view: ViewId,
    /// Its engine.
    pub engine: EngineKind,
    /// The breakpoint's width in CSS px.
    pub viewport_width: u32,
    /// Its cell in pixels, when the window measured it.
    pub pixels: Option<CellPixels>,
    /// Whether it should sit in its cell (the setting on, not released, not failed).
    pub wanted: bool,
    /// Whether its window runs: a closed or unavailable view has nothing to scale.
    pub running: bool,
}

/// What every external view is scaled to.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CellPlan {
    /// The scale per view; `None` leaves it at the breakpoint's size.
    pub per_view: HashMap<ViewId, Option<CellScale>>,
    /// The one factor of the Firefox profile; `None` without a Firefox view in its cell.
    pub firefox: Option<f64>,
}

/// The scales of `views`: a WebKit view its cell's quantized factor, the Firefox views the
/// smallest of theirs (one profile, one pref); each at its own cell's height. A view not
/// wanted in its cell or not measured yet is left at the breakpoint's size. A view that does
/// not run pulls no factor (nothing to fit) but keeps the scale it would get: its restart
/// opens with it.
pub fn plan(views: &[CellInput]) -> CellPlan {
    let factor_of = |view: &CellInput| {
        view.pixels
            .filter(|_| view.wanted)
            .map(|pixels| quantize(cell_factor(pixels, view.viewport_width)))
    };
    let firefox = shared_factor(
        views
            .iter()
            .filter(|view| view.engine == EngineKind::Firefox && view.running)
            .filter_map(factor_of),
    );
    let per_view = views
        .iter()
        .map(|view| {
            let factor = match view.engine {
                EngineKind::Firefox => firefox.filter(|_| factor_of(view).is_some()),
                _ => factor_of(view),
            };
            let scale = factor.and_then(|factor| {
                view.pixels
                    .map(|pixels| cell_scale(pixels, view.viewport_width, factor))
            });
            (view.view, scale)
        })
        .collect();
    CellPlan { per_view, firefox }
}

/// Whether the views' pages are scaled to their cells: the embedding and the scaling on.
fn scaling(embedding: &Embedding) -> bool {
    embedding.on && embedding.scale
}

impl HostEngines {
    /// The window laid `view`'s cell out at `pixels` (physical px): once the size held for
    /// [`SETTLE`], its page is scaled to it. Cheap for a Chromium view, which the engines do
    /// not know, and while the embedding or the scaling is off (the size is kept for later).
    pub fn cell_sized(&self, view: ViewId, pixels: CellPixels) {
        if self.windows.view(view).is_none() {
            return;
        }
        let generation = {
            let mut embedding = lock(&self.embedding);
            if embedding.cells.insert(view, pixels) == Some(pixels) || !scaling(&embedding) {
                return;
            }
            embedding.measure_generation += 1;
            embedding.measure_generation
        };
        let Some(me) = self.me.upgrade() else {
            return;
        };
        self.runtime.spawn(async move {
            tokio::time::sleep(SETTLE).await;
            {
                let mut embedding = lock(&me.embedding);
                if embedding.measure_generation != generation {
                    // A newer size came in: its timer applies.
                    return;
                }
                embedding.settled = embedding.cells.clone();
            }
            me.apply_cells();
        });
    }

    /// The scale `view` opens with (`ViewSpec.cell`): its settled cell's, when the window
    /// measured it and the view is wanted in its cell.
    pub(crate) fn cell_for(&self, view: ViewId, info: &rb_engine::ViewInfo) -> Option<CellScale> {
        let (pixels, wanted, firefox) = {
            let embedding = lock(&self.embedding);
            (
                embedding.settled.get(&view).copied(),
                scaling(&embedding)
                    && !embedding.released.contains(&view)
                    && !embedding.failed.contains(&view),
                embedding.firefox_factor,
            )
        };
        let pixels = pixels.filter(|_| wanted)?;
        let factor = match info.engine {
            EngineKind::Firefox => firefox?,
            _ => quantize(cell_factor(pixels, info.width)),
        };
        Some(cell_scale(pixels, info.width, factor))
    }

    /// The factor every Firefox window renders at (`layout.css.devPixelsPerPx`); `None` the
    /// display's own ratio.
    pub fn firefox_render_scale(&self) -> Option<f64> {
        lock(&self.embedding).firefox_factor
    }

    /// Sends every changed scale to its view, and rebuilds Firefox for a changed factor. The
    /// Firefox factor holds while the embedding is on and no cell asks for another (a view
    /// shown as a window of its own, the last one too, restarts nothing).
    pub(super) fn apply_cells(&self) {
        let Some(host) = self.host.upgrade() else {
            return;
        };
        let views = self.windows.list();
        let inputs: Vec<CellInput> = {
            let mut embedding = lock(&self.embedding);
            let known: HashSet<ViewId> = views.iter().map(|view| view.view).collect();
            embedding.cells.retain(|view, _| known.contains(view));
            embedding.settled.retain(|view, _| known.contains(view));
            embedding
                .applied_cells
                .retain(|view, _| known.contains(view));
            views
                .iter()
                .map(|view| CellInput {
                    view: view.view,
                    engine: view.engine,
                    viewport_width: host.emulation(view.view).map_or(0, |e| e.width),
                    pixels: embedding.settled.get(&view.view).copied(),
                    wanted: scaling(&embedding)
                        && !embedding.released.contains(&view.view)
                        && !embedding.failed.contains(&view.view),
                    running: view.status == ExternalStatus::Running,
                })
                .collect()
        };
        let plan = plan(&inputs);
        for (view, scale) in &plan.per_view {
            self.windows.set_spec_cell(*view, *scale);
        }
        let (sends, firefox_changed, firefox) = {
            let mut embedding = lock(&self.embedding);
            let firefox = if scaling(&embedding) {
                plan.firefox.or(embedding.firefox_factor)
            } else {
                None
            };
            let firefox_changed = embedding.firefox_factor != firefox;
            embedding.firefox_factor = firefox;
            let mut sends = Vec::new();
            for view in &views {
                if view.status != ExternalStatus::Running {
                    embedding.applied_cells.remove(&view.view);
                    continue;
                }
                let wanted = plan.per_view.get(&view.view).copied().flatten();
                // A view nothing was sent to is at the breakpoint's size.
                let applied = embedding
                    .applied_cells
                    .get(&view.view)
                    .copied()
                    .unwrap_or(None);
                // A Firefox view opens at the factor it was started with: a changed factor
                // restarts it, so nothing is sent before.
                let restarts = view.engine == EngineKind::Firefox && firefox_changed;
                if applied != wanted && !restarts {
                    embedding.applied_cells.insert(view.view, wanted);
                    sends.push((view.view, wanted));
                }
            }
            (sends, firefox_changed, firefox)
        };
        for (view, scale) in sends {
            let Some(handle) = self.windows.handle(view) else {
                continue;
            };
            let me = self.me.clone();
            self.runtime.spawn(async move {
                match handle.set_cell(scale).await {
                    Ok(applied) if applied.window_replaced => {
                        tracing::info!(%view, "the view's window was built anew for its cell")
                    }
                    Ok(_) => {}
                    Err(error) => {
                        tracing::debug!(%view, %error, "the view was not scaled to its cell");
                        // Not applied: the next publish sends it again.
                        if let Some(me) = me.upgrade() {
                            lock(&me.embedding).applied_cells.remove(&view);
                        }
                    }
                }
            });
        }
        if firefox_changed {
            tracing::info!(factor = ?firefox, "the Firefox views render at a new factor");
            if let Some(bridge) = super::super::firefox::for_host(&host) {
                bridge.refresh();
            }
        }
    }
}

#[cfg(test)]
#[path = "cells_tests.rs"]
mod tests;
