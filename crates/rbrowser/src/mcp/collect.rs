//! Feeds the MCP log buffers ([`rb_mcp::LogStore`]) while the server runs: every view's CDP
//! session gets `Runtime`, `Log` and `Network` enabled (again after a reattach), and the events
//! of those domains go into the view's buffers. A closed view's buffers go with it. Stops when
//! the server stops (the sender of `stop` is dropped) or the window closes.

use std::collections::HashMap;
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
    // The session each view's domains were enabled on.
    let mut enabled: HashMap<ViewId, String> = HashMap::new();
    enable_new(&host, &mut enabled).await;
    loop {
        tokio::select! {
            changed = stop.changed() => {
                if changed.is_err() || *stop.borrow() {
                    return;
                }
            }
            event = cdp_events.recv() => match event {
                Ok(event) => {
                    let Some(session) = event.session_id.as_deref() else { continue };
                    let mut view = view_of(&enabled, session);
                    if view.is_none() {
                        enable_new(&host, &mut enabled).await;
                        view = view_of(&enabled, session);
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
                    enabled.remove(&view);
                    store.forget(view);
                }
                Ok(ViewEvent::Opened(_) | ViewEvent::Updated(_) | ViewEvent::LayoutChanged)
                | Err(broadcast::error::RecvError::Lagged(_)) => {
                    enable_new(&host, &mut enabled).await;
                }
                Ok(_) => {}
                Err(broadcast::error::RecvError::Closed) => return,
            },
        }
    }
}

fn view_of(enabled: &HashMap<ViewId, String>, session: &str) -> Option<ViewId> {
    enabled
        .iter()
        .find(|(_, known)| known.as_str() == session)
        .map(|(view, _)| *view)
}

/// Enables the log domains on every session not enabled yet.
async fn enable_new(host: &Weak<Host>, enabled: &mut HashMap<ViewId, String>) {
    let Some(host) = host.upgrade() else {
        return;
    };
    for (view, session) in host.cdp_sessions() {
        if enabled.get(&view) == Some(&session) {
            continue;
        }
        for method in rb_mcp::logs::DOMAINS {
            if let Err(error) = host.cdp().call(method, json!({}), Some(&session)).await {
                tracing::debug!(%view, %error, method, "a log domain was not enabled");
            }
        }
        enabled.insert(view, session);
    }
}
