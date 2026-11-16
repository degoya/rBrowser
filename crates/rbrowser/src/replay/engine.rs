//! [`CdpReplay`]: `rb_sync::Replay` for Chromium views. Per event and target view: the plan
//! ([`super::plan`]), one prologue in the agent's world ([`super::prologue`]: echo marker,
//! submit guard, resolution, visual viewport), then the `Input.*` commands at the mapped point
//! ([`super::input`]). A miss is the hub's desync (`NotFound`, `NotVisible`).
//!
//! Nothing here logs an event's content: a typed value may be a password (`sensitive`), and
//! the replay sends it but never writes it anywhere.

use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use futures_util::future::try_join_all;
use rb_engine::ViewId;
use rb_sync::{CommandResult, ElementTarget, Replay, ReplayOutcome, ReplayRequest, SyncEvent};

use super::input::{
    Call, click_calls, hover_calls, input_point, insert_text_call, key_call, tap_calls,
};
use super::metrics::{LatencyStats, LatencySummary};
use super::plan::{Action, Gesture, Options, plan};
use super::port::ReplayPort;
use super::prologue::{expression, parse};

/// The Chromium replay of one window.
pub struct CdpReplay {
    port: Arc<dyn ReplayPort>,
    latency: LatencyStats,
}

impl CdpReplay {
    /// The replay over `port`.
    pub fn new(port: Arc<dyn ReplayPort>) -> Self {
        Self {
            port,
            latency: LatencyStats::default(),
        }
    }

    /// The debug metric: the time per replay, from its turn in the view's queue to the last
    /// `Input.*` command answered.
    pub fn latency(&self) -> LatencySummary {
        self.latency.summary()
    }

    /// Replays `event` into `view` - the hub's replay and MCP's `click`, `type`, `hover` and
    /// `press_key` alike. `top`: the event came from a top frame; `block_submit`: the view
    /// must not submit a form because of it.
    pub async fn replay_into(
        &self,
        view: ViewId,
        event: &SyncEvent,
        top: bool,
        block_submit: bool,
    ) -> ReplayOutcome {
        let started = Instant::now();
        match self.run(view, event, top, block_submit).await {
            Ok(true) => {
                self.record(started.elapsed());
                ReplayOutcome::Done
            }
            Ok(false) => ReplayOutcome::Done,
            Err(outcome) => outcome,
        }
    }

    fn record(&self, took: Duration) {
        if let Some(summary) = self.latency.record(took) {
            tracing::debug!(
                samples = summary.samples,
                p50_ms = summary.p50_ms,
                p95_ms = summary.p95_ms,
                max_ms = summary.max_ms,
                "replay latency"
            );
        }
    }

    /// `Ok(true)` when input was sent, `Ok(false)` when the event needs none.
    async fn run(
        &self,
        view: ViewId,
        event: &SyncEvent,
        top: bool,
        block_submit: bool,
    ) -> Result<bool, ReplayOutcome> {
        if !top {
            // An iframe's point needs its frame's agent and offset: not yet.
            return Err(ReplayOutcome::Unsupported);
        }
        let traits = self
            .port
            .traits(view)
            .ok_or_else(|| ReplayOutcome::Failed("the view is gone".to_owned()))?;
        // Scroll and wheel: scroll sync (RB-020-04), per `sync.scrollMode`.
        if let Some(result) =
            crate::scroll_sync::replay::replay(self.port.as_ref(), view, event).await
        {
            return result;
        }
        let options = Options {
            touch: traits.touch,
            context_menu: matches!(event, SyncEvent::Contextmenu(_)) && self.port.context_menu(),
        };
        let calls = match plan(event, options) {
            Action::Skip => return Ok(false),
            Action::Unsupported => return Err(ReplayOutcome::Unsupported),
            Action::Key(params) => {
                self.prologue(view, None, block_submit).await?;
                vec![key_call(params)]
            }
            Action::Pointer { target, gesture } => {
                let (x, y) = self.prologue(view, Some(&target), block_submit).await?;
                match gesture {
                    Gesture::Click {
                        button,
                        count,
                        modifiers,
                    } => click_calls(x, y, button, count, modifiers),
                    Gesture::Hover => hover_calls(x, y),
                    Gesture::Tap { modifiers } => tap_calls(x, y, modifiers),
                }
            }
            Action::Text { target, text } => {
                self.prologue(view, Some(&target), block_submit).await?;
                vec![insert_text_call(&text)]
            }
            Action::Form(form) => {
                return crate::form_sync::replay(self.port.as_ref(), view, form, block_submit)
                    .await;
            }
        };
        self.send(view, calls).await?;
        Ok(true)
    }

    /// The prologue in `view`; with a target, the point to send the input to.
    async fn prologue(
        &self,
        view: ViewId,
        target: Option<&ElementTarget>,
        block_submit: bool,
    ) -> Result<(f64, f64), ReplayOutcome> {
        let source = expression(target, block_submit).map_err(failed)?;
        let value = self.port.world(view, source).await.map_err(failed)?;
        let answer = parse(&value).map_err(ReplayOutcome::Failed)?;
        match (target, answer.resolved) {
            (None, _) => Ok((0.0, 0.0)),
            (Some(_), Some(CommandResult::Ok { x, y, .. })) => {
                Ok(input_point(x, y, answer.viewport))
            }
            (Some(_), Some(CommandResult::NotFound)) => Err(ReplayOutcome::NotFound),
            (Some(_), Some(CommandResult::NotVisible)) => Err(ReplayOutcome::NotVisible),
            (Some(_), Some(CommandResult::Error { .. })) => Err(ReplayOutcome::Failed(
                "the sync agent refused the target".to_owned(),
            )),
            (Some(_), _) => Err(ReplayOutcome::Failed(
                "the sync agent did not resolve the target".to_owned(),
            )),
        }
    }

    /// Sends an event's commands at once, in order, and waits for all: CDP runs a session's
    /// commands in order and Chromium queues their input in order, so the event costs one
    /// round trip, not one per command (K1). The first poll of each send writes its frame.
    async fn send(&self, view: ViewId, calls: Vec<Call>) -> Result<(), ReplayOutcome> {
        let sends = calls
            .into_iter()
            .map(|call| self.port.send(view, call.method, call.params));
        try_join_all(sends).await.map_err(failed)?;
        Ok(())
    }
}

/// An engine error as a failed replay: CDP's error names the method and its reason, never the
/// parameters.
fn failed(error: rb_engine::EngineError) -> ReplayOutcome {
    ReplayOutcome::Failed(error.to_string())
}

#[async_trait]
impl Replay for CdpReplay {
    async fn dispatch(&self, request: ReplayRequest) -> ReplayOutcome {
        if request.target == request.source {
            // Never into the leader: its own input is the original.
            return ReplayOutcome::Unsupported;
        }
        self.replay_into(
            request.target,
            &request.event,
            request.top,
            request.block_submit,
        )
        .await
    }
}

#[cfg(test)]
#[path = "engine_tests.rs"]
mod tests;
