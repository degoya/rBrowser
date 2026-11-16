//! The agent's execution contexts: CDP reports one per frame and document for the isolated world
//! (`Runtime.executionContextCreated` with `name: "rbrowser"`); a command for the agent of a
//! frame is evaluated in its context.

use std::collections::HashMap;

/// Per CDP session: frame id -> the context of the agent's world in it.
#[derive(Debug, Default)]
pub(crate) struct ContextTable {
    sessions: HashMap<String, HashMap<String, i64>>,
}

impl ContextTable {
    /// The agent's world was created in `frame` (a new document replaces the old one's).
    pub(crate) fn insert(&mut self, session: &str, frame: &str, context: i64) {
        self.sessions
            .entry(session.to_owned())
            .or_default()
            .insert(frame.to_owned(), context);
    }

    /// `context` was destroyed.
    pub(crate) fn remove(&mut self, session: &str, context: i64) {
        if let Some(frames) = self.sessions.get_mut(session) {
            frames.retain(|_, id| *id != context);
        }
    }

    /// Every context of `session` is gone (navigation, detach).
    pub(crate) fn clear(&mut self, session: &str) {
        self.sessions.remove(session);
    }

    /// The agent's context in `frame`.
    pub(crate) fn context_of(&self, session: &str, frame: &str) -> Option<i64> {
        self.sessions.get(session)?.get(frame).copied()
    }

    /// The frame of `context`.
    pub(crate) fn frame_of(&self, session: &str, context: i64) -> Option<&str> {
        self.sessions
            .get(session)?
            .iter()
            .find(|(_, id)| **id == context)
            .map(|(frame, _)| frame.as_str())
    }

    /// Every agent context of `session`, in no particular order.
    pub(crate) fn contexts(&self, session: &str) -> Vec<i64> {
        self.sessions
            .get(session)
            .map(|frames| frames.values().copied().collect())
            .unwrap_or_default()
    }
}

#[cfg(test)]
#[path = "contexts_tests.rs"]
mod tests;
