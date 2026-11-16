//! [`WebkitReplay`]: the SyncHub's `rb_sync::Replay` for WebKit views (RB-040-04). The view's
//! agent resolves the target first (`webkit.view.agent`, `resolve`), then the action runs as
//! real input through Playwright's mouse and keyboard, after the agent's replay marker (no echo).
//!
//! | Event | In WebKit |
//! | --- | --- |
//! | `click` | resolve, then move, press, release with the click count (no back/forward button) |
//! | `click` with `detail` 0, `dblclick` | nothing: the key or the click with count 2 makes it |
//! | `hover` | resolve, then a move; nothing in a touch view |
//! | `keydown`, `keyup` | the key; keys Playwright cannot name (`Dead`, `Process`) are skipped |
//! | `input` `insertText` | resolve the field, then the text |
//! | `scroll` of the document | the same share of the view's scroll range |
//! | in an iframe; `contextmenu`, `pointer`, `wheel`, element `scroll`, `change`, `focus`, `submit` | unsupported: skipped without desync |
//!
//! A missing or hidden target desyncs the view ("desynced" badge), as in Chromium.

use std::sync::{Arc, Weak};

use async_trait::async_trait;
use rb_engine::{Button, EngineError, InputEvent, Phase};
use rb_sync::{
    AgentCommand, CommandResult, ElementTarget, MouseButton, Replay, ReplayOutcome, ReplayRequest,
    SyncEvent,
};

use super::engine::WebkitEngine;
use super::view::WebkitView;

/// How long the target's agent captures nothing after a replay (its echo).
const QUIET_MS: u32 = 300;
/// How long a submit is blocked in a view that must not submit (leader-only submit).
const BLOCK_SUBMIT_MS: u32 = 1000;
/// The `inputType` whose text is replayed.
const INSERT_TEXT: &str = "insertText";
/// Keys with no name Playwright knows.
const UNNAMED_KEYS: [&str; 4] = ["Unidentified", "Dead", "Process", ""];

/// What one event becomes.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    /// Nothing to send; counts as replayed.
    Skip,
    /// Not replayed in WebKit; skipped without desync.
    Unsupported,
    /// Resolve `target`, then these events at its point (`x`, `y` are filled in).
    AtTarget {
        /// Where.
        target: ElementTarget,
        /// What, at the point.
        gesture: Gesture,
    },
    /// Input that needs no target.
    Input(Vec<InputEvent>),
    /// The document's scroll position and range.
    Scroll {
        /// `scrollLeft`.
        x: f64,
        /// `scrollTop`.
        y: f64,
        /// The largest `scrollLeft`.
        max_x: f64,
        /// The largest `scrollTop`.
        max_y: f64,
    },
}

/// What happens at a resolved point.
#[derive(Clone, Debug, PartialEq)]
pub enum Gesture {
    /// Press and release.
    Click {
        /// The button.
        button: Button,
        /// The click count.
        count: u32,
        /// CDP modifier bits.
        modifiers: u8,
    },
    /// A move only.
    Hover,
    /// Typed text (the field must be the target).
    Text(String),
}

/// What `event` becomes in a view; `touch` for a touch view.
pub fn plan(request: &ReplayRequest, touch: bool) -> Action {
    if !request.top {
        return Action::Unsupported;
    }
    match &*request.event {
        SyncEvent::Click(click) if click.detail == 0 => Action::Skip,
        SyncEvent::Click(click) => match button(click.button) {
            Some(button) => Action::AtTarget {
                target: click.target.clone(),
                gesture: Gesture::Click {
                    button,
                    count: click.detail.min(3),
                    modifiers: click.modifiers,
                },
            },
            None => Action::Unsupported,
        },
        SyncEvent::Dblclick(_) => Action::Skip,
        SyncEvent::Hover(_) if touch => Action::Skip,
        SyncEvent::Hover(hover) => Action::AtTarget {
            target: hover.target.clone(),
            gesture: Gesture::Hover,
        },
        SyncEvent::Keydown(key) | SyncEvent::Keyup(key) => {
            if UNNAMED_KEYS.contains(&key.key.as_str()) {
                return Action::Skip;
            }
            let phase = if matches!(*request.event, SyncEvent::Keydown(_)) {
                Phase::Down
            } else {
                Phase::Up
            };
            Action::Input(vec![InputEvent::Key {
                phase,
                key: key.key.clone(),
                code: key.code.clone(),
                text: None,
                modifiers: key.modifiers,
            }])
        }
        SyncEvent::Input(input) => match input.data.as_deref() {
            Some(text) if input.input_type == INSERT_TEXT && !text.is_empty() => Action::AtTarget {
                target: input.target.clone(),
                gesture: Gesture::Text(text.to_owned()),
            },
            _ => Action::Skip,
        },
        SyncEvent::Scroll(scroll) if scroll.container.is_none() => Action::Scroll {
            x: scroll.x,
            y: scroll.y,
            max_x: scroll.max_x,
            max_y: scroll.max_y,
        },
        SyncEvent::Contextmenu(_)
        | SyncEvent::Pointer(_)
        | SyncEvent::Wheel(_)
        | SyncEvent::Scroll(_)
        | SyncEvent::Change(_)
        | SyncEvent::Submit(_)
        | SyncEvent::Focus(_) => Action::Unsupported,
    }
}

/// Playwright's button; it has none for back and forward.
fn button(button: MouseButton) -> Option<Button> {
    match button {
        MouseButton::None => Some(Button::None),
        MouseButton::Left => Some(Button::Left),
        MouseButton::Middle => Some(Button::Middle),
        MouseButton::Right => Some(Button::Right),
        MouseButton::Back | MouseButton::Forward => None,
    }
}

/// The input of `gesture` at (`x`, `y`).
pub fn gesture_input(gesture: &Gesture, x: f64, y: f64) -> Vec<InputEvent> {
    match gesture {
        Gesture::Click {
            button,
            count,
            modifiers,
        } => [Phase::Down, Phase::Up]
            .into_iter()
            .map(|phase| InputEvent::Mouse {
                phase,
                x,
                y,
                button: *button,
                clicks: *count,
                modifiers: *modifiers,
            })
            .collect(),
        Gesture::Hover => vec![InputEvent::Mouse {
            phase: Phase::Move,
            x,
            y,
            button: Button::None,
            clicks: 0,
            modifiers: 0,
        }],
        Gesture::Text(text) => vec![InputEvent::Text { text: text.clone() }],
    }
}

/// The replay into the WebKit views of one engine.
pub struct WebkitReplay {
    engine: Weak<WebkitEngine>,
}

impl WebkitReplay {
    /// The replay of `engine`'s views.
    pub fn new(engine: &Arc<WebkitEngine>) -> Self {
        Self {
            engine: Arc::downgrade(engine),
        }
    }

    async fn run(&self, view: &WebkitView, request: &ReplayRequest) -> ReplayOutcome {
        let action = plan(request, view.emulation().touch);
        let quiet = AgentCommand::Replay { ms: QUIET_MS };
        let block = if request.block_submit {
            BLOCK_SUBMIT_MS
        } else {
            0
        };
        let sent = match action {
            Action::Skip => return ReplayOutcome::Done,
            Action::Unsupported => return ReplayOutcome::Unsupported,
            Action::Input(events) => view.input(&events, Some(&quiet), block).await,
            Action::Scroll { x, y, max_x, max_y } => view.scroll(x, y, max_x, max_y).await,
            Action::AtTarget { target, gesture } => {
                let resolved = view.agent(&AgentCommand::Resolve { target }).await;
                let (x, y) = match resolved {
                    Ok(CommandResult::Ok { x, y, .. }) => (x, y),
                    Ok(CommandResult::NotFound) => return ReplayOutcome::NotFound,
                    Ok(CommandResult::NotVisible) => return ReplayOutcome::NotVisible,
                    Ok(CommandResult::Error { message }) => return ReplayOutcome::Failed(message),
                    Ok(CommandResult::Done) => {
                        return ReplayOutcome::Failed("resolve answered without a point".into());
                    }
                    Err(error) => return failed(error),
                };
                view.input(&gesture_input(&gesture, x, y), Some(&quiet), block)
                    .await
            }
        };
        match sent {
            Ok(()) => ReplayOutcome::Done,
            Err(error) => failed(error),
        }
    }
}

fn failed(error: EngineError) -> ReplayOutcome {
    match error {
        // The window went away: the router stops sending it events.
        EngineError::Closed => ReplayOutcome::Unsupported,
        other => ReplayOutcome::Failed(other.to_string()),
    }
}

#[async_trait]
impl Replay for WebkitReplay {
    async fn dispatch(&self, request: ReplayRequest) -> ReplayOutcome {
        let view = self
            .engine
            .upgrade()
            .and_then(|engine| engine.view(request.target));
        match view {
            Some(view) => self.run(&view, &request).await,
            None => ReplayOutcome::Unsupported,
        }
    }
}

#[cfg(test)]
#[path = "replay_tests.rs"]
mod tests;
