//! The tasks that keep [`super::PageApi`] current (RB-020-08): the host's view events (a new
//! breakpoint, size or emulation, a rebuilt grid, a reloaded config) give each view its page
//! state again, and the agents' reports bring the `ready` of a new document (it gets the state)
//! and the pages' bus messages (relayed, `bus.rs`, `relay.rs`). Both run as long as the host
//! does; the reports' task waits for no page.

use std::sync::Arc;

use rb_engine::{ViewEvent, ViewHost, ViewId};
use rb_sync::{AgentMessage, PageState};
use tokio::runtime::Handle;
use tokio::sync::broadcast::error::RecvError;

use super::page_state::{PageSwitches, page_state};
use super::relay::BusRelay;
use crate::host::Host;

/// Starts both tasks for `host` on `runtime`.
pub fn start(host: &Arc<Host>, runtime: &Handle) {
    let mut events = host.subscribe();
    let weak = Arc::downgrade(host);
    runtime.spawn(async move {
        loop {
            let event = events.recv().await;
            let Some(host) = weak.upgrade() else {
                return;
            };
            match event {
                Ok(ViewEvent::Opened(view) | ViewEvent::Updated(view)) => {
                    refresh(&host, view).await;
                }
                Ok(ViewEvent::LayoutChanged) | Err(RecvError::Lagged(_)) => {
                    refresh_all(&host).await;
                }
                Ok(ViewEvent::Closed(view)) => host.page_api().forget(view),
                Ok(_) => {}
                Err(RecvError::Closed) => return,
            }
        }
    });
    let mut reports = host.sync_agents().subscribe();
    let weak = Arc::downgrade(host);
    let handle = runtime.clone();
    runtime.spawn(async move {
        // Nothing here waits for a page: a hung one must not hold up the reports behind it.
        let mut relay = BusRelay::new(handle.clone());
        loop {
            let report = match reports.recv().await {
                Ok(report) => report,
                Err(RecvError::Lagged(missed)) => {
                    tracing::warn!(missed, "agent reports dropped (page API)");
                    continue;
                }
                Err(RecvError::Closed) => return,
            };
            let Some(host) = weak.upgrade() else {
                return;
            };
            match &report.message {
                AgentMessage::Ready(ready) if ready.top => {
                    let view = report.view;
                    handle.spawn(async move { host.page_api().on_ready(view).await });
                }
                AgentMessage::Bus(message) => relay.relay(&host, report.view, message),
                _ => {}
            }
        }
    });
}

/// The page state of `view` now under `switches`; `None` for an unknown view.
pub fn state_of(host: &Host, view: ViewId, switches: &PageSwitches) -> Option<PageState> {
    let views = host.views();
    let (index, info) = views.iter().enumerate().find(|(_, info)| info.id == view)?;
    let emulation = host.emulation(view)?;
    Some(page_state(info, index, &emulation, switches))
}

/// Gives `view` its page state again.
pub async fn refresh(host: &Host, view: ViewId) {
    let switches = host.page_api().switches(&host.config());
    if let Some(state) = state_of(host, view, &switches) {
        host.page_api().update(view, state, switches.api).await;
    }
}

/// Gives every view its page state again (the grid, the config or a toggle changed).
pub async fn refresh_all(host: &Host) {
    let switches = host.page_api().switches(&host.config());
    for (index, info) in host.views().iter().enumerate() {
        let Some(emulation) = host.emulation(info.id) else {
            continue;
        };
        let state = page_state(info, index, &emulation, &switches);
        host.page_api().update(info.id, state, switches.api).await;
    }
}

#[cfg(test)]
#[path = "page_tasks_tests.rs"]
mod tests;
