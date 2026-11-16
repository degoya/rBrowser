//! The interval reload of [`super::HotReload`]: its schedule for the options and environment
//! in effect, and the task that reloads when due and nobody types (RB-030-05).

use std::sync::Arc;
use std::time::{Duration, Instant};

use rb_engine::Target;

use super::super::interval::{IntervalReload, Tick};
use super::super::perform::{HowTo, Work, evaluate, perform};
use super::super::script::TYPING_PROBE;
use super::{HotReload, INTERVAL_LOOK, Inner};

impl HotReload {
    pub(super) fn spawn_interval(&self) {
        let weak = Arc::downgrade(&self.inner);
        self.inner.runtime.spawn(async move {
            loop {
                let Some(inner) = weak.upgrade() else {
                    return;
                };
                let wait = inner.interval_step().await;
                drop(inner);
                tokio::time::sleep(wait).await;
            }
        });
    }
}

impl Inner {
    /// The interval reload for the options and environment in effect.
    pub(super) fn reset_interval(&self) {
        {
            let mut state = self.state();
            let period = state.options.interval(state.environment.as_deref());
            let pause = state.options.interval_pause();
            let keep = state.interval.as_ref().map(IntervalReload::period) == period;
            if !keep {
                state.interval =
                    period.map(|period| IntervalReload::new(period, pause, Instant::now()));
            }
        }
        self.publish();
    }

    /// One look of the interval task: reloads when due and nobody types; how long to sleep.
    pub(super) async fn interval_step(&self) -> Duration {
        {
            let mut state = self.state();
            let tick = match &state.interval {
                _ if state.interval_busy => return INTERVAL_LOOK,
                Some(interval) => interval.tick(Instant::now()),
                None => return INTERVAL_LOOK,
            };
            match tick {
                Tick::Wait(wait) => return wait.min(INTERVAL_LOOK),
                Tick::Due if state.paused => return INTERVAL_LOOK,
                Tick::Due => state.interval_busy = true,
            }
        }
        if self.someone_types().await {
            if let Some(interval) = self.state().interval.as_mut() {
                interval.note_input(Instant::now());
            }
        } else {
            let how = HowTo::default();
            perform(self.host.as_ref(), Target::All, &Work::Full, how).await;
            self.reloaded();
        }
        self.state().interval_busy = false;
        INTERVAL_LOOK
    }

    /// Whether a view has a focused text field (its page has focus).
    async fn someone_types(&self) -> bool {
        for view in self.host.views() {
            if view.engine == rb_engine::EngineKind::Chromium
                && evaluate(self.host.as_ref(), view.id, TYPING_PROBE).await
                    == Some(serde_json::Value::Bool(true))
            {
                return true;
            }
        }
        false
    }
}
