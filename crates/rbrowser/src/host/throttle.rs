//! The host's half of the throttling (RB-020-09, [`crate::perf::throttle`]): the views' facts
//! in, the window and CDP calls out. Run after every change of a view's area, the active view,
//! the grid, a CDP session and the settings.

use std::sync::Arc;

use rb_engine::{ViewEvent, ViewId};
use serde_json::json;

use super::Host;
use crate::cdp::lock;
use crate::perf::{ThrottleAction, ThrottleSettings, ThrottleState, ViewFacts};

impl Host {
    /// Throttles the views as `settings` say from now on.
    pub fn set_throttle_settings(&self, settings: ThrottleSettings) {
        let changed = {
            let mut current = lock(&self.throttle_settings);
            std::mem::replace(&mut *current, settings) != settings
        };
        if changed {
            self.rethrottle();
        }
    }

    /// The throttling in effect.
    pub fn throttle_settings(&self) -> ThrottleSettings {
        *lock(&self.throttle_settings)
    }

    /// What `view` is throttled to; [`ThrottleState::Visible`] for an unknown view.
    pub fn throttle_state(&self, view: ViewId) -> ThrottleState {
        lock(&self.throttle).state(view)
    }

    /// `view` has a new CDP session, whose attach sent its emulation's CPU rate.
    pub(super) fn throttle_session_reset(&self, view: ViewId) {
        let rate = self
            .state()
            .views
            .get(&view)
            .map_or(1.0, |entry| entry.emulation.cpu_throttling.unwrap_or(1.0));
        lock(&self.throttle).session_reset(view, rate);
        self.rethrottle();
    }

    /// Tells the browser what the throttling asks for now.
    pub(super) fn rethrottle(&self) {
        let settings = self.throttle_settings();
        let mut sessions = Vec::new();
        let actions = {
            let mut state = self.state();
            let active = state.active;
            let facts: Vec<ViewFacts> = state
                .views
                .values()
                .map(|entry| ViewFacts {
                    view: entry.id,
                    zero_area: entry.off_screen
                        || entry
                            .measured
                            .is_some_and(|rect| rect.width == 0 || rect.height == 0),
                    active: active == Some(entry.id),
                    attached: entry.session.is_some(),
                    emulation_rate: entry.emulation.cpu_throttling.unwrap_or(1.0),
                })
                .collect();
            let actions = lock(&self.throttle).plan(settings, &facts);
            for action in &actions {
                match action {
                    ThrottleAction::Hide(view) | ThrottleAction::Show(view) => {
                        if let Some(entry) = state.views.get_mut(view) {
                            entry.hidden = matches!(action, ThrottleAction::Hide(_));
                        }
                    }
                    ThrottleAction::CpuRate { view, rate } => {
                        if let Some(session) = state
                            .views
                            .get(view)
                            .and_then(|entry| entry.session.clone())
                        {
                            sessions.push((*view, session, *rate));
                        }
                    }
                    ThrottleAction::CatchUp(_) => {}
                }
            }
            actions
        };
        for action in actions {
            match action {
                ThrottleAction::Hide(view) => {
                    self.ui.set_hidden(view, true);
                    self.emit(ViewEvent::Updated(view));
                }
                ThrottleAction::Show(view) => {
                    self.ui.set_hidden(view, false);
                    self.emit(ViewEvent::Updated(view));
                }
                ThrottleAction::CatchUp(view) => {
                    if let Some(host) = self.me.upgrade() {
                        crate::perf::catch_up(&host, view);
                    }
                }
                ThrottleAction::CpuRate { view, .. } => self.emit(ViewEvent::Updated(view)),
            }
        }
        for (view, session, rate) in sessions {
            self.spawn_cpu_rate(view, session, rate);
        }
    }

    fn spawn_cpu_rate(&self, view: ViewId, session: String, rate: f64) {
        let cdp = Arc::clone(&self.cdp);
        self.runtime.spawn(async move {
            let params = json!({ "rate": rate });
            if let Err(error) = cdp
                .call("Emulation.setCPUThrottlingRate", params, Some(&session))
                .await
            {
                tracing::warn!(%view, %error, "the CPU budget was not applied");
            }
        });
    }
}

#[cfg(test)]
#[path = "throttle_tests.rs"]
mod tests;
