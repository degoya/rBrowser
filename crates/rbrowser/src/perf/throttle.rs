//! The throttling of views (RB-020-09), pure: from the views' facts and the settings to what
//! the browser is told. A view without area on screen is reported hidden (`WasHidden`):
//! Chromium stops painting it and throttles its timers, while CDP - navigation sync, the sync
//! agent's scroll apply - still reaches it; shown again, it catches up with the leader
//! ([`super::catchup`]). Visible views that are not active may get a CPU budget
//! (`Emulation.setCPUThrottlingRate`, `performance.inactiveFps`): Chromium has no per-view
//! frame cap for windowed browsers, so a slower main thread is the lever. The budget never
//! lowers a view's own CPU emulation (RB-080-10).

use std::collections::BTreeMap;

use rb_config::PerformanceSettings;
use rb_engine::ViewId;
use serde::Serialize;

/// The frame rate the budget is measured against.
const FULL_FPS: u32 = 60;
/// The strongest slow-down the budget asks for.
pub const MAX_CPU_RATE: f64 = 20.0;

/// What the throttling follows (`performance.throttleHidden`, `performance.inactiveFps`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThrottleSettings {
    /// Views without area are reported hidden.
    pub throttle_hidden: bool,
    /// The frame budget of visible views that are not active; 0 or 60 and above is none.
    pub inactive_fps: u32,
}

impl Default for ThrottleSettings {
    fn default() -> Self {
        Self::from(&PerformanceSettings::default())
    }
}

impl From<&PerformanceSettings> for ThrottleSettings {
    fn from(settings: &PerformanceSettings) -> Self {
        let settings = settings.clamped();
        Self {
            throttle_hidden: settings.throttle_hidden,
            inactive_fps: settings.inactive_fps,
        }
    }
}

/// What one view is throttled to, for its label and `view_metrics` (RB-060-10 reads it with
/// [`super::throttle_state`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum ThrottleState {
    /// Rendered at full speed.
    Visible,
    /// Reported hidden: no painting, slow timers.
    Hidden,
    /// Visible, its main thread slowed to about `fps` frames of work per second.
    CpuLimited {
        /// The frame budget.
        fps: u32,
    },
}

/// One view as the throttling sees it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewFacts {
    /// The view.
    pub view: ViewId,
    /// The window laid it out with no area (or not on screen).
    pub zero_area: bool,
    /// It is the active view (the sync leader).
    pub active: bool,
    /// It has a CDP session.
    pub attached: bool,
    /// The CPU throttling of its own emulation (`view.emulate.cpu`), 1 without.
    pub emulation_rate: f64,
}

/// One thing to tell the browser.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ThrottleAction {
    /// `WasHidden(true)`.
    Hide(ViewId),
    /// `WasHidden(false)`.
    Show(ViewId),
    /// Bring the view to the leader's URL and scroll position.
    CatchUp(ViewId),
    /// `Emulation.setCPUThrottlingRate { rate }`.
    CpuRate {
        /// The view.
        view: ViewId,
        /// The rate, 1 for full speed.
        rate: f64,
    },
}

/// What the browser was told of one view.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Applied {
    hidden: bool,
    /// The CPU rate sent last; `None` before the session exists.
    rate: Option<f64>,
    /// The frame budget behind `rate`, for [`ThrottleState`].
    fps: Option<u32>,
}

/// The throttling of the views of one window.
#[derive(Clone, Debug, Default)]
pub struct Throttle {
    views: BTreeMap<ViewId, Applied>,
}

impl Throttle {
    /// What to tell the browser so `views` are throttled as `settings` say; records it as told.
    /// Views not in `views` are forgotten. Actions come per view in `views` order.
    pub fn plan(&mut self, settings: ThrottleSettings, views: &[ViewFacts]) -> Vec<ThrottleAction> {
        self.views
            .retain(|id, _| views.iter().any(|facts| facts.view == *id));
        let mut actions = Vec::new();
        for facts in views {
            let applied = self.views.entry(facts.view).or_default();
            let hide = settings.throttle_hidden && facts.zero_area;
            if hide != applied.hidden {
                applied.hidden = hide;
                if hide {
                    actions.push(ThrottleAction::Hide(facts.view));
                } else {
                    actions.push(ThrottleAction::Show(facts.view));
                    actions.push(ThrottleAction::CatchUp(facts.view));
                }
            }
            if !facts.attached {
                continue;
            }
            let budget = (!facts.active && !hide)
                .then_some(settings.inactive_fps)
                .filter(|fps| *fps > 0 && *fps < FULL_FPS);
            let rate = budget.map_or(1.0, cpu_rate).max(facts.emulation_rate);
            applied.fps = budget.filter(|_| rate > facts.emulation_rate);
            let sent = applied.rate.unwrap_or(1.0);
            if (rate - sent).abs() > f64::EPSILON {
                applied.rate = Some(rate);
                actions.push(ThrottleAction::CpuRate {
                    view: facts.view,
                    rate,
                });
            }
        }
        actions
    }

    /// `view` has a new CDP session, whose emulation sent `emulation_rate` (1 without).
    pub fn session_reset(&mut self, view: ViewId, emulation_rate: f64) {
        let applied = self.views.entry(view).or_default();
        applied.rate = Some(emulation_rate);
        applied.fps = None;
    }

    /// What `view` is throttled to; [`ThrottleState::Visible`] for an unknown view.
    pub fn state(&self, view: ViewId) -> ThrottleState {
        match self.views.get(&view) {
            Some(applied) if applied.hidden => ThrottleState::Hidden,
            Some(Applied { fps: Some(fps), .. }) => ThrottleState::CpuLimited { fps: *fps },
            _ => ThrottleState::Visible,
        }
    }
}

/// The CPU throttling rate of a frame budget of `fps`: 60 / `fps`, at most [`MAX_CPU_RATE`];
/// 1 for no budget.
pub fn cpu_rate(fps: u32) -> f64 {
    if fps == 0 || fps >= FULL_FPS {
        return 1.0;
    }
    (f64::from(FULL_FPS) / f64::from(fps)).min(MAX_CPU_RATE)
}

#[cfg(test)]
#[path = "throttle_tests.rs"]
mod tests;
