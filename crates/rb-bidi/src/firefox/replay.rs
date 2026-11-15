//! [`FirefoxReplay`]: the SyncHub's `rb_sync::Replay` for Firefox views (RB-040-02). Each event
//! first sets the agent's replay marker (no echo), then: clicks, hover and wheel resolve their
//! target through the agent and replay as real input (`input.performActions`, CSS px); keys
//! replay as keys, with the modifiers Firefox holds brought in step with the event's first
//! ([`HeldKeys`]); scroll, input, change and focus are the agent's own commands (`scroll`,
//! `apply`, `focus`), as in Chromium - one resolver for every engine, role and text selectors
//! and the scroll anchor included. With `block_submit` the view's form submits are stopped for
//! a moment: the leader submits, the view follows by URL. Events from iframes are skipped
//! (`Unsupported`) until the frame offsets of RB-020-03 reach Firefox. When another view leads,
//! every key and button still held is released first (`input.releaseActions`).
//!
//! [`HeldKeys`]: crate::input::HeldKeys

use std::sync::Weak;

use async_trait::async_trait;
use rb_engine::{Button, InputEvent, Phase};
use rb_sync::{
    AgentCommand, ChangeData, CommandResult, ElementTarget, MouseButton, Replay, ReplayOutcome,
    ReplayRequest, SyncEvent, is_file_skip,
};
use serde_json::json;

use super::agent::{self, BLOCK_SUBMIT, page};
use super::engine::FirefoxEngine;
use super::view::ViewState;
use crate::client::lock;
use crate::error::BidiError;
use crate::input::{actions_for, perform_actions};

/// How long the agent captures nothing around a replayed event.
const MARKER_MS: u32 = 500;
/// How long a follower's form submits are stopped after a replayed click or Enter.
const BLOCK_SUBMIT_MS: u32 = 1000;
/// CSS px per wheel line and per page (`WheelEvent.deltaMode` 1 and 2).
const LINE_PX: f64 = 40.0;
const PAGE_PX: f64 = 800.0;

/// The replay of a Firefox engine's views.
pub struct FirefoxReplay {
    engine: Weak<FirefoxEngine>,
}

impl FirefoxReplay {
    pub(crate) fn new(engine: Weak<FirefoxEngine>) -> Self {
        Self { engine }
    }
}

#[async_trait]
impl Replay for FirefoxReplay {
    async fn dispatch(&self, request: ReplayRequest) -> ReplayOutcome {
        let Some(view) = self
            .engine
            .upgrade()
            .and_then(|engine| engine.view_state(request.target))
        else {
            return ReplayOutcome::Unsupported;
        };
        if !request.top {
            return ReplayOutcome::Unsupported;
        }
        let new_leader = {
            let mut leader = lock(&view.leader);
            let changed = leader.is_some_and(|leader| leader != request.source);
            *leader = Some(request.source);
            changed
        };
        if new_leader {
            view.release_input().await;
            *lock(&view.leader) = Some(request.source);
        }
        replay(&view, &request)
            .await
            .unwrap_or_else(|error| ReplayOutcome::Failed(error.to_string()))
    }
}

/// Replays `request` into `view`.
pub async fn replay(view: &ViewState, request: &ReplayRequest) -> Result<ReplayOutcome, BidiError> {
    let block = request.block_submit;
    match &*request.event {
        // A click replays from `click`; the submit itself replays nowhere (leader only).
        SyncEvent::Pointer(_) | SyncEvent::Dblclick(_) | SyncEvent::Submit(_) => {
            Ok(ReplayOutcome::Unsupported)
        }
        SyncEvent::Click(mouse) | SyncEvent::Contextmenu(mouse) => {
            let button = if matches!(*request.event, SyncEvent::Contextmenu(_)) {
                Button::Right
            } else {
                match mouse.button {
                    MouseButton::Left => Button::Left,
                    MouseButton::Middle => Button::Middle,
                    MouseButton::Right => Button::Right,
                    MouseButton::None | MouseButton::Back | MouseButton::Forward => {
                        return Ok(ReplayOutcome::Unsupported);
                    }
                }
            };
            marker(view).await;
            let (x, y) = match resolve(view, &mouse.target).await? {
                Ok(point) => point,
                Err(outcome) => return Ok(outcome),
            };
            if block {
                block_submit(view).await?;
            }
            for phase in [Phase::Down, Phase::Up] {
                let input = InputEvent::Mouse {
                    phase,
                    x,
                    y,
                    button,
                    clicks: mouse.detail.max(1),
                    modifiers: mouse.modifiers,
                };
                perform(view, &input).await?;
            }
            // The click pressed and released its modifiers itself.
            lock(&view.held).clicked(mouse.modifiers);
            Ok(ReplayOutcome::Done)
        }
        SyncEvent::Hover(hover) => {
            marker(view).await;
            let (x, y) = match resolve(view, &hover.target).await? {
                Ok(point) => point,
                Err(outcome) => return Ok(outcome),
            };
            let input = InputEvent::Mouse {
                phase: Phase::Move,
                x,
                y,
                button: Button::None,
                clicks: 0,
                modifiers: 0,
            };
            perform(view, &input).await?;
            Ok(ReplayOutcome::Done)
        }
        SyncEvent::Wheel(wheel) => {
            marker(view).await;
            let (x, y) = match resolve(view, &wheel.target).await? {
                Ok(point) => point,
                Err(outcome) => return Ok(outcome),
            };
            let scale = match wheel.delta_mode {
                1 => LINE_PX,
                2 => PAGE_PX,
                _ => 1.0,
            };
            let input = InputEvent::Wheel {
                x,
                y,
                delta_x: wheel.delta_x * scale,
                delta_y: wheel.delta_y * scale,
            };
            perform(view, &input).await?;
            Ok(ReplayOutcome::Done)
        }
        SyncEvent::Scroll(scroll) => {
            marker(view).await;
            run(view, &AgentCommand::Scroll(scroll.clone())).await
        }
        SyncEvent::Keydown(key) | SyncEvent::Keyup(key) => {
            let down = matches!(*request.event, SyncEvent::Keydown(_));
            let Some(keyboard) = lock(&view.held).key(&key.key, &key.code, down, key.modifiers)
            else {
                return Ok(ReplayOutcome::Unsupported);
            };
            marker(view).await;
            if block && down {
                block_submit(view).await?;
            }
            view.client
                .send(perform_actions(&view.context, vec![keyboard]))
                .await?;
            Ok(ReplayOutcome::Done)
        }
        SyncEvent::Input(input) => {
            marker(view).await;
            // The replayed keys typed the text already; the agent sets the value only when it
            // still differs (an IME, autocorrect, a paste).
            let state = ChangeData {
                value: input.value.clone(),
                checked: None,
                selected: None,
                sensitive: input.sensitive,
                target: input.target.clone(),
            };
            run(view, &AgentCommand::Apply(state)).await
        }
        // A file input carries no state: nothing to set, nothing desyncs.
        SyncEvent::Change(change) if is_file_skip(change) => Ok(ReplayOutcome::Done),
        SyncEvent::Change(change) => {
            marker(view).await;
            run(view, &AgentCommand::Apply(change.clone())).await
        }
        SyncEvent::Focus(focus) => {
            marker(view).await;
            let command = AgentCommand::Focus {
                target: focus.target.clone(),
            };
            run(view, &command).await
        }
    }
}

/// The agent captures nothing for a moment, so the replay causes no echo. A view without an
/// agent still replays.
async fn marker(view: &ViewState) {
    let command = AgentCommand::Replay { ms: MARKER_MS };
    if let Err(error) = agent::command(&view.client, &view.context, &command).await {
        tracing::debug!(view = %view.id, %error, "replay marker not set");
    }
}

/// The view's form submits are stopped for a moment.
async fn block_submit(view: &ViewState) -> Result<(), BidiError> {
    page(
        &view.client,
        &view.context,
        BLOCK_SUBMIT,
        &json!({ "ms": BLOCK_SUBMIT_MS }),
    )
    .await
    .map(drop)
}

/// The point of `target` in the view, or the outcome when there is none.
async fn resolve(
    view: &ViewState,
    target: &ElementTarget,
) -> Result<Result<(f64, f64), ReplayOutcome>, BidiError> {
    let command = AgentCommand::Resolve {
        target: target.clone(),
    };
    Ok(
        match agent::command(&view.client, &view.context, &command).await? {
            CommandResult::Ok { x, y, .. } => Ok((x, y)),
            CommandResult::Done => Err(ReplayOutcome::Failed(
                "the agent answered resolve without a point".to_owned(),
            )),
            other => Err(outcome_of(other)),
        },
    )
}

/// Runs the agent's `command` in the view; its answer is the outcome.
async fn run(view: &ViewState, command: &AgentCommand) -> Result<ReplayOutcome, BidiError> {
    Ok(outcome_of(
        agent::command(&view.client, &view.context, command).await?,
    ))
}

/// The replay outcome of an agent's answer. An error's message never quotes a value.
fn outcome_of(result: CommandResult) -> ReplayOutcome {
    match result {
        CommandResult::Ok { .. } | CommandResult::Done => ReplayOutcome::Done,
        CommandResult::NotFound => ReplayOutcome::NotFound,
        CommandResult::NotVisible => ReplayOutcome::NotVisible,
        CommandResult::Error { message } => ReplayOutcome::Failed(message),
    }
}

async fn perform(view: &ViewState, input: &InputEvent) -> Result<(), BidiError> {
    let sources = actions_for(input).map_err(|reason| BidiError::Protocol {
        method: "input.performActions".to_owned(),
        reason,
    })?;
    view.client
        .send(perform_actions(&view.context, sources))
        .await
        .map(drop)
}

#[cfg(test)]
#[path = "replay_tests.rs"]
mod tests;
