//! The external views and their lifecycle, without a runtime: which views are external, their
//! status (the badge and the placeholder in the grid), the last URL to restore, and whether a
//! crashed view is restarted - at most [`Backoff::max_attempts`] times in a row, each after a
//! longer wait. A window the person closed stays closed until they open it again (RB-080-38).

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use rb_config::EngineKind;
use serde::{Deserialize, Serialize};

use super::WindowRect;
use crate::ViewId;

/// How long a view must have run for its next end to count as a new streak (its restart
/// attempts start again from one).
pub const STABLE_RUN: Duration = Duration::from_secs(60);

/// The status of an external view.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ExternalStatus {
    /// The engine starts or the window opens.
    Starting,
    /// The window is open.
    Running,
    /// The engine ended or the view crashed: the badge "Engine stopped".
    Exited,
    /// The person closed the window: it stays closed until "Open again" (RB-080-38).
    Closed,
    /// Waiting to restart after an end.
    Restarting,
    /// It could not be started (the engine failed to start, three restarts failed).
    Failed,
    /// The engine is not available (not implemented yet, not installed).
    Unavailable,
}

/// What the registry knows of one external view.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalView {
    /// The view.
    pub view: ViewId,
    /// Its label (the breakpoint's).
    pub label: String,
    /// Its breakpoint.
    pub breakpoint: String,
    /// Its engine.
    pub engine: EngineKind,
    /// Where it stands.
    pub status: ExternalStatus,
    /// Why it is not running, for the user (an engine's error, "not available yet").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// The last committed URL: what a restart loads again.
    pub url: String,
    /// The window's last rectangle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window: Option<WindowRect>,
    /// The emulation fields the engine did not apply as asked.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unsupported: Vec<String>,
    /// The window sits in its grid cell, owned by the main window (RB-080-39); `false` while
    /// it is a window of its own (`engines.external.embed` off, shown as its own window, or
    /// the embedding failed).
    #[serde(default)]
    pub embedded: bool,
    /// The engine built the window anew (RB-080-40) and the app has not heard it yet.
    #[serde(skip)]
    window_replaced: bool,
    /// Automatic restarts in the current streak.
    #[serde(skip)]
    attempts: u8,
    /// When it last started running.
    #[serde(skip)]
    since: Option<Instant>,
}

/// One external view the layout wants.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExternalSpec {
    /// The view.
    pub view: ViewId,
    /// Its label.
    pub label: String,
    /// Its breakpoint.
    pub breakpoint: String,
    /// Its engine.
    pub engine: EngineKind,
    /// The URL it starts with.
    pub url: String,
}

/// The waits between automatic restarts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Backoff {
    /// Restarts in a row before the view stays ended.
    pub max_attempts: u8,
    /// The first wait; each further one doubles.
    pub base: Duration,
}

impl Default for Backoff {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base: Duration::from_secs(1),
        }
    }
}

impl Backoff {
    /// The wait before restart number `attempt` (1-based); `None` past the last.
    pub fn delay(&self, attempt: u8) -> Option<Duration> {
        if attempt == 0 || attempt > self.max_attempts {
            return None;
        }
        Some(
            self.base
                .saturating_mul(1 << u32::from(attempt - 1).min(16)),
        )
    }
}

/// What to do after a view ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExitAction {
    /// Nothing: the person closed the window, the restart after a crash is off, or the view is
    /// unknown.
    Stay,
    /// Restart after the wait.
    RestartAfter(Duration),
    /// The streak used every attempt: the view stays ended.
    GiveUp,
}

/// A request named a view or engine the registry does not have.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum RegistryError {
    /// No external view has this id.
    #[error("view {0} is not an external view")]
    UnknownView(ViewId),
}

/// The external views.
#[derive(Debug, Default)]
pub struct Registry {
    views: BTreeMap<ViewId, ExternalView>,
    backoff: Backoff,
    restart_on_crash: bool,
}

impl Registry {
    /// An empty registry restarting crashed views with `backoff` when `restart_on_crash` is on
    /// (`engines.external.restartOnCrash`).
    pub fn new(backoff: Backoff, restart_on_crash: bool) -> Self {
        Self {
            views: BTreeMap::new(),
            backoff,
            restart_on_crash,
        }
    }

    /// The restart after a crash on or off (`engines.external.restartOnCrash`).
    pub fn set_restart_on_crash(&mut self, on: bool) {
        self.restart_on_crash = on;
    }

    /// Makes the registry hold exactly `wanted`: returns the views that are new (to start) and
    /// the ones that went (to close). A kept view keeps its state; one whose engine changed is
    /// both gone and new, and starts on the page it showed.
    pub fn sync(&mut self, wanted: &[ExternalSpec]) -> (Vec<ViewId>, Vec<ViewId>) {
        let gone: Vec<ViewId> = self
            .views
            .values()
            .filter(|known| {
                !wanted
                    .iter()
                    .any(|spec| spec.view == known.view && spec.engine == known.engine)
            })
            .map(|known| known.view)
            .collect();
        let mut last_urls = BTreeMap::new();
        for view in &gone {
            if let Some(known) = self.views.remove(view) {
                last_urls.insert(*view, known.url);
            }
        }
        let mut added = Vec::new();
        for spec in wanted {
            if let Some(kept) = self.views.get_mut(&spec.view) {
                kept.label.clone_from(&spec.label);
                continue;
            }
            added.push(spec.view);
            let url = last_urls
                .remove(&spec.view)
                .unwrap_or_else(|| spec.url.clone());
            self.views.insert(
                spec.view,
                ExternalView {
                    view: spec.view,
                    label: spec.label.clone(),
                    breakpoint: spec.breakpoint.clone(),
                    engine: spec.engine,
                    status: ExternalStatus::Starting,
                    reason: None,
                    url,
                    window: None,
                    unsupported: Vec::new(),
                    embedded: false,
                    window_replaced: false,
                    attempts: 0,
                    since: None,
                },
            );
        }
        (added, gone)
    }

    /// One view.
    pub fn get(&self, view: ViewId) -> Option<&ExternalView> {
        self.views.get(&view)
    }

    /// Every view, by id.
    pub fn list(&self) -> Vec<ExternalView> {
        self.views.values().cloned().collect()
    }

    /// The views of `engine`, by id.
    pub fn of_engine(&self, engine: EngineKind) -> Vec<ViewId> {
        self.views
            .values()
            .filter(|view| view.engine == engine)
            .map(|view| view.view)
            .collect()
    }

    /// `view` is starting (again): a new window, not embedded yet.
    pub fn starting(&mut self, view: ViewId) {
        self.set(view, ExternalStatus::Starting, None);
        if let Some(entry) = self.views.get_mut(&view) {
            entry.embedded = false;
        }
    }

    /// `view`'s window sits in its cell (`true`) or is a window of its own; answers whether
    /// that changed. A view that does not run is never embedded.
    pub fn set_embedded(&mut self, view: ViewId, embedded: bool) -> bool {
        let Some(entry) = self.views.get_mut(&view) else {
            return false;
        };
        let embedded = embedded && entry.status == ExternalStatus::Running;
        let changed = entry.embedded != embedded;
        entry.embedded = embedded;
        changed
    }

    /// The engine built `view`'s window anew (a WebKit context at another factor, RB-080-40):
    /// it is not in its cell any more; [`Registry::take_replaced`] hands it to the app once.
    pub fn window_replaced(&mut self, view: ViewId) {
        if let Some(entry) = self.views.get_mut(&view) {
            entry.embedded = false;
            entry.window_replaced = true;
        }
    }

    /// The views whose window the engine built anew since the last call, by id.
    pub fn take_replaced(&mut self) -> Vec<ViewId> {
        let mut replaced: Vec<ViewId> = self
            .views
            .values_mut()
            .filter_map(|entry| std::mem::take(&mut entry.window_replaced).then_some(entry.view))
            .collect();
        replaced.sort();
        replaced
    }

    /// `view`'s window is open at `now`, with the emulation fields its engine did not apply.
    pub fn started(&mut self, view: ViewId, unsupported: Vec<String>, now: Instant) {
        if let Some(entry) = self.views.get_mut(&view) {
            entry.status = ExternalStatus::Running;
            entry.reason = None;
            entry.unsupported = unsupported;
            entry.since = Some(now);
        }
    }

    /// `view` could not start: `Failed` with the engine's reason.
    pub fn failed(&mut self, view: ViewId, reason: String) {
        self.set(view, ExternalStatus::Failed, Some(reason));
    }

    /// `view`'s engine is not available, for `reason`.
    pub fn unavailable(&mut self, view: ViewId, reason: String) {
        self.set(view, ExternalStatus::Unavailable, Some(reason));
    }

    /// `view` committed `url`: a restart loads it again.
    pub fn navigated(&mut self, view: ViewId, url: &str) {
        if let Some(entry) = self.views.get_mut(&view) {
            url.clone_into(&mut entry.url);
        }
    }

    /// `view`'s window moved to `rect`.
    pub fn moved(&mut self, view: ViewId, rect: WindowRect) {
        if let Some(entry) = self.views.get_mut(&view) {
            entry.window = Some(rect);
        }
    }

    /// `view` ended at `now`: closed by the person (`crashed` false) it shows `Closed` and
    /// stays so; crashed, it shows `Exited` (or `Restarting`), and the answer says whether and
    /// when it restarts. A view that ran [`STABLE_RUN`] starts a new streak.
    pub fn exited(&mut self, view: ViewId, crashed: bool, now: Instant) -> ExitAction {
        let (restart_on_crash, backoff) = (self.restart_on_crash, self.backoff);
        let Some(entry) = self.views.get_mut(&view) else {
            return ExitAction::Stay;
        };
        if entry
            .since
            .is_some_and(|since| now.saturating_duration_since(since) >= STABLE_RUN)
        {
            entry.attempts = 0;
        }
        entry.since = None;
        if !crashed {
            entry.status = ExternalStatus::Closed;
            entry.embedded = false;
            entry.reason = None;
            return ExitAction::Stay;
        }
        entry.status = ExternalStatus::Exited;
        // Its window is gone: not in its cell any more.
        entry.embedded = false;
        if !restart_on_crash {
            return ExitAction::Stay;
        }
        let attempt = entry.attempts.saturating_add(1);
        match backoff.delay(attempt) {
            Some(wait) => {
                entry.attempts = attempt;
                entry.status = ExternalStatus::Restarting;
                ExitAction::RestartAfter(wait)
            }
            None => ExitAction::GiveUp,
        }
    }

    /// The user asked to restart `view`: a new streak.
    pub fn restart_requested(&mut self, view: ViewId) -> Result<(), RegistryError> {
        let entry = self
            .views
            .get_mut(&view)
            .ok_or(RegistryError::UnknownView(view))?;
        entry.attempts = 0;
        entry.status = ExternalStatus::Restarting;
        entry.reason = None;
        Ok(())
    }

    fn set(&mut self, view: ViewId, status: ExternalStatus, reason: Option<String>) {
        if let Some(entry) = self.views.get_mut(&view) {
            entry.status = status;
            entry.reason = reason;
            if status != ExternalStatus::Running {
                entry.embedded = false;
            }
        }
    }
}

#[cfg(test)]
#[path = "registry_tests.rs"]
mod tests;
