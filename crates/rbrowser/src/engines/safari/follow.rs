//! A Safari window - and an iOS view (RB-095-04) - follows the navigations of its cell
//! (RB-095-03), as WebKit's does (`super::super::webkit::follow`): every navigation the sync,
//! the address bar or MCP makes in the cell's Chromium stand-in is the URL the Safari window
//! loads too, unless it is there already or the URL is no page (the cell's marker, internal
//! pages).

use std::sync::{Arc, Weak};

use rb_engine::{ViewEvent, ViewHandle, ViewHost};
use rb_safari::SafariEngine;
use tokio::runtime::Handle;
use tokio::sync::broadcast::error::RecvError;

use crate::engines::webkit::follow::follows;
use crate::host::Host;

/// Follows `host`'s navigations into `engine`'s views, on `runtime`.
pub(in crate::engines) fn watch(host: &Host, engine: &Arc<SafariEngine>, runtime: &Handle) {
    let mut events = host.subscribe();
    let engine: Weak<SafariEngine> = Arc::downgrade(engine);
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
            if let Err(error) = ViewHandle::navigate(handle.as_ref(), &url).await {
                tracing::debug!(%view, %error, "the Safari window did not follow");
            }
        }
    });
}
