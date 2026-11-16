//! Firefox and the grid, both ways. A Firefox view keeps its cell, whose Chromium view stands in
//! for it (RB-040-01): every navigation the grid gives the view - the address bar, MCP, a
//! follow of the leader - loads in the stand-in first, and Firefox follows the stand-in's URL.
//! With `engines.firefox.syncSource` on, Firefox leads as well: a click in its window makes it
//! the active view, its agent's events go to the SyncHub like a grid view's, and its own
//! navigations load in the stand-in, from where the other views follow. A URL either side
//! already shows is never loaded again, so the two never chase each other. Taking the lead goes
//! through the host like any change of the active view, so the hub reconfigures every agent;
//! a Firefox view out of sync (sync off, the view excluded) never leads.

use std::sync::Arc;
use std::time::Duration;

use rb_bidi::FirefoxEngine;
use rb_bidi::firefox::AgentReport;
use rb_engine::{Target, ViewEvent, ViewHost, ViewId};
use rb_sync::{AgentMessage, PointerPhase, SyncEvent};
use tokio::runtime::Handle;
use tokio::sync::broadcast::error::RecvError;
use tokio::task::JoinHandle;

use super::FirefoxBridge;
use crate::cdp::lock;
use crate::host::Host;

/// The tasks that follow `firefox`: its agents' reports and its own navigations.
pub(super) fn start(
    bridge: &Arc<FirefoxBridge>,
    firefox: &Arc<FirefoxEngine>,
    runtime: &Handle,
) -> Vec<JoinHandle<()>> {
    let mut reports = firefox.reports();
    let mut navigations = firefox.navigations();
    let lead = {
        let bridge = Arc::downgrade(bridge);
        runtime.spawn(async move {
            loop {
                let report = match reports.recv().await {
                    Ok(report) => report,
                    Err(RecvError::Lagged(missed)) => {
                        tracing::warn!(missed, "Firefox sync events dropped");
                        continue;
                    }
                    Err(RecvError::Closed) => return,
                };
                let Some(bridge) = bridge.upgrade() else {
                    return;
                };
                bridge.on_report(&report).await;
            }
        })
    };
    let mirror = {
        let bridge = Arc::downgrade(bridge);
        runtime.spawn(async move {
            loop {
                let (view, url) = match navigations.recv().await {
                    Ok(navigation) => navigation,
                    Err(RecvError::Lagged(_)) => continue,
                    Err(RecvError::Closed) => return,
                };
                let Some(bridge) = bridge.upgrade() else {
                    return;
                };
                bridge.firefox_navigated(view, &url);
            }
        })
    };
    vec![lead, mirror]
}

/// Follows `host`'s views: a stand-in's navigation goes to Firefox, a view's changed emulation
/// and a changed trust or sync state too ([`super::live`]); a layout with a Firefox breakpoint
/// while Firefox is missing gets the notice.
pub(super) fn watch_host(bridge: &Arc<FirefoxBridge>, host: &Host) {
    let mut events = host.subscribe();
    let weak = Arc::downgrade(bridge);
    bridge.runtime.spawn(async move {
        loop {
            let event = events.recv().await;
            let Some(bridge) = weak.upgrade() else {
                return;
            };
            match event {
                Ok(ViewEvent::Navigated { view, url }) => bridge.grid_navigated(view, &url).await,
                Ok(ViewEvent::Opened(_) | ViewEvent::LayoutChanged) => bridge.notify_if_missing(),
                Ok(ViewEvent::Updated(view)) => bridge.view_updated(view),
                Ok(ViewEvent::StateChanged) => bridge.state_changed(),
                Ok(_) | Err(RecvError::Lagged(_)) => {}
                Err(RecvError::Closed) => return,
            }
        }
    });
}

/// Whether the person's `event` in a Firefox window makes it the active view: a press or a
/// click (keys follow a click into the page).
pub fn takes_lead(event: &SyncEvent) -> bool {
    match event {
        SyncEvent::Pointer(pointer) => pointer.phase == PointerPhase::Down,
        SyncEvent::Click(_) | SyncEvent::Contextmenu(_) => true,
        _ => false,
    }
}

impl FirefoxBridge {
    /// A Firefox agent reported: its event goes to the hub, the view leading on a click - when
    /// sync is on and the view is not excluded.
    pub(super) async fn on_report(&self, report: &AgentReport) {
        // The Web Vitals of a Firefox view, whatever its sync role (RB-060-10).
        if let AgentMessage::Vitals(_) = &report.message {
            if let Some(host) = self.host() {
                crate::vitals::on_agent_message(&host, report.view, &report.message);
            }
            return;
        }
        if !self.sync_source() {
            return;
        }
        let AgentMessage::Event(message) = &report.message else {
            return;
        };
        let Some(host) = self.host() else {
            return;
        };
        let Some(sync) = crate::sync_hub::for_host(&host) else {
            return;
        };
        let view = report.view;
        let (enabled, excluded) = sync
            .hub()
            .with(|hub| (hub.switches().enabled, hub.is_excluded(view)));
        if !enabled || excluded {
            return;
        }
        let leads = takes_lead(&message.event);
        if leads && host.active_view() != Some(view) {
            if let Err(error) = host.set_active_view(view) {
                tracing::debug!(%view, %error, "the Firefox view could not lead");
                return;
            }
            // The person types here now: nothing the replay held stays down.
            if let Some(firefox) = self.firefox() {
                firefox.release_input(view).await;
            }
        }
        // The hub learns of the lead from the host's event (and reconfigures the agents); this
        // event routes once it has.
        if leads && !hub_follows(&sync, view).await {
            tracing::debug!(%view, "the hub did not take the Firefox view as leader in time");
        }
        let route = sync.hub().on_event(view, message);
        tracing::trace!(%view, seq = message.seq, ?route, "Firefox sync event");
    }

    /// Firefox committed `url` in `view`: the stand-in loads it (only when Firefox may lead).
    pub(super) fn firefox_navigated(&self, view: ViewId, url: &str) {
        if !self.sync_source() {
            return;
        }
        let Some(host) = self.host() else {
            return;
        };
        if host.view(view).is_some_and(|info| info.url != url)
            && let Err(error) = host.navigate(Target::View(view), url)
        {
            tracing::debug!(%view, %error, "the stand-in did not follow Firefox");
        }
    }

    /// The stand-in of `view` committed `url`: Firefox loads it unless it shows it already.
    pub(super) async fn grid_navigated(&self, view: ViewId, url: &str) {
        let Some(firefox) = self.firefox() else {
            return;
        };
        let Some(state) = firefox.view_state(view) else {
            return;
        };
        if state.url() == url {
            return;
        }
        let handle = lock(&self.windows)
            .upgrade()
            .and_then(|windows| windows.handle(view));
        if let Some(handle) = handle
            && let Err(error) = handle.navigate(url).await
        {
            tracing::debug!(%view, %error, "Firefox did not follow its stand-in");
        }
    }
}

/// How long a lead taken in Firefox waits for the hub to hear of it.
const LEAD_WAIT: Duration = Duration::from_millis(500);

/// Waits - at most [`LEAD_WAIT`] - until the hub has `view` as its active view.
async fn hub_follows(sync: &crate::sync_hub::HostSync, view: ViewId) -> bool {
    let wait = async {
        while sync.hub().with(|hub| hub.active()) != Some(view) {
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    };
    tokio::time::timeout(LEAD_WAIT, wait).await.is_ok()
}

#[cfg(test)]
#[path = "follow_tests.rs"]
mod tests;
