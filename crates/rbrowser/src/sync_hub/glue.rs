//! [`HostSync`]: the hub of one window, fed by the agents' reports and the host's view events.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};

use rb_config::SyncConfig;
use rb_engine::{Target, ViewEvent, ViewHost, ViewId};
use rb_ipc::SyncState;
use rb_sync::{
    AgentConfig, AgentMessage, EngineRouter, Hub, HubError, Replay, SubmitPolicy, SyncHub, SyncKind,
};
use tokio::runtime::Handle;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::watch;

use super::switches_from;
use crate::form_sync::SkipHint;
use crate::host::Host;

/// The hub of one window.
pub struct HostSync {
    me: Weak<HostSync>,
    host: Weak<Host>,
    hub: Arc<SyncHub>,
    /// The replay per engine the hub runs (RB-040-01): Chromium's for the grid, Firefox's and
    /// WebKit's for the external windows.
    router: Arc<EngineRouter>,
    runtime: Handle,
    /// Serialises the agents' configuration, so the last state always wins.
    configuring: tokio::sync::Mutex<()>,
    /// The reconfigurations asked for so far, and the last one done ([`HostSync::configured`]).
    requested: AtomicU64,
    configured: watch::Sender<u64>,
    /// The label strip that says the next click is not synced (RB-020-05).
    skip_hint: SkipHint,
}

impl HostSync {
    /// The hub of `host`'s window, following its views and agents on `runtime`. It replays
    /// nothing until the engine's replay is set (RB-020-03).
    pub fn start(host: &Arc<Host>, runtime: &Handle) -> Arc<Self> {
        let sync = host.config().sync;
        let router = Arc::new(EngineRouter::default());
        let sync = Arc::new_cyclic(|me: &Weak<HostSync>| {
            let changed = me.clone();
            let hub = SyncHub::new(
                Hub::new(switches_from(&sync)),
                router.clone(),
                timeout_of(&sync),
                runtime.clone(),
                Arc::new(move || {
                    if let Some(sync) = changed.upgrade() {
                        sync.publish();
                    }
                }),
            );
            HostSync {
                me: me.clone(),
                host: Arc::downgrade(host),
                hub,
                router,
                runtime: runtime.clone(),
                configuring: tokio::sync::Mutex::new(()),
                requested: AtomicU64::new(0),
                configured: watch::Sender::new(0),
                skip_hint: SkipHint::default(),
            }
        });
        sync.views_changed();
        sync.watch_reports(host);
        sync.watch_views(host);
        sync
    }

    /// Chromium's replay from now on (RB-020-03); the router sends it the grid's views.
    pub fn set_replay(&self, replay: Arc<dyn Replay>) {
        self.router
            .set_engine(rb_engine::EngineKind::Chromium, replay);
    }

    /// The replay per engine and the views' routes (RB-040-01).
    pub fn router(&self) -> &Arc<EngineRouter> {
        &self.router
    }

    /// Who besides the leader submits a form from now on (RB-020-07's sessions).
    pub fn set_submit_policy(&self, policy: Arc<dyn SubmitPolicy>) {
        self.hub.with(|hub| hub.set_submit_policy(policy));
    }

    /// The routing state, for a look or a change; the caller publishes and reconfigures.
    pub(crate) fn hub(&self) -> &SyncHub {
        &self.hub
    }

    /// The configuration `view`'s agent should have, with the scroll mode in effect (the one
    /// `sync_set` set at runtime, else `sync.scrollMode`; RB-090-33).
    pub fn agent_config(&self, sync: &SyncConfig, view: ViewId) -> AgentConfig {
        let mode = self
            .host
            .upgrade()
            .map_or(sync.scroll_mode, |host| crate::scroll_sync::mode(&host));
        self.hub.with(|hub| AgentConfig {
            active: hub.captures(view),
            capture: hub.switches().capture(),
            hover_throttle_ms: sync.hover_throttle_ms,
            scroll_debounce_ms: sync.scroll_debounce_ms,
            scroll_mode: crate::sync_agent::scroll_mode_of(mode),
        })
    }

    /// The scroll mode changed at runtime (`sync_set { scrollMode }`): the agents follow, so
    /// only wheel mode listens for the wheel (RB-090-33).
    pub fn scroll_mode_changed(&self) {
        // The published state names the mode: the Firefox views' agents follow it.
        self.changed();
    }

    /// The sync state the shell shows (and `sync_set` answers), with navigation sync's part
    /// (RB-020-06).
    pub fn state(&self, sync: &SyncConfig) -> SyncState {
        let status = self.hub.with(|hub| hub.status());
        let navigation = self
            .host
            .upgrade()
            .map(|host| crate::nav_sync::status(&host))
            .unwrap_or_default();
        let switches = status.switches;
        let scroll_mode = self
            .host
            .upgrade()
            .map_or(sync.scroll_mode, |host| crate::scroll_sync::mode(&host));
        SyncState {
            enabled: switches.enabled,
            features: SyncConfig {
                navigation: switches.navigation,
                scroll: switches.scroll,
                click: switches.click,
                input: switches.input,
                keyboard: switches.keyboard,
                hover: switches.hover,
                forms: switches.forms,
                focus: switches.focus,
                leader_only_submit: switches.leader_only_submit,
                scroll_mode,
                ..sync.clone()
            },
            excluded: status.excluded,
            desynced: status.desynced,
            navigation_decoupled: navigation.decoupled,
            browsersync: navigation.browsersync,
            skip_next_click: status.skip_next_click,
        }
    }

    /// `Leader S N`: the active view's next click is not synced; its label strip says so.
    pub fn skip_next_click(&self) {
        self.hub.with(|hub| hub.skip_next_click());
        self.publish();
    }

    /// Sync on or off.
    pub fn set_enabled(&self, on: bool) {
        self.hub.with(|hub| hub.set_enabled(on));
        self.changed();
    }

    /// One kind on or off.
    pub fn set_kind(&self, kind: SyncKind, on: bool) {
        self.hub.with(|hub| hub.set_kind(kind, on));
        self.changed();
    }

    /// `view` out of sync (`excluded`) or back in.
    pub fn exclude(&self, view: ViewId, excluded: bool) -> Result<(), HubError> {
        self.hub.with(|hub| hub.exclude(view, excluded))?;
        self.changed();
        Ok(())
    }

    /// `view` loads the leader's URL; its badge goes. Without a URL to load, or when the load
    /// does not start, the view takes events again at once.
    pub fn resync(&self, view: ViewId) -> Result<(), HubError> {
        let leader = self.hub.with(|hub| hub.resync(view, Instant::now()))?;
        if let Some(leader) = leader {
            let url = self
                .host
                .upgrade()
                .and_then(|host| host.view(leader))
                .map(|info| info.url)
                .unwrap_or_default();
            if url.is_empty() {
                self.hub.with(|hub| hub.load_failed(view));
            } else {
                self.load(view, &url, "resync failed");
            }
        }
        self.publish();
        Ok(())
    }

    /// `view` loads `url`, which the hub marked as its load; a navigation that does not start
    /// takes the mark back.
    fn load(&self, view: ViewId, url: &str, failed: &'static str) {
        let result = match self.host.upgrade() {
            Some(host) => host.navigate(Target::View(view), url),
            None => Err(rb_engine::EngineError::Invalid(
                "the window is closed".into(),
            )),
        };
        if let Err(error) = result {
            tracing::warn!(%view, %error, "{failed}");
            self.hub.with(|hub| hub.load_failed(view));
        }
    }

    /// Waits until the agents have the configuration of every change made so far.
    pub async fn configured(&self) {
        let wanted = self.requested.load(Ordering::SeqCst);
        let mut done = self.configured.subscribe();
        // The sender lives as long as `self`.
        let _ = done.wait_for(|done| *done >= wanted).await;
    }

    /// Waits - at most `timeout` - until the agents have their configuration and no view loads
    /// a URL the hub gave it (`sync_set` answers after its work, RB-030-01).
    pub async fn settled(&self, timeout: Duration) {
        let work = async {
            self.configured().await;
            while self.any_loading() {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        };
        // Out of time: the state as it is.
        let _ = tokio::time::timeout(timeout, work).await;
    }

    fn any_loading(&self) -> bool {
        let Some(host) = self.host.upgrade() else {
            return false;
        };
        let now = Instant::now();
        host.views()
            .iter()
            .any(|info| self.hub.with(|hub| hub.is_loading(info.id, now)))
    }

    /// The project's `sync.*` changed.
    pub fn config_changed(&self) {
        let Some(host) = self.host.upgrade() else {
            return;
        };
        let sync = host.config().sync;
        self.hub.with(|hub| hub.set_switches(switches_from(&sync)));
        self.hub.set_timeout(timeout_of(&sync));
        self.changed();
    }

    /// A switch or the active view changed: the agents and the shell follow.
    fn changed(&self) {
        self.reconfigure();
        self.publish();
    }

    /// Puts the current sync state into the shell's state.
    pub(crate) fn publish(&self) {
        let Some(host) = self.host.upgrade() else {
            return;
        };
        let state = self.state(&host.config().sync);
        let armed = self
            .hub
            .with(|hub| hub.active().filter(|_| hub.skip_next_armed()));
        host.update_shell_state(|shell| shell.sync = Some(state));
        self.skip_hint.show(&host, armed);
    }

    /// Sends every agent whose configuration is not the wanted one the wanted one - computed
    /// when the task runs, so the last change always wins.
    fn reconfigure(&self) {
        let Some(sync) = self.me.upgrade() else {
            return;
        };
        let number = self.requested.fetch_add(1, Ordering::SeqCst) + 1;
        self.runtime.spawn(async move {
            let serial = sync.configuring.lock().await;
            if let Some(host) = sync.host.upgrade() {
                let config = host.config().sync;
                let agents = host.sync_agents().clone();
                let mut wanted: Vec<(ViewId, AgentConfig)> = host
                    .views()
                    .iter()
                    .map(|info| (info.id, sync.agent_config(&config, info.id)))
                    .filter(|(view, wanted)| agents.needs_config(*view, wanted))
                    .collect();
                // The old active view stops capturing before the new one starts.
                wanted.sort_by_key(|(_, wanted)| wanted.active);
                for (view, config) in wanted {
                    agents.configure(view, config).await;
                }
            }
            drop(serial);
            sync.configured
                .send_modify(|done| *done = (*done).max(number));
        });
    }

    fn views_changed(&self) {
        let Some(host) = self.host.upgrade() else {
            return;
        };
        let views: Vec<ViewId> = host.views().iter().map(|info| info.id).collect();
        self.hub.set_views(&views);
        let active = host.active_view();
        self.hub.with(|hub| hub.set_active(active));
        self.changed();
    }

    fn active_changed(&self, active: Option<ViewId>) {
        if self.hub.with(|hub| hub.set_active(active)) {
            self.reconfigure();
            // The skip hint moves with the active view (RB-020-05).
            if self.hub.with(|hub| hub.skip_next_armed()) {
                self.publish();
            }
        }
    }

    fn navigated(&self, view: ViewId, url: &str) {
        let follow = self
            .hub
            .with(|hub| hub.navigated(view, url, Instant::now()));
        for navigation in follow {
            self.load(navigation.view, &navigation.url, "follow failed");
        }
        self.publish();
    }

    /// `view`'s title, size or loading state changed: the end of a load the hub gave, or of a
    /// leader's load after a leader-only submit that stayed on its URL (the followers load it).
    fn updated(&self, view: ViewId) {
        let Some(host) = self.host.upgrade() else {
            return;
        };
        let (Some(info), Some(ended)) = (host.view(view), host.loads_ended(view)) else {
            return;
        };
        let state = (info.loading, ended);
        let follow = self
            .hub
            .with(|hub| hub.load_state(view, state, &info.url, Instant::now()));
        for navigation in follow {
            self.load(navigation.view, &navigation.url, "follow failed");
        }
    }

    fn watch_reports(&self, host: &Host) {
        let mut reports = host.sync_agents().subscribe();
        let weak = self.me.clone();
        self.runtime.spawn(async move {
            loop {
                let report = match reports.recv().await {
                    Ok(report) => report,
                    Err(RecvError::Lagged(missed)) => {
                        tracing::warn!(missed, "sync events dropped");
                        continue;
                    }
                    Err(RecvError::Closed) => return,
                };
                let Some(sync) = weak.upgrade() else {
                    return;
                };
                if let AgentMessage::Event(message) = &report.message {
                    // Input in a view: hot reload's interval reload waits (RB-030-05).
                    crate::reload::note_input();
                    let route = sync.hub.on_event(report.view, message);
                    tracing::trace!(view = %report.view, seq = message.seq, ?route, "sync event");
                }
            }
        });
    }

    fn watch_views(&self, host: &Host) {
        let mut events = host.subscribe();
        let weak = self.me.clone();
        self.runtime.spawn(async move {
            loop {
                let event = events.recv().await;
                let Some(sync) = weak.upgrade() else {
                    return;
                };
                match event {
                    Ok(ViewEvent::Opened(_) | ViewEvent::Closed(_) | ViewEvent::LayoutChanged)
                    | Err(RecvError::Lagged(_)) => sync.views_changed(),
                    Ok(ViewEvent::ActiveChanged(active)) => sync.active_changed(active),
                    Ok(ViewEvent::Navigated { view, url }) => sync.navigated(view, &url),
                    Ok(ViewEvent::Updated(view)) => sync.updated(view),
                    Ok(_) => {}
                    Err(RecvError::Closed) => return,
                }
            }
        });
    }
}

fn timeout_of(sync: &SyncConfig) -> Duration {
    Duration::from_millis(u64::from(sync.desync_timeout_ms))
}

#[cfg(test)]
#[path = "glue_tests.rs"]
mod tests;
