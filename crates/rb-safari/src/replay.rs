//! [`SafariReplay`]: the SyncHub's `rb_sync::Replay` for Safari views (RB-095-03), the way
//! Firefox's replays (`rb_bidi::firefox::replay`): each event first sets the agent's replay
//! marker, then clicks, hover and wheel resolve their target through the agent and replay as
//! WebDriver actions (`POST /actions`, CSS px of the viewport); keys replay as keys with the held
//! modifiers brought in step ([`HeldKeys`]); scroll, input, change and focus are the agent's own
//! commands - one resolver for every engine. A target the agent does not find is
//! [`ReplayOutcome::NotFound`]: the view is desynced as in Chromium. With `block_submit` the
//! view's submits stop for a moment. Iframe events are skipped until frame offsets reach
//! Safari. When another view leads, what the replay held is released first. A Safari view is
//! never a source: Safari locks a person's input in its automated windows. iOS views
//! (RB-095-04) replay a click as a touch and take no hover and no wheel.
//!
//! [`HeldKeys`]: rb_bidi::input::HeldKeys

use std::sync::{Arc, Weak};

use async_trait::async_trait;
use rb_engine::{Button, EngineError, InputEvent, Phase};
use rb_sync::{
    AgentCommand, ChangeData, CommandResult, ElementTarget, MouseButton, Replay, ReplayOutcome,
    ReplayRequest, SyncEvent, is_file_skip,
};
use rb_util::lock;
use serde_json::Value;

use crate::agent::{self, BLOCK_SUBMIT};
use crate::engine::SafariEngine;
use crate::view::SafariView;

/// How long the agent captures nothing around a replayed event.
const MARKER_MS: u32 = 500;
/// How long a follower's form submits are stopped after a replayed click or Enter.
const BLOCK_SUBMIT_MS: u32 = 1000;
/// CSS px per wheel line and per page (`WheelEvent.deltaMode` 1 and 2).
const LINE_PX: f64 = 40.0;
const PAGE_PX: f64 = 800.0;

/// The replay of a Safari engine's views.
pub struct SafariReplay {
    engine: Weak<SafariEngine>,
}

impl SafariReplay {
    /// The replay into `engine`'s views.
    pub fn new(engine: &Arc<SafariEngine>) -> Self {
        Self {
            engine: Arc::downgrade(engine),
        }
    }
}

#[async_trait]
impl Replay for SafariReplay {
    async fn dispatch(&self, request: ReplayRequest) -> ReplayOutcome {
        let Some(view) = self
            .engine
            .upgrade()
            .and_then(|engine| engine.view(request.target))
        else {
            return ReplayOutcome::Unsupported;
        };
        if !request.top {
            return ReplayOutcome::Unsupported;
        }
        let Ok(_turn) = view.turn().await else {
            return ReplayOutcome::Unsupported;
        };
        let new_leader = {
            let mut leader = lock(&view.leader);
            let changed = leader.is_some_and(|leader| leader != request.source);
            *leader = Some(request.source);
            changed
        };
        if new_leader {
            view.release_input().await;
        }
        replay(&view, &request)
            .await
            .unwrap_or_else(|error| ReplayOutcome::Failed(error.to_string()))
    }
}

/// Replays `request` into `view`, within its turn.
pub async fn replay(
    view: &SafariView,
    request: &ReplayRequest,
) -> Result<ReplayOutcome, EngineError> {
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
        // A phone has no hover and no wheel; the scroll follows through `Scroll` (RB-095-04).
        SyncEvent::Hover(_) | SyncEvent::Wheel(_) if view.ios() => Ok(ReplayOutcome::Unsupported),
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
            let sent = view.session().perform_actions(vec![keyboard]).await;
            view.checked(sent)?;
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

/// The agent captures nothing for a moment. A view whose agent did not answer still replays.
async fn marker(view: &SafariView) {
    let command = AgentCommand::Replay { ms: MARKER_MS };
    if let Err(error) = agent::command(view.session(), &view.agent, &command).await {
        tracing::debug!(%error, "replay marker not set");
    }
}

/// The view's form submits are stopped for a moment.
async fn block_submit(view: &SafariView) -> Result<(), EngineError> {
    let ms = Value::from(BLOCK_SUBMIT_MS);
    let blocked = view.session().execute(BLOCK_SUBMIT, vec![ms]).await;
    view.checked(blocked).map(drop)
}

/// The point of `target` in the view, or the outcome when there is none.
async fn resolve(
    view: &SafariView,
    target: &ElementTarget,
) -> Result<Result<(f64, f64), ReplayOutcome>, EngineError> {
    let command = AgentCommand::Resolve {
        target: target.clone(),
    };
    let answer = view.checked(agent::command(view.session(), &view.agent, &command).await)?;
    Ok(match answer {
        CommandResult::Ok { x, y, .. } => Ok((x, y)),
        CommandResult::Done => Err(ReplayOutcome::Failed(
            "the agent answered resolve without a point".to_owned(),
        )),
        other => Err(outcome_of(other)),
    })
}

/// Runs the agent's `command` in the view; its answer is the outcome.
async fn run(view: &SafariView, command: &AgentCommand) -> Result<ReplayOutcome, EngineError> {
    let answer = view.checked(agent::command(view.session(), &view.agent, command).await)?;
    Ok(outcome_of(answer))
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

async fn perform(view: &SafariView, input: &InputEvent) -> Result<(), EngineError> {
    // On iOS a click is a touch; hover and the wheel have none (RB-095-04).
    let sources = view.sources_for(input)?;
    if sources.is_empty() {
        return Ok(());
    }
    view.checked(view.session().perform_actions(sources).await)
}

#[cfg(test)]
#[path = "replay_tests.rs"]
mod tests;
