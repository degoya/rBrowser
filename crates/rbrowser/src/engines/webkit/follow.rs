//! A WebKit window follows the navigations of its cell (RB-040-04). Chromium shows each external
//! view's cell in the grid (RB-040-01), so every navigation the sync, the address bar or MCP
//! makes there - a host `Navigated` event of that view - is the URL the WebKit window loads too.
//! Reload, back and forward reach the window through the sync's replay of the keys and through
//! the navigations they cause.

use std::sync::{Arc, Weak};

use rb_engine::{ViewEvent, ViewHost};
use tokio::runtime::Handle;
use tokio::sync::broadcast::error::RecvError;

use super::engine::WebkitEngine;
use crate::host::Host;

/// Whether a WebKit view at `current` loads `url`: a page URL it is not at already. The cell's
/// marker URL (`about:blank#rb-view-<id>`) and other internal pages are never followed.
pub fn follows(current: &str, url: &str) -> bool {
    let page = ["http://", "https://", "file:"]
        .iter()
        .any(|scheme| url.starts_with(scheme));
    page && url != current
}

/// Follows `host`'s navigations into `engine`'s views, on `runtime`.
pub(super) fn watch(host: &Host, engine: &Arc<WebkitEngine>, runtime: &Handle) {
    let mut events = host.subscribe();
    let engine: Weak<WebkitEngine> = Arc::downgrade(engine);
    runtime.spawn(async move {
        loop {
            let (view, url) = match events.recv().await {
                Ok(ViewEvent::Navigated { view, url }) => (view, url),
                Ok(_) | Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => return,
            };
            let Some(engine) = engine.upgrade() else {
                return;
            };
            let Some(handle) = engine.view(view) else {
                continue;
            };
            if !follows(&handle.url(), &url) {
                continue;
            }
            if let Err(error) = rb_engine::ViewHandle::navigate(handle.as_ref(), &url).await {
                tracing::debug!(%view, %error, "the WebKit window did not follow");
            }
        }
    });
}

#[cfg(test)]
#[path = "follow_tests.rs"]
mod tests;
