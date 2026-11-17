//! What feeds the footers besides a new grid or config (RB-080-44, RB-090-71, RB-090-73): the
//! view events (a size, a breakpoint, an emulation, a navigation, a new layout), the vitals bars
//! the core words from the shell state's `metrics` (`crate::cell_info` hands them over after
//! every burst of changes, `view_footer::vitals`) and the Lighthouse runs on disk
//! (`view_footer::lighthouse_runs` hands them over when they changed), of which each view shows
//! the newest of its page and breakpoint (`view_footer::lighthouse`).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use rb_config::{FileNameTime, ViewFooterSettings};
use rb_engine::{ViewEvent, ViewId};
use tokio::sync::broadcast::error::RecvError;

use super::super::Host;
use super::super::state::{HostState, ViewState};
use crate::cell_info::{Piece, PieceAction};
use crate::view_footer::lighthouse::{LighthouseBar, Measured, bar, breakpoint_name, latest};

impl Host {
    /// `view` committed `url`: on another document than before its footer names that
    /// document's queries when the footer read them, else none until the next analysis.
    pub(super) fn footer_navigated(&self, view: ViewId, url: &str) {
        if self.state().footer.navigated(view, url) {
            self.refresh_footer(Some(view));
        }
    }

    /// The vitals bar of every view as worded now (`view_footer::vitals::bars`); the strips of
    /// the views whose bar changed are written again while `ui.viewFooter.vitals` shows them -
    /// one burst of readings rewrites only the views it moved.
    pub fn set_footer_vitals(&self, bars: BTreeMap<ViewId, Piece>) {
        let changed = {
            let mut state = self.state();
            let old = &state.footer.vitals;
            let changed = changed_views(old, &bars);
            if changed.is_empty() {
                return;
            }
            state.footer.vitals = bars;
            let shown = state
                .footer
                .settings
                .as_ref()
                .is_some_and(|settings| settings.vitals);
            if !shown {
                return;
            }
            changed
        };
        self.refresh_footers_of(Some(&changed));
    }

    /// The vitals bar of `view` as handed over last, shown or not.
    #[cfg(test)]
    pub fn footer_vitals_of(&self, view: ViewId) -> Option<Piece> {
        self.state().footer.vitals.get(&view).cloned()
    }

    /// What a click on the vitals bar of `view` does: mark the element of its worst vital, or
    /// make the view active; read from the bar the window shows, never from the window.
    pub fn footer_vitals_action(&self, view: ViewId) -> PieceAction {
        self.state()
            .footer
            .sent
            .get(&view)
            .and_then(|footer| footer.vitals.as_ref())
            .map_or(PieceAction::Activate, |piece| piece.action)
    }
}

impl Host {
    /// The Lighthouse runs on disk as read now (RB-090-73); the footers are written again where
    /// a view's bar changed while `ui.viewFooter.lighthouse` shows them.
    pub fn set_footer_lighthouse(&self, runs: Vec<Measured>) {
        {
            let mut state = self.state();
            if state.footer.lighthouse == runs {
                return;
            }
            state.footer.lighthouse = runs;
            let shown = state
                .footer
                .settings
                .as_ref()
                .is_some_and(|settings| settings.lighthouse);
            if !shown {
                return;
            }
        }
        self.refresh_footers();
    }

    /// The run and breakpoint whose report a click on the Lighthouse bar of `view` opens: the
    /// bar the window shows; `None` without one.
    pub fn footer_lighthouse_target(&self, view: ViewId) -> Option<(String, String)> {
        self.state()
            .footer
            .sent
            .get(&view)
            .filter(|footer| footer.lighthouse_shown)
            .and_then(|footer| footer.lighthouse.as_ref())
            .map(|bar| (bar.run.clone(), bar.breakpoint.clone()))
    }
}

/// The vitals bar and the Lighthouse bar of `entry` while `settings` show them: the vitals as
/// handed over last, the newest Lighthouse run of the page the view shows and its breakpoint.
pub(super) fn extra_bars(
    state: &HostState,
    settings: &ViewFooterSettings,
    entry: &ViewState,
) -> (Option<Piece>, Option<LighthouseBar>) {
    let vitals = settings
        .vitals
        .then(|| state.footer.vitals.get(&entry.id).cloned())
        .flatten();
    let lighthouse = settings
        .lighthouse
        .then(|| lighthouse_of(state, entry))
        .flatten();
    (vitals, lighthouse)
}

/// The Lighthouse bar of `entry`: the newest run of its document and breakpoint, its time in
/// the local zone.
fn lighthouse_of(state: &HostState, entry: &ViewState) -> Option<LighthouseBar> {
    let document = state.footer.documents.get(&entry.id)?;
    let breakpoint = entry.breakpoint.as_ref().map(|shown| shown.id.as_str());
    let name = breakpoint_name(entry.id, breakpoint);
    let measured = latest(&state.footer.lighthouse, document, &name)?;
    let offset = measured.started_at.map_or(0, |secs| {
        crate::local_time::offset_in(FileNameTime::Local, secs)
    });
    Some(bar(measured, offset))
}

/// The views whose bar is not the same in `old` and `new`: changed, come or gone.
fn changed_views(old: &BTreeMap<ViewId, Piece>, new: &BTreeMap<ViewId, Piece>) -> BTreeSet<ViewId> {
    old.keys()
        .chain(new.keys())
        .filter(|view| old.get(view) != new.get(view))
        .copied()
        .collect()
}

/// Starts the task that keeps the footers of `host` current.
pub(in crate::host) fn watch(host: &Arc<Host>) {
    let mut events = host.events.subscribe();
    let weak = Arc::downgrade(host);
    host.runtime.spawn(async move {
        loop {
            let event = events.recv().await;
            let Some(host) = weak.upgrade() else {
                return;
            };
            match event {
                // A size, a breakpoint, an emulation: that view's footer only.
                Ok(ViewEvent::Updated(view)) => host.refresh_footer(Some(view)),
                Ok(ViewEvent::Navigated { view, url }) => host.footer_navigated(view, &url),
                // A new grid area or new places: the footers of the views whose width changed
                // are fitted again; nothing else is written (RB-080-53).
                Ok(ViewEvent::LayoutChanged) => host.refit_footers(),
                // A grid carries its footers, a config change and an analysis say so
                // themselves, and the active view changes none.
                Ok(_) => {}
                Err(RecvError::Lagged(_)) => host.refresh_footers(),
                Err(RecvError::Closed) => return,
            }
        }
    });
}

#[cfg(test)]
#[path = "footer_lighthouse_tests.rs"]
mod lighthouse_tests;
