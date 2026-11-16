//! The running Firefox kept in step with the app: each view's emulation as the host has it -
//! after `emulation.set`, a changed breakpoint or preset, a changed workspace trust - and the
//! agents' configuration as the SyncHub's switches have it. The host's events drive both
//! ([`super::follow`]): `Updated(view)` for one view's emulation, `StateChanged` (the trust, the
//! sync switches) for everything.
//!
//! The project's headers reach Firefox only in a trusted workspace, as in Chromium
//! (`host::attach::apply_metrics`). Chromium keeps the headers set live with `emulation.set` in
//! an untrusted one; the host does not hand its live overrides out, so Firefox sends no headers
//! at all there.

use std::sync::{Arc, Weak};

use rb_bidi::firefox::EmulationFilter;
use rb_engine::{Emulation, ViewHost, ViewId};
use rb_sync::AgentConfig;

use super::FirefoxBridge;
use crate::cdp::lock;
use crate::host::Host;

/// What of `emulation` reaches a Firefox view of `host`: all of it in a trusted workspace, no
/// headers in an untrusted one.
pub fn firefox_emulation(host: &Host, emulation: &Emulation) -> Emulation {
    let mut emulation = emulation.clone();
    if !host.shell_state().trusted {
        emulation.headers.clear();
    }
    emulation
}

/// The engine's filter for `host`'s window; without the window, no headers.
pub(super) fn filter_for(host: Weak<Host>) -> EmulationFilter {
    Arc::new(move |_view, emulation| match host.upgrade() {
        Some(host) => firefox_emulation(&host, emulation),
        None => Emulation {
            headers: Default::default(),
            ..emulation.clone()
        },
    })
}

/// The Firefox agents' configuration: the project's `sync.*` with the hub's switches over it
/// (sync off, a kind off). They capture while Firefox may lead and sync is on; the hub routes
/// only the active view's events.
pub(super) fn agent_config(host: &Host, sync_source: bool) -> AgentConfig {
    let mut agent = crate::sync_agent::agent_config(&host.config().sync, sync_source);
    if let Some(sync) = crate::sync_hub::for_host(host) {
        let switches = sync.hub().with(|hub| hub.switches());
        agent.capture = switches.capture();
        agent.active = sync_source && switches.enabled;
    }
    agent
}

impl FirefoxBridge {
    /// The host's emulation of `only` (every Firefox view with `None`) to its window where the
    /// window has another. One run at a time, each reading the state when it runs: the last
    /// change wins.
    pub(super) async fn sync_emulation(&self, only: Option<ViewId>) {
        let _serial = self.emulating.lock().await;
        let (Some(host), Some(firefox)) = (self.host(), self.firefox()) else {
            return;
        };
        let windows = lock(&self.windows).upgrade();
        for state in firefox.live_views() {
            if only.is_some_and(|view| view != state.id) {
                continue;
            }
            let Some(wanted) = host.emulation(state.id) else {
                continue;
            };
            if state.applied_emulation() == Some(firefox_emulation(&host, &wanted)) {
                continue;
            }
            let Some(handle) = windows
                .as_ref()
                .and_then(|windows| windows.handle(state.id))
            else {
                continue;
            };
            if let Err(error) = handle.apply_emulation(&wanted).await {
                tracing::debug!(view = %state.id, %error, "Firefox did not take the emulation");
            }
        }
    }

    /// The agents get the configuration again when the hub's switches changed it.
    pub(super) fn sync_agents(&self) {
        let (Some(host), Some(firefox)) = (self.host(), self.firefox()) else {
            return;
        };
        let wanted = agent_config(&host, self.sync_source());
        if firefox.agent_config() != wanted {
            firefox.set_agent_config(wanted);
        }
    }

    /// Something the shell shows changed - the trust, the sync switches: the agents and every
    /// view's emulation again.
    pub(super) fn state_changed(self: &Arc<Self>) {
        self.sync_agents();
        let me = self.clone();
        self.runtime
            .spawn(async move { me.sync_emulation(None).await });
    }

    /// `view`'s title, size or emulation changed: its emulation again.
    pub(super) fn view_updated(self: &Arc<Self>, view: ViewId) {
        let me = self.clone();
        self.runtime
            .spawn(async move { me.sync_emulation(Some(view)).await });
    }
}

#[cfg(test)]
#[path = "live_tests.rs"]
mod tests;
