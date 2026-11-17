//! An external view in its grid cell (RB-080-39), the core's half. [`decide`] says per external
//! view whether its window should be asked into its cell or given back as a window of its own;
//! [`HostEngines`] keeps what was asked of the window, what the person released ("show as own
//! window": `view.pop_out`, `view.external.arrange`) and what failed, and the window
//! (`cef_app/embed.rs`) confirms a binding ([`HostEngines::embedded_bound`]) or reports why it
//! could not ([`HostEngines::embed_failed`]). Everything here compiles everywhere; the Win32
//! calls are the window's. The Chromium stand-in in the cell stays underneath: it shows the
//! view while the window is away or the embedding failed. [`cells`] scales each embedded
//! view's page to its cell (RB-080-40).

pub mod cells;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use rb_engine::ViewId;
use rb_engine::external::{CellPixels, CellScale, ExternalStatus, ExternalView, NativeWindow};

use super::HostEngines;
use crate::cdp::lock;
use crate::host::ui::EmbedRequest;

/// What the embedding knows beyond the registry.
#[derive(Debug, Default)]
pub struct Embedding {
    /// `engines.external.embed`.
    pub on: bool,
    /// `engines.external.scaleToCell`.
    pub scale: bool,
    /// The views whose window was asked into its cell (confirmed or not yet).
    pub requested: HashSet<ViewId>,
    /// The views the person shows as windows of their own until `view.pop_in`.
    pub released: HashSet<ViewId>,
    /// The views whose window could not be embedded; a new window tries again.
    pub failed: HashSet<ViewId>,
    /// The handle of each running view as last seen; another one is a new window.
    pub handles: HashMap<ViewId, usize>,
    /// Each view's cell in pixels, as the window laid it out last (RB-080-40).
    pub cells: HashMap<ViewId, CellPixels>,
    /// The cells as they were when they last settled ([`cells::SETTLE`]): what the views get.
    pub settled: HashMap<ViewId, CellPixels>,
    /// Stamps the settle timers: only the latest applies.
    pub measure_generation: u64,
    /// The scale last sent to each running view.
    pub applied_cells: HashMap<ViewId, Option<CellScale>>,
    /// The one factor the Firefox profile renders at; `None` without a Firefox view in its cell.
    pub firefox_factor: Option<f64>,
}

/// The processes that may own an engine's windows: `root` (the process the launcher knows)
/// and every process below it in `links` (pid, parent). Firefox on Windows starts a launcher
/// process whose child is the browser that owns the `MozillaWindowClass` windows; the pid
/// rBrowser holds is the launcher's (`-wait-for-browser` keeps it), so the window is looked
/// for in the whole family (RB-080-39, finding 2 of the review).
pub fn process_family(root: u32, links: &[(u32, u32)]) -> Vec<u32> {
    crate::perf::process::descendants(root, links)
        .into_iter()
        .collect()
}

/// What to do for one external view.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EmbedAction {
    /// Ask the window to show it in its cell.
    Embed(EmbedRequest),
    /// Give it back as a window of its own.
    Release,
    /// Leave it as it is.
    Nothing,
}

/// The action for `view` under `embedding`, given how its engine names its window (`hint`,
/// `None` while the engine does not know it): a running view whose window is known is asked in
/// once, unless the setting is off, the person released it or the embedding failed; a view
/// that was asked in (or sits in its cell) and should not is released.
pub fn decide(
    embedding: &Embedding,
    view: &ExternalView,
    hint: Option<NativeWindow>,
) -> EmbedAction {
    let running = view.status == ExternalStatus::Running;
    let requested = embedding.requested.contains(&view.view);
    let wanted = embedding.on
        && running
        && !embedding.released.contains(&view.view)
        && !embedding.failed.contains(&view.view);
    match (wanted, hint) {
        (true, Some(window)) if !requested => EmbedAction::Embed(EmbedRequest {
            view: view.view,
            window,
            expected: view.window,
        }),
        (true, _) => EmbedAction::Nothing,
        (false, _) if requested || view.embedded => EmbedAction::Release,
        (false, _) => EmbedAction::Nothing,
    }
}

impl HostEngines {
    /// Asks the window to embed or release each external view as [`decide`] says; runs inside
    /// `publish`, before the state goes to the shell. A view that does not run is a new window
    /// next time: nothing asked, released or failed holds for it.
    pub(super) fn sync_embedding(&self, ui: &dyn crate::host::ui::UiBackend) {
        // The views and their handles are read under the lock: two publishes (the restart's
        // and the started view's, on different threads) see them in one order, so the newer
        // window is never judged by the older's state.
        let mut embedding = lock(&self.embedding);
        let views = self.windows.list();
        let running: HashSet<ViewId> = views
            .iter()
            .filter(|view| view.status == ExternalStatus::Running)
            .map(|view| view.view)
            .collect();
        embedding.requested.retain(|view| running.contains(view));
        embedding.released.retain(|view| running.contains(view));
        embedding.failed.retain(|view| running.contains(view));
        // A new window (a restart, a window the engine built anew for its cell - RB-080-40)
        // is asked in afresh: nothing asked, released or failed holds for the old one.
        let handles: HashMap<ViewId, usize> = views
            .iter()
            .filter_map(|view| {
                let handle = self.windows.handle(view.view)?;
                Some((view.view, Arc::as_ptr(&handle).cast::<()>() as usize))
            })
            .collect();
        for (view, handle) in &handles {
            if embedding.handles.get(view) != Some(handle) {
                embedding.requested.remove(view);
                embedding.released.remove(view);
                embedding.failed.remove(view);
            }
        }
        embedding.handles = handles;
        for view in self.windows.take_replaced() {
            embedding.requested.remove(&view);
        }
        for view in &views {
            let hint = self
                .windows
                .handle(view.view)
                .and_then(|handle| handle.native_window());
            match decide(&embedding, view, hint) {
                EmbedAction::Embed(request) => {
                    embedding.requested.insert(view.view);
                    ui.embed_external(request);
                }
                EmbedAction::Release => {
                    embedding.requested.remove(&view.view);
                    self.windows.set_embedded(view.view, false);
                    let rect = running
                        .contains(&view.view)
                        .then(|| self.windows.placement_rect(view.view))
                        .flatten();
                    ui.release_external(view.view, rect);
                }
                EmbedAction::Nothing => {}
            }
        }
        drop(embedding);
        self.apply_cells();
    }

    /// The window holds `view`'s window in its cell now.
    pub fn embedded_bound(&self, view: ViewId) {
        if self.windows.set_embedded(view, true) {
            tracing::info!(%view, "external view shown in its cell");
            self.publish();
        }
    }

    /// The window could not embed `view`'s window, for `reason`: the view stays a window of
    /// its own (the stand-in in its cell shows it) until its next window.
    pub fn embed_failed(&self, view: ViewId, reason: &str) {
        tracing::warn!(%view, reason, "the external view stays a window of its own");
        {
            let mut embedding = lock(&self.embedding);
            embedding.failed.insert(view);
            embedding.requested.remove(&view);
        }
        self.windows.set_embedded(view, false);
        self.publish();
    }

    /// Whether `view` is an external view (on Firefox or WebKit).
    pub fn is_external(&self, view: ViewId) -> bool {
        self.windows.view(view).is_some()
    }

    /// Whether `view` sits in its cell or was asked in.
    pub fn is_embedded(&self, view: ViewId) -> bool {
        lock(&self.embedding).requested.contains(&view)
            || self.windows.view(view).is_some_and(|known| known.embedded)
    }

    /// Shows `view` as a window of its own (`view.pop_out` of an external view); answers
    /// whether it was in its cell.
    pub fn release(&self, view: ViewId) -> bool {
        let was = self.is_embedded(view);
        lock(&self.embedding).released.insert(view);
        self.publish();
        was
    }

    /// Puts `view` back into its cell (`view.pop_in` of an external view); answers whether that
    /// changes anything: it was shown as a window of its own and the embedding is on.
    pub fn embed_again(&self, view: ViewId) -> bool {
        let was = {
            let mut embedding = lock(&self.embedding);
            embedding.failed.remove(&view);
            embedding.released.remove(&view) && embedding.on
        };
        self.publish();
        was
    }

    /// Shows every running external view as a window of its own (`view.external.arrange` with
    /// the embedding on); answers the views released from their cells.
    pub fn release_all(&self) -> Vec<ViewId> {
        let running: Vec<ViewId> = self
            .windows
            .list()
            .into_iter()
            .filter(|view| view.status == ExternalStatus::Running)
            .map(|view| view.view)
            .collect();
        let released: Vec<ViewId> = running
            .iter()
            .copied()
            .filter(|view| self.is_embedded(*view))
            .collect();
        lock(&self.embedding).released.extend(running);
        self.publish();
        released
    }
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "cells_host_tests.rs"]
mod cells_host_tests;
