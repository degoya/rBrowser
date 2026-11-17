//! The catch-up of a view shown again (RB-020-09), pure. A hidden view still follows
//! navigation and scroll sync - CDP and the sync agent reach it - so this is the safety net
//! for what it missed: a view on another URL than the leader's loads the leader's (the hub's
//! resync), one on the same URL takes the leader's last document scroll. A view out of sync on
//! purpose (sync off, excluded, decoupled, desynced with its badge) is left alone.

use std::collections::HashMap;

use rb_engine::ViewId;
use rb_sync::{EventMessage, ScrollData, SyncEvent};

/// What the catch-up knows of a view shown again.
#[derive(Clone, Debug, PartialEq)]
pub struct CatchUpFacts {
    /// The view.
    pub view: ViewId,
    /// Its URL.
    pub view_url: String,
    /// The leader (the active view) and its URL.
    pub leader: Option<(ViewId, String)>,
    /// Sync is on.
    pub enabled: bool,
    /// Navigation sync is on.
    pub navigation: bool,
    /// Scroll sync is on.
    pub scroll: bool,
    /// The view is excluded from sync.
    pub excluded: bool,
    /// The view's navigation is decoupled (RB-020-06).
    pub decoupled: bool,
    /// The view is desynced: its badge asks the person to resync.
    pub desynced: bool,
}

/// The one step a view shown again takes.
#[derive(Clone, Debug, PartialEq)]
pub enum CatchUpStep {
    /// Load the leader's URL (`HostSync::resync`).
    Resync,
    /// Replay the leader's last document scroll.
    Scroll {
        /// The leader.
        source: ViewId,
        /// Its last scroll (boxed: the variant would make every step large).
        data: Box<ScrollData>,
    },
}

/// The step `facts` call for, with the leaders' last scrolls in `memo`.
pub fn plan(facts: &CatchUpFacts, memo: &ScrollMemo) -> Option<CatchUpStep> {
    let (leader, leader_url) = facts.leader.as_ref()?;
    if *leader == facts.view || !facts.enabled || facts.excluded || facts.desynced {
        return None;
    }
    if facts.view_url != *leader_url {
        let follows = facts.navigation && !facts.decoupled && !leader_url.is_empty();
        return follows.then_some(CatchUpStep::Resync);
    }
    if !facts.scroll {
        return None;
    }
    memo.get(*leader).map(|data| CatchUpStep::Scroll {
        source: *leader,
        data: Box::new(data.clone()),
    })
}

/// The last document scroll of each view's top frame, while it stays on its document.
#[derive(Clone, Debug, Default)]
pub struct ScrollMemo {
    last: HashMap<ViewId, ScrollData>,
}

impl ScrollMemo {
    /// `view`'s agent reported `message`.
    pub fn observe(&mut self, view: ViewId, message: &EventMessage) {
        if let SyncEvent::Scroll(data) = &message.event
            && message.top
            && data.container.is_none()
        {
            self.last.insert(view, data.clone());
        }
    }

    /// `view` left its document.
    pub fn navigated(&mut self, view: ViewId) {
        self.last.remove(&view);
    }

    /// Keeps only `views`.
    pub fn retain(&mut self, views: &[ViewId]) {
        self.last.retain(|view, _| views.contains(view));
    }

    /// `view`'s last document scroll.
    pub fn get(&self, view: ViewId) -> Option<&ScrollData> {
        self.last.get(&view)
    }
}

#[cfg(test)]
#[path = "catchup_tests.rs"]
mod tests;
