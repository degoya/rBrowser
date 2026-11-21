//! An Android view follows the navigations of its cell (RB-095-02), as a WebKit window does
//! (`super::super::webkit::follow`): the Chromium stand-in in the cell takes every navigation
//! the sync, the address bar or MCP makes, and each one the device's tab is not at already is
//! loaded on the device too.

use std::sync::{Arc, Weak};

use rb_engine::{ViewEvent, ViewHandle as _, ViewHost};
use tokio::runtime::Handle;
use tokio::sync::broadcast::error::RecvError;

use super::engine::AndroidEngine;
use crate::engines::webkit::follow::follows;
use crate::host::Host;

/// Follows `host`'s navigations into `engine`'s views, on `runtime`.
pub(super) fn watch(host: &Host, engine: &Arc<AndroidEngine>, runtime: &Handle) {
    let mut events = host.subscribe();
    let engine: Weak<AndroidEngine> = Arc::downgrade(engine);
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
            if let Err(error) = handle.navigate(&url).await {
                tracing::debug!(%view, %error, "the Android view did not follow");
            }
        }
    });
}
