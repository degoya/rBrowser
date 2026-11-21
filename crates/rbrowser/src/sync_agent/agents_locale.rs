//! The page locale of each view's agents (RB-092-02): `dir` and `lang` from the view's emulation
//! and the pseudo-localisation of `overlays.pseudo_locale`, kept per view and sent as
//! [`AgentCommand::Locale`] to every agent of the view when it changes - and to the agent of
//! every new document, after its configuration, so a reload or a navigation keeps it. A child
//! of `agents`: it shares [`SyncAgents`]' state.

use rb_config::TextDirection;
use rb_engine::{Emulation, ViewId};
use rb_sync::{AgentCommand, PageDir, PageLocale, PseudoLocale};

use super::SyncAgents;
use crate::cdp::lock;

/// The agent's direction of the config's.
fn page_dir(dir: TextDirection) -> PageDir {
    match dir {
        TextDirection::Ltr => PageDir::Ltr,
        TextDirection::Rtl => PageDir::Rtl,
        TextDirection::Auto => PageDir::Auto,
    }
}

impl SyncAgents {
    /// What `view`'s page shows of its locale now (plain for a view rBrowser does not know).
    pub fn page_locale(&self, view: ViewId) -> PageLocale {
        lock(&self.state)
            .locales
            .get(&view)
            .cloned()
            .unwrap_or_default()
    }

    /// `view`'s pseudo-localisation from now on; `true` when that is a change.
    pub fn set_pseudo(&self, view: ViewId, pseudo: Option<PseudoLocale>) -> bool {
        self.update_locale(view, |locale| locale.pseudo = pseudo)
    }

    /// The view's emulation was applied: its `dir` and `lang` reach the agents when they
    /// changed (they ride on no CDP call; the agent sets them on the live document).
    pub async fn follow_emulation(&self, view: ViewId, emulation: &Emulation) {
        let dir = emulation.dir.map(page_dir);
        let lang = emulation.lang.clone();
        let changed = self.update_locale(view, |locale| {
            locale.dir = dir;
            locale.lang = lang;
        });
        if changed {
            self.push_locale(view).await;
        }
    }

    /// Sends `view`'s locale to every agent of the view.
    pub async fn push_locale(&self, view: ViewId) {
        let command = AgentCommand::Locale {
            locale: self.page_locale(view),
        };
        for (session, context) in self.contexts_of(view) {
            if let Err(error) = self.evaluate(&session, context, &command).await {
                tracing::debug!(%view, %error, "page locale not applied");
            }
        }
    }

    /// The agent of a new document (`context` in `session`) gets the view's locale - unless the
    /// page is plain, as a new document is.
    pub(super) async fn locale_ready(&self, view: ViewId, session: &str, context: i64) {
        let locale = self.page_locale(view);
        if locale.is_plain() {
            return;
        }
        let command = AgentCommand::Locale { locale };
        if let Err(error) = self.evaluate(session, context, &command).await {
            tracing::debug!(%view, %error, "page locale not applied to a new document");
        }
    }

    /// Every agent context of `view`: its sessions' (the page's and its iframes').
    pub(crate) fn contexts_of(&self, view: ViewId) -> Vec<(String, i64)> {
        let state = lock(&self.state);
        state
            .sessions
            .iter()
            .filter(|(_, entry)| entry.view == view)
            .flat_map(|(session, _)| {
                let contexts = state.contexts.contexts(session);
                contexts.into_iter().map(move |id| (session.clone(), id))
            })
            .collect()
    }

    /// Changes `view`'s locale with `change`; `true` when it differs afterwards. A plain locale
    /// is not kept.
    fn update_locale(&self, view: ViewId, change: impl FnOnce(&mut PageLocale)) -> bool {
        let mut state = lock(&self.state);
        let before = state.locales.get(&view).cloned().unwrap_or_default();
        let mut after = before.clone();
        change(&mut after);
        if after == before {
            return false;
        }
        if after.is_plain() {
            state.locales.remove(&view);
        } else {
            state.locales.insert(view, after);
        }
        true
    }
}

#[cfg(test)]
#[path = "agents_locale_tests.rs"]
mod tests;
