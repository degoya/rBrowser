//! The desync state: a view that could not follow an event (its target is missing or hidden,
//! or the replay did not answer in time) is desynced. It gets no further events until it
//! navigates successfully or is resynced (it loads the leader's URL); the shell draws a badge
//! in the grid gap above it.

use std::collections::BTreeMap;

use rb_engine::ViewId;
use serde::{Deserialize, Serialize};

use crate::event::SyncEvent;
use crate::switches::{event_type, selector_of};

/// Why a view is desynced.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DesyncReason {
    /// No element matches the target (`not_found`).
    NotFound,
    /// Matches exist, none is rendered (`not_visible`).
    NotVisible,
    /// The replay did not answer within `sync.desyncTimeoutMs`, or the view fell too far behind.
    Timeout,
    /// The replay failed (the engine's error is logged, not shown).
    Failed,
}

/// One desynced view: what the badge's tooltip and MCP's `views_list` show.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewDesync {
    /// The view.
    pub view: ViewId,
    /// Why.
    pub reason: DesyncReason,
    /// The CSS path of the element the event aimed at; absent for an event without a target.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selector: Option<String>,
    /// The type of the event that could not follow (`click`).
    pub event: String,
}

/// The desynced views.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DesyncTable {
    views: BTreeMap<ViewId, ViewDesync>,
}

impl DesyncTable {
    /// `view` could not follow `event`; `true` when it was in sync until now. The first reason
    /// stays until the view is cleared.
    pub fn mark(&mut self, view: ViewId, reason: DesyncReason, event: &SyncEvent) -> bool {
        if self.views.contains_key(&view) {
            return false;
        }
        self.views.insert(
            view,
            ViewDesync {
                view,
                reason,
                selector: selector_of(event),
                event: event_type(event).to_owned(),
            },
        );
        true
    }

    /// `view` is in sync again (navigated, resynced); `true` when it was desynced.
    pub fn clear(&mut self, view: ViewId) -> bool {
        self.views.remove(&view).is_some()
    }

    /// Whether `view` is desynced.
    pub fn is_desynced(&self, view: ViewId) -> bool {
        self.views.contains_key(&view)
    }

    /// `view`'s desync.
    pub fn get(&self, view: ViewId) -> Option<&ViewDesync> {
        self.views.get(&view)
    }

    /// Every desynced view, by id.
    pub fn list(&self) -> Vec<ViewDesync> {
        self.views.values().cloned().collect()
    }

    /// Forgets the views not in `views`.
    pub fn retain(&mut self, views: &[ViewId]) {
        self.views.retain(|view, _| views.contains(view));
    }
}

#[cfg(test)]
#[path = "desync_tests.rs"]
mod tests;
