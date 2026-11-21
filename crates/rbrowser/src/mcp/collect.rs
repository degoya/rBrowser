//! Feeds the log buffers ([`rb_mcp::LogStore`]) of MCP and the plugins granted `console-logs`,
//! one collector per window (`super::shared_logs`): every view's CDP session gets `Runtime`,
//! `Log` and `Audits` enabled (again after a reattach), and `Network` once the store wants the
//! requests (RB-090-34); the events of those domains go into the view's buffers. A closed view's
//! buffers go with it. A session of no view (DevTools, the analyser, a proxy client) is looked up
//! once, then again only after a view event. Stops when the last holder lets the buffers go (the
//! sender of `stop` is dropped) or the window closes.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Weak};

use rb_engine::{ViewEvent, ViewHost as _, ViewId};
use rb_mcp::LogStore;
use serde_json::json;
use tokio::sync::{broadcast, watch};

use crate::control::Core;
use crate::host::Host;

/// Starts feeding `store` from `core`'s views; ends when `stop` is dropped or set.
pub fn spawn(core: &Core, store: Arc<LogStore>, stop: watch::Receiver<bool>) {
    let host = Arc::downgrade(core.host());
    core.runtime().spawn(run(host, store, stop));
}

async fn run(host: Weak<Host>, store: Arc<LogStore>, mut stop: watch::Receiver<bool>) {
    let Some(strong) = host.upgrade() else {
        return;
    };
    let mut cdp_events = strong.cdp().subscribe();
    let mut view_events = strong.subscribe();
    drop(strong);
    let mut network = store.network_wanted();
    let mut sessions = Sessions::default();
    let wanted = *network.borrow_and_update();
    enable_new(&host, &mut sessions, wanted).await;
    loop {
        tokio::select! {
            changed = stop.changed() => {
                if changed.is_err() || *stop.borrow() {
                    return;
                }
            }
            // The store lives as long as this task: the sender cannot go first.
            Ok(()) = network.changed() => {
                let wanted = *network.borrow_and_update();
                enable_new(&host, &mut sessions, wanted).await;
            }
            event = cdp_events.recv() => match event {
                Ok(event) => {
                    if event.method == "Target.detachedFromTarget"
                        && let Some(gone) = event.params["sessionId"].as_str()
                    {
                        sessions.detached(gone);
                    }
                    let Some(session) = event.session_id.as_deref() else { continue };
                    let mut view = sessions.view_of(session);
                    if view.is_none() && sessions.try_enable(session) {
                        // Bound first: no borrow of the watch across the await.
                        let wanted = *network.borrow();
                        enable_new(&host, &mut sessions, wanted).await;
                        view = sessions.view_of(session);
                    }
                    if let Some(view) = view {
                        store.record(view, &event.method, &event.params);
                    }
                }
                Err(broadcast::error::RecvError::Lagged(missed)) => {
                    tracing::debug!(missed, "the MCP log buffers missed CDP events");
                }
                Err(broadcast::error::RecvError::Closed) => return,
            },
            event = view_events.recv() => match event {
                Ok(ViewEvent::Closed(view)) => {
                    sessions.closed(view);
                    store.forget(view);
                }
                Ok(ViewEvent::Opened(_) | ViewEvent::Updated(_) | ViewEvent::LayoutChanged)
                | Err(broadcast::error::RecvError::Lagged(_)) => {
                    sessions.views_changed();
                    let wanted = *network.borrow();
                    enable_new(&host, &mut sessions, wanted).await;
                }
                Ok(_) => {}
                Err(broadcast::error::RecvError::Closed) => return,
            },
        }
    }
}

/// The session each view's domains were enabled on, the one `Network` was, and the sessions of
/// no view looked up since the last view event.
#[derive(Default)]
struct Sessions {
    enabled: HashMap<ViewId, String>,
    network: HashMap<ViewId, String>,
    tried: HashSet<String>,
}

impl Sessions {
    fn view_of(&self, session: &str) -> Option<ViewId> {
        self.enabled
            .iter()
            .find(|(_, known)| known.as_str() == session)
            .map(|(view, _)| *view)
    }

    /// Whether `session`, of no view, is worth a look for new views: once until the views
    /// change.
    fn try_enable(&mut self, session: &str) -> bool {
        self.tried.insert(session.to_owned())
    }

    fn views_changed(&mut self) {
        self.tried.clear();
    }

    fn detached(&mut self, session: &str) {
        self.tried.remove(session);
    }

    fn closed(&mut self, view: ViewId) {
        self.enabled.remove(&view);
        self.network.remove(&view);
    }

    /// The enable calls `current`'s sessions still need - the log domains on a session not
    /// enabled yet, `Network` too while `network` is wanted - taken as done.
    fn plan(
        &mut self,
        current: &[(ViewId, String)],
        network: bool,
    ) -> Vec<(ViewId, String, &'static str)> {
        let mut calls = Vec::new();
        for (view, session) in current {
            if self.enabled.get(view) != Some(session) {
                calls.extend(
                    rb_mcp::logs::DOMAINS
                        .iter()
                        .map(|method| (*view, session.clone(), *method)),
                );
                self.enabled.insert(*view, session.clone());
            }
            if network && self.network.get(view) != Some(session) {
                calls.push((*view, session.clone(), rb_mcp::logs::NETWORK_DOMAIN));
                self.network.insert(*view, session.clone());
            }
        }
        calls
    }
}

/// Enables the log domains on every session not enabled yet, `Network` while it is wanted.
async fn enable_new(host: &Weak<Host>, sessions: &mut Sessions, network: bool) {
    let Some(host) = host.upgrade() else {
        return;
    };
    for (view, session, method) in sessions.plan(&host.cdp_sessions(), network) {
        if let Err(error) = host.cdp().call(method, json!({}), Some(&session)).await {
            tracing::debug!(%view, %error, method, "a log domain was not enabled");
        }
    }
}

#[cfg(test)]
#[path = "collect_tests.rs"]
mod tests;
