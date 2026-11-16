//! [`NavSyncHost`]: navigation sync for one window, fed by the host's view events, the CDP
//! events of the views' pages and the agents' reports.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, Weak};
use std::time::Instant;

use rb_engine::{EngineError, Target, ViewEvent, ViewHost, ViewId};
use rb_ipc::{CoreToShell, Notification, NotificationLevel};
use rb_sync::nav_url::origin_of;
use rb_sync::{
    AgentCommand, AgentMessage, CommandResult, DetectedTool, FollowMode, Follower, NavFollow,
    NavigationSync, Settle,
};
use serde_json::Value;
use tokio::runtime::Handle;
use tokio::sync::broadcast::error::RecvError;

use super::{
    BROWSERSYNC_PAUSED_KEY, BrowsersyncGuard, DETECT_BROWSERSYNC_KEY, GuardAction, NavStatus,
    configured_decoupled, options_from,
};
use crate::cdp::{CdpEvent, lock};
use crate::control::Core;
use crate::host::Host;
use crate::sync_hub;

/// Navigation sync of one window.
pub struct NavSyncHost {
    me: Weak<NavSyncHost>,
    host: Weak<Host>,
    core: Weak<Core>,
    runtime: Handle,
    nav: Mutex<NavigationSync>,
    guard: Mutex<BrowsersyncGuard>,
}

impl NavSyncHost {
    /// Navigation sync of `core`'s window, following its views, pages and agents.
    pub fn start(core: &Core) -> Arc<Self> {
        let host = core.host();
        let config = host.config();
        let nav = Arc::new_cyclic(|me: &Weak<NavSyncHost>| NavSyncHost {
            me: me.clone(),
            host: Arc::downgrade(host),
            core: core.arc().as_ref().map(Arc::downgrade).unwrap_or_default(),
            runtime: core.runtime().clone(),
            nav: Mutex::new(NavigationSync::new(options_from(&config))),
            guard: Mutex::new(BrowsersyncGuard::default()),
        });
        nav.views_changed();
        nav.watch_views(host);
        nav.watch_pages(host);
        nav.watch_reports(host);
        nav
    }

    /// Whether `view` leads and follows navigations.
    pub fn is_coupled(&self, view: ViewId) -> bool {
        lock(&self.nav).is_coupled(view)
    }

    /// Couples `view`'s navigation or decouples it; `true` when that changed.
    pub fn couple(&self, view: ViewId, coupled: bool) -> Result<bool, EngineError> {
        let host = self.host()?;
        if host.view(view).is_none() {
            return Err(EngineError::UnknownView(view));
        }
        let changed = lock(&self.nav).couple(view, coupled);
        if changed {
            self.changed(&host);
        }
        Ok(changed)
    }

    /// What the shell and MCP show.
    pub fn status(&self) -> NavStatus {
        let views: Vec<ViewId> = self
            .host
            .upgrade()
            .map(|host| host.views().iter().map(|info| info.id).collect())
            .unwrap_or_default();
        NavStatus {
            decoupled: lock(&self.nav).decoupled(&views),
            browsersync: lock(&self.guard).paused().map(str::to_owned),
        }
    }

    /// The project config changed: options, the decoupled views (runtime changes go) and,
    /// with `sync.detectBrowsersync` off, the guard.
    pub fn config_changed(&self) {
        let Ok(host) = self.host() else {
            return;
        };
        let config = host.config();
        let decoupled = configured_decoupled(&host, &config);
        {
            let mut nav = lock(&self.nav);
            nav.set_options(options_from(&config));
            nav.clear_overrides();
            nav.set_configured(decoupled);
        }
        if !config.sync.detect_browsersync {
            lock(&self.guard).reset();
        }
        self.changed(&host);
    }

    fn host(&self) -> Result<Arc<Host>, EngineError> {
        self.host
            .upgrade()
            .ok_or_else(|| EngineError::Invalid("the window is closed".to_owned()))
    }

    /// The decoupled views reach the hub (they follow no submit) and the shell.
    fn changed(&self, host: &Host) {
        let decoupled = self.status().decoupled;
        if let Some(sync) = sync_hub::for_host(host) {
            sync.hub()
                .with(|hub| hub.set_navigation_decoupled(&decoupled));
            sync.publish();
        }
    }

    fn views_changed(&self) {
        let Ok(host) = self.host() else {
            return;
        };
        let views: Vec<ViewId> = host.views().iter().map(|info| info.id).collect();
        let decoupled = configured_decoupled(&host, &host.config());
        {
            let mut nav = lock(&self.nav);
            nav.retain(&views);
            nav.set_configured(decoupled);
        }
        self.changed(&host);
    }

    /// `view` committed `url`: the active view's navigation arms the check.
    fn committed(&self, view: ViewId, url: &str) {
        let Ok(host) = self.host() else {
            return;
        };
        let leads = sync_hub::for_host(&host).is_some_and(|sync| {
            sync.hub().with(|hub| {
                let switches = hub.switches();
                switches.enabled
                    && switches.navigation
                    && hub.active() == Some(view)
                    && !hub.is_excluded(view)
            })
        });
        let settle = lock(&self.nav).committed(view, url, leads);
        if let Some(settle) = settle {
            self.schedule(settle);
        }
    }

    fn schedule(&self, settle: Settle) {
        let weak = self.me.clone();
        self.runtime.spawn(async move {
            tokio::time::sleep(settle.delay).await;
            if let Some(nav) = weak.upgrade() {
                nav.settle(settle.generation);
            }
        });
    }

    /// The check `generation` is due: the followers that are elsewhere follow.
    fn settle(&self, generation: u64) {
        let Ok(host) = self.host() else {
            return;
        };
        let Some(sync) = sync_hub::for_host(&host) else {
            return;
        };
        let views = host.views();
        let now = Instant::now();
        let (leader, loading) = sync.hub().with(|hub| {
            let switches = hub.switches();
            let leader = hub.active().filter(|leader| {
                switches.enabled && switches.navigation && !hub.is_excluded(*leader)
            });
            let loading: BTreeMap<ViewId, bool> = views
                .iter()
                .filter(|info| !hub.is_excluded(info.id))
                .map(|info| (info.id, info.loading || hub.is_loading(info.id, now)))
                .collect();
            (leader, loading)
        });
        let Some(leader) = leader else {
            lock(&self.nav).cancel();
            return;
        };
        let Some(leader_url) = views
            .iter()
            .find(|info| info.id == leader)
            .map(|info| info.url.clone())
        else {
            return;
        };
        let followers: Vec<Follower> = views
            .iter()
            .filter_map(|info| {
                loading.get(&info.id).map(|loading| Follower {
                    view: info.id,
                    url: info.url.clone(),
                    loading: *loading,
                })
            })
            .collect();
        let outcome = lock(&self.nav).settle(generation, leader, &leader_url, &followers);
        for follow in outcome.follows {
            self.follow(&host, follow);
        }
        if let Some(retry) = outcome.retry {
            self.schedule(retry);
        }
    }

    fn follow(&self, host: &Arc<Host>, follow: NavFollow) {
        match follow.mode {
            FollowMode::Load => load(host, follow.view, &follow.url),
            FollowMode::History => {
                let host = host.clone();
                self.runtime.spawn(async move {
                    let command = AgentCommand::Navigate {
                        url: follow.url.clone(),
                    };
                    match host.sync_agents().command(follow.view, &command).await {
                        Ok(CommandResult::Done) => {}
                        // No agent, another origin by now, a page that threw: load it.
                        other => {
                            let view = follow.view;
                            tracing::debug!(%view, ?other, "route not followed in its document");
                            load(&host, follow.view, &follow.url);
                        }
                    }
                });
            }
        }
    }

    /// A page's own CDP events: a navigation within the document, a new document.
    fn on_page_event(&self, event: &CdpEvent) {
        let (frame, within) = match event.method.as_str() {
            "Page.navigatedWithinDocument" => (event.params["frameId"].as_str(), true),
            "Page.frameNavigated" if event.params.pointer("/frame/parentId").is_none() => (
                event.params.pointer("/frame/id").and_then(Value::as_str),
                false,
            ),
            _ => return,
        };
        let (Some(frame), Ok(host)) = (frame, self.host()) else {
            return;
        };
        // A top frame's id is its view's target id.
        let Some(view) = host
            .views()
            .iter()
            .map(|info| info.id)
            .find(|view| host.target_id(*view).as_deref() == Some(frame))
        else {
            return;
        };
        let mut nav = lock(&self.nav);
        match (within, event.params["url"].as_str()) {
            (true, Some(url)) => nav.within_document(view, url),
            (true, None) => {}
            (false, _) => nav.cross_document(view),
        }
    }

    /// `view`'s agent found Browsersync in its page.
    fn on_browsersync(&self, view: ViewId) {
        let Ok(host) = self.host() else {
            return;
        };
        let Some(origin) = host.view(view).and_then(|info| origin_of(&info.url)) else {
            return;
        };
        let Some(sync) = sync_hub::for_host(&host) else {
            return;
        };
        let detect = host.config().sync.detect_browsersync;
        let sync_on = sync.hub().with(|hub| hub.switches().enabled);
        let action = lock(&self.guard).detected(&origin, detect, sync_on);
        match action {
            GuardAction::Pause => {
                tracing::info!(%view, "Browsersync's ghost mode found: sync off");
                lock(&self.nav).cancel();
                sync.set_enabled(false);
                if let Some(core) = self.core.upgrade() {
                    core.notify(CoreToShell::Notification(Notification {
                        level: NotificationLevel::Warning,
                        message_key: BROWSERSYNC_PAUSED_KEY.to_owned(),
                        args: [("origin".to_owned(), origin)].into(),
                        settings_key: Some(DETECT_BROWSERSYNC_KEY.to_owned()),
                    }));
                }
            }
            GuardAction::Allow => sync.publish(),
            GuardAction::Nothing => {}
        }
    }

    fn watch_views(&self, host: &Host) {
        let mut events = host.subscribe();
        let weak = self.me.clone();
        self.runtime.spawn(async move {
            loop {
                let event = events.recv().await;
                let Some(nav) = weak.upgrade() else {
                    return;
                };
                match event {
                    Ok(ViewEvent::Opened(_) | ViewEvent::Closed(_) | ViewEvent::LayoutChanged)
                    | Err(RecvError::Lagged(_)) => nav.views_changed(),
                    Ok(ViewEvent::ActiveChanged(_)) => lock(&nav.nav).cancel(),
                    Ok(ViewEvent::Navigated { view, url }) => nav.committed(view, &url),
                    Ok(_) => {}
                    Err(RecvError::Closed) => return,
                }
            }
        });
    }

    fn watch_pages(&self, host: &Host) {
        let mut events = host.cdp().subscribe();
        let weak = self.me.clone();
        self.runtime.spawn(async move {
            loop {
                let event = match events.recv().await {
                    Ok(event) => event,
                    Err(RecvError::Lagged(_)) => continue,
                    Err(RecvError::Closed) => return,
                };
                let Some(nav) = weak.upgrade() else {
                    return;
                };
                nav.on_page_event(&event);
            }
        });
    }

    fn watch_reports(&self, host: &Host) {
        let mut reports = host.sync_agents().subscribe();
        let weak = self.me.clone();
        self.runtime.spawn(async move {
            loop {
                let report = match reports.recv().await {
                    Ok(report) => report,
                    Err(RecvError::Lagged(_)) => continue,
                    Err(RecvError::Closed) => return,
                };
                let Some(nav) = weak.upgrade() else {
                    return;
                };
                if let AgentMessage::Detected(detected) = &report.message
                    && detected.top
                    && detected.tool == DetectedTool::Browsersync
                {
                    nav.on_browsersync(report.view);
                }
            }
        });
    }
}

/// `view` loads `url`; until it commits or its load ends, the hub routes it no events - marked
/// before the navigation starts, so no event slips into the document that is going away, and
/// taken back when it does not start.
fn load(host: &Host, view: ViewId, url: &str) {
    let sync = sync_hub::for_host(host);
    if let Some(sync) = &sync {
        sync.hub()
            .with(|hub| hub.mark_loading(view, Instant::now()));
    }
    if let Err(error) = host.navigate(Target::View(view), url) {
        tracing::warn!(%view, %error, "navigation sync: follow failed");
        if let Some(sync) = &sync {
            sync.hub().with(|hub| hub.load_failed(view));
        }
    }
}

#[cfg(test)]
#[path = "glue_tests.rs"]
mod tests;
