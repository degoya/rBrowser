//! The host's part of sessions (RB-020-07): which session a view is in, moving views into
//! another session, and clearing a session's data. A view cannot change its request context, so
//! a move replaces it in its cell by a new view created in the session's context, which loads
//! the old view's URL once its CDP session is attached.

use rb_config::DEFAULT_SESSION;
use rb_engine::{EngineError, Target, ViewEvent, ViewId};
use serde_json::json;
use tokio::sync::oneshot;

use super::Host;
use super::rebuild::rebuild_state;
use super::state::ViewState;
use crate::sessions::{Moved, STORAGE_TYPES, effective_session, origins_to_clear, require_known};

/// What [`Host::clear_session`] clears.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ClearPlan {
    /// The session's views, grid order; they reload afterwards.
    pub views: Vec<ViewId>,
    /// The origins whose storage is cleared.
    pub origins: Vec<String>,
}

impl Host {
    /// The session `view` lives in.
    pub fn view_session(&self, view: ViewId) -> Result<String, EngineError> {
        let state = self.state();
        let entry = state
            .views
            .get(&view)
            .ok_or(EngineError::UnknownView(view))?;
        Ok(entry.context_session().to_owned())
    }

    /// The views of `session`, grid order.
    pub fn session_views(&self, session: &str) -> Vec<ViewId> {
        let state = self.state();
        state
            .order
            .iter()
            .filter(|id| {
                state
                    .views
                    .get(id)
                    .is_some_and(|entry| entry.context_session() == session)
            })
            .copied()
            .collect()
    }

    /// Moves the views of `target` into `session` (`default` or one of `sessions`): each one in
    /// another session is replaced by a new view in `session`'s context that loads its URL. The
    /// choice survives config changes.
    pub fn assign_session(&self, target: Target, session: &str) -> Result<Vec<Moved>, EngineError> {
        let views = {
            let state = self.state();
            require_known(&state.config, session)
                .map_err(|error| EngineError::Invalid(error.to_string()))?;
            match target {
                Target::All => state.order.clone(),
                Target::View(view) if state.views.contains_key(&view) => vec![view],
                Target::View(view) => return Err(EngineError::UnknownView(view)),
            }
        };
        let moves: Vec<(ViewId, String)> = views
            .into_iter()
            .map(|view| (view, session.to_owned()))
            .collect();
        self.move_views(&moves, true)
    }

    /// The config changed: views whose session was not chosen by hand follow their breakpoint's
    /// `session`.
    pub fn follow_config_sessions(&self) -> Result<Vec<Moved>, EngineError> {
        let moves: Vec<(ViewId, String)> = {
            let state = self.state();
            state
                .order
                .iter()
                .filter_map(|id| state.views.get(id))
                .filter(|entry| !entry.session_chosen)
                .filter_map(|entry| {
                    let wanted = effective_session(
                        None,
                        entry.breakpoint.as_ref().and_then(|b| b.session.as_deref()),
                    );
                    (wanted != entry.context_session()
                        && require_known(&state.config, wanted).is_ok())
                    .then(|| (entry.id, wanted.to_owned()))
                })
                .collect()
        };
        self.move_views(&moves, false)
    }

    /// Replaces each view of `moves` that is not in its session yet by a new one in it. The
    /// moves and the rebuild run on a copy of the state, committed only when the rebuild
    /// succeeds, so a failure leaves host and grid as they were.
    fn move_views(
        &self,
        moves: &[(ViewId, String)],
        chosen: bool,
    ) -> Result<Vec<Moved>, EngineError> {
        let mut moved = Vec::new();
        let mut state = self.state();
        let mut next = state.clone();
        let start = next
            .config
            .start_url
            .clone()
            .unwrap_or_else(|| "about:blank".to_owned());
        for (from, session) in moves {
            let Some(entry) = next.views.get(from) else {
                continue;
            };
            if entry.context_session() == session {
                continue;
            }
            let Some(old) = next.views.remove(from) else {
                continue;
            };
            let view = next.allocate_id();
            let url = if old.url.is_empty() {
                old.pending_url.clone().unwrap_or_else(|| start.clone())
            } else {
                old.url.clone()
            };
            next.views
                .insert(view, replacement(old, view, url, session, chosen));
            for slot in next.order.iter_mut().filter(|id| **id == *from) {
                *slot = view;
            }
            if next.active == Some(*from) {
                next.active = Some(view);
            }
            moved.push(Moved { from: *from, view });
        }
        if moved.is_empty() {
            return Ok(moved);
        }
        let layout = next.layout.clone();
        let rebuilt = rebuild_state(&mut next, &layout)?;
        *state = next;
        drop(state);
        self.finish_rebuild(&layout, rebuilt);
        for change in &moved {
            // The old view is gone without passing through the rebuild's closed list.
            self.agents.forget(change.from);
            self.emit(ViewEvent::Closed(change.from));
            self.emit(ViewEvent::Opened(change.view));
        }
        Ok(moved)
    }

    /// Clears `session`'s data: every cookie of its context (through the window), the storage
    /// of every origin its views show (over CDP, on a view of the session, so the session's own
    /// storage partition), then reloads its views. Returns at once with what is cleared.
    pub fn clear_session(&self, session: &str) -> Result<ClearPlan, EngineError> {
        let (views, calls) = {
            let state = self.state();
            require_known(&state.config, session)
                .map_err(|error| EngineError::Invalid(error.to_string()))?;
            let entries: Vec<&ViewState> = state
                .order
                .iter()
                .filter_map(|id| state.views.get(id))
                .filter(|entry| entry.context_session() == session)
                .collect();
            let calls: Vec<(String, String)> = origins_to_clear(
                entries
                    .iter()
                    .filter(|entry| entry.session.is_some())
                    .map(|entry| entry.url.as_str()),
            )
            .into_iter()
            .filter_map(|origin| {
                let cdp = entries.iter().find_map(|entry| {
                    let shows = rb_util::url::origin_of(&entry.url).as_ref() == Some(&origin);
                    entry.session.clone().filter(|_| shows)
                })?;
                Some((origin, cdp))
            })
            .collect();
            let views: Vec<ViewId> = entries.iter().map(|entry| entry.id).collect();
            (views, calls)
        };
        let plan = ClearPlan {
            views: views.clone(),
            origins: calls.iter().map(|(origin, _)| origin.clone()).collect(),
        };
        let (done, cookies) = oneshot::channel();
        self.ui.clear_session_cookies(
            session.to_owned(),
            Box::new(move |cleared| {
                // The receiver only goes when the task below is gone.
                let _ = done.send(cleared);
            }),
        );
        let (cdp, ui, session) = (self.cdp.clone(), self.ui.clone(), session.to_owned());
        self.runtime.spawn(async move {
            if !cookies.await.unwrap_or(false) {
                tracing::warn!(%session, "the session's cookies were not cleared");
            }
            for (origin, cdp_session) in calls {
                let params = json!({ "origin": origin, "storageTypes": STORAGE_TYPES });
                if let Err(error) = cdp
                    .call("Storage.clearDataForOrigin", params, Some(&cdp_session))
                    .await
                {
                    tracing::warn!(%session, %error, "a session's site data was not cleared");
                }
            }
            for view in views {
                ui.reload(view, true);
            }
        });
        Ok(plan)
    }
}

/// `old` as the new view `view` in `session`: cell, breakpoint, zoom and emulation stay; the
/// browser, its target and CDP session are new, and it loads `url` once attached.
fn replacement(
    old: ViewState,
    view: ViewId,
    url: String,
    session: &str,
    chosen: bool,
) -> ViewState {
    ViewState {
        id: view,
        measured: None,
        applied: None,
        applied_emulation: None,
        hidden: false,
        target_id: None,
        session: None,
        browser_context: None,
        attaching: false,
        pending_url: Some(url),
        url: String::new(),
        title: String::new(),
        loading: true,
        loads_ended: 0,
        context_session: (session != DEFAULT_SESSION).then(|| session.to_owned()),
        session_chosen: chosen || old.session_chosen,
        // The new browser starts in the grid; the old one's window closes with it (RB-080-14).
        detached: None,
        ..old
    }
}

#[cfg(test)]
#[path = "sessions_tests.rs"]
mod tests;
