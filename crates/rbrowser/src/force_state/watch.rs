//! Navigation ends forced states (RB-090-53): `DOM.documentUpdated` on rBrowser's session of a
//! view (a navigation, a reload) - Chromium dropped the nodes and their states - empties the
//! view's record; `Target.detachedFromTarget` of that session (the view closed) forgets the view.
//! Either way the status bar says what stays forced. A docked DevTools frontend's own events
//! come on its session and are not rBrowser's. Started with the first forced state, once per
//! window.

use std::sync::{Arc, OnceLock, Weak};

use rb_engine::ViewId;
use serde_json::Value;

use super::model::Registry;
use crate::cdp::{CdpEvent, lock};
use crate::control::Core;

/// The events the watch reads, each on a tap of its own.
pub const DOCUMENT_UPDATED: &str = "DOM.documentUpdated";
pub const DETACHED: &str = "Target.detachedFromTarget";
/// Events that may wait for the watch.
const TAP_CAPACITY: usize = 64;

static STARTED: OnceLock<()> = OnceLock::new();

/// What `event` ends in `registry`: the view whose forced states went, and whether it had any.
pub fn on_event(registry: &mut Registry, event: &CdpEvent) -> Option<(ViewId, bool)> {
    match event.method.as_str() {
        DOCUMENT_UPDATED => registry.document_updated(event.session_id.as_deref()?),
        DETACHED => registry.detached(event.params.get("sessionId").and_then(Value::as_str)?),
        _ => None,
    }
}

/// Starts the watch of `core`'s window, once.
pub fn ensure(core: &Core) {
    STARTED.get_or_init(|| start(core));
}

fn start(core: &Core) {
    let Some(core_arc) = core.arc() else {
        return;
    };
    let weak: Weak<Core> = Arc::downgrade(&core_arc);
    let cdp = core.host().cdp().clone();
    let mut documents = cdp.tap(DOCUMENT_UPDATED, TAP_CAPACITY);
    let mut detached = cdp.tap(DETACHED, TAP_CAPACITY);
    core.runtime().spawn(async move {
        loop {
            let event = tokio::select! {
                Some(event) = documents.recv() => event,
                Some(event) = detached.recv() => event,
                else => break,
            };
            let ended = on_event(&mut lock(super::registry()), &event);
            let Some((view, true)) = ended else {
                continue;
            };
            let Some(core) = weak.upgrade() else {
                break;
            };
            tracing::info!(%view, "force state: the page changed; its forced states are gone");
            let forced = lock(super::registry()).forced_views();
            core.notify(super::command::status(&core, &forced));
        }
    });
}

#[cfg(test)]
#[path = "watch_tests.rs"]
mod tests;
