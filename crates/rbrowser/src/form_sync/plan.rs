//! What a form event becomes in a follower view (RB-020-05): a [`FormAction`].
//!
//! | Event | Action |
//! | --- | --- |
//! | `change` with a state | the agent's `apply`: the leader's state, set in the view |
//! | `change` without one | skipped: a file input (its path is never captured) - no desync |
//! | `focus` | the agent's `focus`, without scrolling (only with `sync.focus`) |
//!
//! Everything else is input replay's (`crate::replay::plan`).

use rb_sync::{AgentCommand, ChangeData, ElementTarget, SyncEvent, is_file_skip, selector_of};

/// What form sync does in a view. `Debug` masks values (`ChangeData`).
#[derive(Clone, Debug, PartialEq)]
pub enum FormAction {
    /// Set the control to the leader's state.
    Apply(ChangeData),
    /// Focus the element.
    Focus(ElementTarget),
    /// A file input changed: nothing is synced. `field` is its CSS path, for the log.
    FileSkipped {
        /// The input's CSS path (never a value).
        field: String,
    },
}

impl FormAction {
    /// The agent command that carries it out; `None` when nothing is sent.
    pub fn command(&self) -> Option<AgentCommand> {
        match self {
            Self::Apply(change) => Some(AgentCommand::Apply(change.clone())),
            Self::Focus(target) => Some(AgentCommand::Focus {
                target: target.clone(),
            }),
            Self::FileSkipped { .. } => None,
        }
    }
}

/// The form action of `event`; `None` when the event is not form sync's.
pub fn plan(event: &SyncEvent) -> Option<FormAction> {
    match event {
        SyncEvent::Change(change) if is_file_skip(change) => Some(FormAction::FileSkipped {
            field: selector_of(event).unwrap_or_default(),
        }),
        SyncEvent::Change(change) => Some(FormAction::Apply(change.clone())),
        SyncEvent::Focus(focus) => Some(FormAction::Focus(focus.target.clone())),
        _ => None,
    }
}

#[cfg(test)]
#[path = "plan_tests.rs"]
mod tests;
