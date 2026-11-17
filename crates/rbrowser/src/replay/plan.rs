//! What a captured event becomes in a Chromium view (RB-020-03): an [`Action`].
//!
//! | Event | Action |
//! | --- | --- |
//! | `click` | resolve, then move, press, release with the click count; a tap in a touch view |
//! | `click` with `detail` 0 | nothing: the engine made it from a key (Enter, Space) or a label, and the key replays it |
//! | `dblclick` | nothing: the click with count 2 makes it |
//! | `contextmenu` | resolve, then a right click - only with `sync.contextMenu` |
//! | `hover` | resolve, then one mouse move; not in touch views |
//! | `keydown`, `keyup` | `Input.dispatchKeyEvent` ([`super::keys`]) |
//! | `input` `insertText` | resolve, then `Input.insertText` with its data |
//! | other `input` types | nothing: the key that made them (Backspace, Ctrl+V, Enter) replays it |
//! | `pointer` | unsupported: the click replays it; drags and press-and-hold come later |
//! | `wheel`, `scroll` | not planned here: `CdpReplay` hands them to `crate::scroll_sync` first (RB-020-04) |
//! | `change`, `focus` | form sync's ([`crate::form_sync`], RB-020-05): the agent sets the state |
//! | `submit` | unsupported: the hub's follow |
//!
//! "Nothing" counts as replayed; "unsupported" is skipped without a desync.

use rb_sync::{ElementTarget, MouseButton, SyncEvent};
use serde_json::Value;

use super::keys::{KeyInput, KeyPhase, key_event, replay_text};
use crate::form_sync::{self, FormAction};

/// The `inputType` whose text is replayed.
const INSERT_TEXT: &str = "insertText";

/// How a view replays.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Options {
    /// The view emulates touch: a click is a tap.
    pub touch: bool,
    /// `sync.contextMenu`: right clicks are replayed.
    pub context_menu: bool,
}

/// What the pointer does at the resolved point.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gesture {
    /// Move, press, release.
    Click {
        /// The button.
        button: MouseButton,
        /// The click count.
        count: u32,
        /// CDP modifier bits.
        modifiers: u8,
    },
    /// Move only.
    Hover,
    /// Touch start and end.
    Tap {
        /// CDP modifier bits.
        modifiers: u8,
    },
}

/// What to do in the view.
#[derive(Clone, PartialEq)]
pub enum Action {
    /// Nothing to send; the event counts as replayed.
    Skip,
    /// This engine does not replay the event: skipped, no desync.
    Unsupported,
    /// Resolve `target`, then the gesture at its point.
    Pointer {
        /// Where.
        target: ElementTarget,
        /// What.
        gesture: Gesture,
    },
    /// `Input.dispatchKeyEvent` with these parameters; nothing to resolve.
    Key(Value),
    /// Resolve `target` (the field), then `Input.insertText`.
    Text {
        /// The field.
        target: ElementTarget,
        /// What was typed.
        text: String,
    },
    /// Form sync (RB-020-05): the agent sets a control's state or the focus.
    Form(FormAction),
}

/// The text never shows: it may be a password.
impl std::fmt::Debug for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Skip => f.write_str("Skip"),
            Self::Unsupported => f.write_str("Unsupported"),
            Self::Pointer { gesture, .. } => f.debug_tuple("Pointer").field(gesture).finish(),
            Self::Key(_) => f.write_str("Key"),
            Self::Text { .. } => f.write_str("Text"),
            Self::Form(form) => f.debug_tuple("Form").field(form).finish(),
        }
    }
}

/// What `event` becomes in a view with `options`.
pub fn plan(event: &SyncEvent, options: Options) -> Action {
    match event {
        SyncEvent::Click(click) => {
            if click.detail == 0 || (options.touch && click.detail > 1) {
                // detail 0: made by a key or a label, which replays itself. A second tap in a
                // touch view would be a double-tap zoom (ADR 0001 addendum).
                return Action::Skip;
            }
            let gesture = if options.touch {
                Gesture::Tap {
                    modifiers: click.modifiers,
                }
            } else {
                Gesture::Click {
                    button: click.button,
                    count: click.detail,
                    modifiers: click.modifiers,
                }
            };
            Action::Pointer {
                target: click.target.clone(),
                gesture,
            }
        }
        SyncEvent::Dblclick(_) => Action::Skip,
        SyncEvent::Contextmenu(_) if options.touch => Action::Unsupported,
        SyncEvent::Contextmenu(_) if !options.context_menu => Action::Skip,
        SyncEvent::Contextmenu(menu) => Action::Pointer {
            target: menu.target.clone(),
            gesture: Gesture::Click {
                button: MouseButton::Right,
                count: 1,
                modifiers: menu.modifiers,
            },
        },
        SyncEvent::Hover(_) if options.touch => Action::Unsupported,
        SyncEvent::Hover(hover) => Action::Pointer {
            target: hover.target.clone(),
            gesture: Gesture::Hover,
        },
        SyncEvent::Keydown(key) => {
            let input = KeyInput::from(key);
            Action::Key(key_event(&input, KeyPhase::Down, replay_text(&key.key)))
        }
        SyncEvent::Keyup(key) => Action::Key(key_event(&KeyInput::from(key), KeyPhase::Up, None)),
        SyncEvent::Input(input) => match input.data.as_deref() {
            Some(text) if input.input_type == INSERT_TEXT && !text.is_empty() => Action::Text {
                target: input.target.clone(),
                text: text.to_owned(),
            },
            _ => Action::Skip,
        },
        SyncEvent::Change(_) | SyncEvent::Focus(_) => {
            form_sync::plan(event).map_or(Action::Skip, Action::Form)
        }
        SyncEvent::Pointer(_)
        | SyncEvent::Wheel(_)
        | SyncEvent::Scroll(_)
        | SyncEvent::Submit(_) => Action::Unsupported,
    }
}

#[cfg(test)]
#[path = "plan_tests.rs"]
mod tests;
