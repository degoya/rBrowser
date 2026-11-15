//! What a view's menu showed last, kept until an entry of it is chosen (RB-080-45).
//!
//! CEF asks its client for the context menu handler once per callback: the handler that gets
//! `on_context_menu_command` is not the object that got `on_before_context_menu`. Whatever the
//! command needs from the build - the id -> action table - therefore belongs to the view's
//! client, which lives as long as the view and makes every handler: the client holds one
//! [`ViewMenu`] and gives each handler a clone, which shares the block. Nothing outlives the
//! view, and a later view with the same id starts with no block.

use std::sync::{Arc, Mutex};

use rb_engine::ViewId;

use super::model::{Action, FIRST_ID, Item, build, find};
use super::state::state_for;
use crate::cdp::lock;
use crate::control::Core;

/// The status bar's text for a chosen entry the shown block does not have.
pub const ENTRY_GONE_KEY: &str = "contextmenu.notice.entryGone";

/// What a view's menu showed.
#[derive(Debug, Default)]
struct Shown {
    /// The block of the last showing.
    items: Vec<Item>,
    /// The highest entry id any showing of this menu handed out: the ids from
    /// [`FIRST_ID`] to it are this menu's, whatever the last block still has of them.
    last_id: Option<i32>,
}

/// The highest entry id among `items`, submenus included.
fn last_id(items: &[Item]) -> Option<i32> {
    items
        .iter()
        .filter_map(|item| match item {
            Item::Entry(entry) => Some(entry.id),
            Item::Separator => None,
            Item::Menu { items, .. } => last_id(items),
        })
        .max()
}

/// The menu of one view. A clone is the same menu: it shares the block that was shown.
#[derive(Clone, Debug)]
pub struct ViewMenu {
    view: ViewId,
    shown: Arc<Mutex<Shown>>,
}

impl ViewMenu {
    /// A menu of `view` that showed nothing yet: one per view client, cloned into its handlers.
    #[must_use]
    pub fn for_view(view: ViewId) -> Self {
        Self {
            view,
            shown: Arc::new(Mutex::new(Shown::default())),
        }
    }

    /// The menu opens at `point`: builds the block from the core's state, remembers it for
    /// [`Self::chosen`] and returns it for rendering.
    pub fn opened(&self, core: &Core, point: Option<(i32, i32)>) -> Vec<Item> {
        let items = build(&state_for(core, self.view, point));
        self.remember(items.clone());
        items
    }

    /// Remembers `items` as the block this view shows.
    pub fn remember(&self, items: Vec<Item>) {
        let mut shown = lock(&self.shown);
        shown.last_id = shown.last_id.max(last_id(&items));
        shown.items = items;
    }

    /// What the entry `id` of the block this view showed last does.
    #[must_use]
    pub fn action(&self, id: i32) -> Option<Action> {
        find(&lock(&self.shown).items, id).map(|entry| entry.action.clone())
    }

    /// Whether `id` is an entry id this menu handed out at one of its showings.
    fn handed_out(&self, id: i32) -> bool {
        lock(&self.shown)
            .last_id
            .is_some_and(|last| (FIRST_ID..=last).contains(&id))
    }

    /// The entry `id` was chosen: runs it off the calling thread. `false` for an id this menu
    /// never handed out (Chromium's own, or anyone else's in CEF's user range): its default
    /// runs. An id this menu did hand out that its last block no longer has is handled too -
    /// nothing can run, and the log and the status bar say so: a menu entry never does nothing
    /// silently.
    pub fn chosen(&self, core: &Arc<Core>, id: i32) -> bool {
        let view = self.view;
        match self.action(id) {
            Some(action) => {
                match &action {
                    Action::Run { command, .. } => {
                        tracing::info!(%view, id, %command, "the context menu runs a command");
                    }
                    Action::SkipNextClick => {
                        tracing::info!(%view, id, "the context menu skips the next click");
                    }
                }
                super::run::run(core, view, &action);
                true
            }
            None if self.handed_out(id) => {
                tracing::warn!(%view, id, "the context menu's entry is not in the shown block");
                core.notify(crate::control::warning_notification(ENTRY_GONE_KEY, None));
                true
            }
            None => false,
        }
    }
}

#[cfg(test)]
#[path = "shown_tests.rs"]
mod tests;
