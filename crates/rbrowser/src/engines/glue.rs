//! [`HostEngines`]: the external windows of one window, fed by the host's views; each running
//! one is asked into its grid cell (`super::embed`, RB-080-39).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak};

use rb_config::{Breakpoint, EmulationFields, EngineKind, ExternalWindowSettings};
use rb_engine::external::{Backoff, ExternalStatus, ExternalView, ExternalWindows};
use rb_engine::unsupported::UnsupportedEngine;
use rb_engine::{Engine, ViewEvent, ViewHost, ViewId, ViewInfo, ViewSpec, engine_name};
use rb_sync::ViewRoute;
use tokio::runtime::Handle;
use tokio::sync::broadcast::error::RecvError;

use super::embed::Embedding;
use super::placement_of;
use crate::cdp::lock;
use crate::host::Host;

/// The external windows of one window.
pub struct HostEngines {
    pub(super) me: Weak<HostEngines>,
    pub(super) host: Weak<Host>,
    pub(super) runtime: Handle,
    pub(super) windows: Arc<ExternalWindows>,
    show_tiles: AtomicBool,
    /// The views in their cells, released or failed (RB-080-39).
    pub(super) embedding: Mutex<Embedding>,
}

impl HostEngines {
    /// The external windows of `host`'s window, following its views on `runtime`. Firefox and
    /// WebKit are "not available yet" until their jobs plug in ([`HostEngines::windows`]).
    pub fn start(
        host: &Arc<Host>,
        runtime: &Handle,
        settings: &ExternalWindowSettings,
    ) -> Arc<Self> {
        Self::start_with(host, runtime, settings, Vec::new())
    }

    /// [`HostEngines::start`] with `engines` in place of the stand-ins of their kinds, set
    /// before the first view starts (RB-040-02: Firefox).
    pub fn start_with(
        host: &Arc<Host>,
        runtime: &Handle,
        settings: &ExternalWindowSettings,
        engines: Vec<Arc<dyn Engine>>,
    ) -> Arc<Self> {
        let plugged = engines;
        let engines = Arc::new_cyclic(|me: &Weak<HostEngines>| {
            let changed = me.clone();
            let windows = ExternalWindows::new(
                runtime.clone(),
                Backoff::default(),
                settings.restart_on_crash,
                Arc::new(move || {
                    if let Some(engines) = changed.upgrade() {
                        engines.publish();
                    }
                }),
            );
            for kind in [EngineKind::Firefox, EngineKind::Webkit] {
                windows.set_engine(Arc::new(UnsupportedEngine::not_yet(kind)));
            }
            for engine in plugged {
                windows.set_engine(engine);
            }
            HostEngines {
                me: me.clone(),
                host: Arc::downgrade(host),
                runtime: runtime.clone(),
                windows,
                show_tiles: AtomicBool::new(settings.show_tiles),
                embedding: Mutex::new(Embedding {
                    on: settings.embed,
                    scale: settings.scale_to_cell,
                    ..Embedding::default()
                }),
            }
        });
        engines.apply_settings(settings);
        engines.views_changed();
        engines.watch_views(host, runtime);
        engines
    }

    /// The external windows (RB-040-02 and RB-040-04 set their engine here).
    pub fn windows(&self) -> &Arc<ExternalWindows> {
        &self.windows
    }

    /// `engines.external.*` in effect.
    pub fn apply_settings(&self, settings: &ExternalWindowSettings) {
        self.windows.set_placement(placement_of(settings));
        self.windows.set_restart_on_crash(settings.restart_on_crash);
        self.show_tiles
            .store(settings.show_tiles, Ordering::Release);
        {
            let mut embedding = lock(&self.embedding);
            embedding.on = settings.embed;
            embedding.scale = settings.scale_to_cell;
            // What was measured while it was off counts from now on.
            embedding.settled = embedding.cells.clone();
        }
        self.publish();
    }

    /// The external views, by id.
    pub fn list(&self) -> Vec<ExternalView> {
        self.windows.list()
    }

    /// The host's views changed: the external ones are the views on Firefox or WebKit.
    pub(crate) fn views_changed(&self) {
        let Some(host) = self.host.upgrade() else {
            return;
        };
        let config = host.config();
        let start_url = config.start_url.clone().unwrap_or_default();
        let wanted = host
            .views()
            .into_iter()
            .filter(|info| info.engine != EngineKind::Chromium)
            .map(|info| {
                let breakpoint = info
                    .breakpoint
                    .as_deref()
                    .or(info.cell_breakpoint.as_deref())
                    .and_then(|id| config.breakpoints.iter().find(|bp| bp.id == id))
                    .cloned()
                    .unwrap_or_else(|| stand_in(&info));
                let url = if info.url.is_empty() {
                    start_url.clone()
                } else {
                    info.url.clone()
                };
                ViewSpec {
                    view: Some(info.id),
                    emulation: host.emulation(info.id).unwrap_or_default(),
                    url,
                    title: window_title(&info.label, info.engine),
                    window: None,
                    cell: self.cell_for(info.id, &info),
                    breakpoint,
                }
            })
            .collect();
        self.windows.sync(wanted);
    }

    /// Puts the external views into the shell's state (the badge in each cell's label strip,
    /// unless `engines.external.showTiles` is off) and the sync's routes, after asking the
    /// window to embed or release their windows (RB-080-39).
    pub(crate) fn publish(&self) {
        let Some(host) = self.host.upgrade() else {
            return;
        };
        self.sync_embedding(host.ui().as_ref());
        let views = self.windows.list();
        if let Some(sync) = crate::sync_hub::for_host(&host) {
            sync.router().set_routes(views.iter().filter_map(route_of));
        }
        let shown = if self.show_tiles.load(Ordering::Acquire) {
            views
        } else {
            Vec::new()
        };
        host.update_shell_state(|shell| shell.externals = shown);
    }

    fn watch_views(&self, host: &Host, runtime: &Handle) {
        let mut events = host.subscribe();
        let weak = self.me.clone();
        runtime.spawn(async move {
            loop {
                let event = events.recv().await;
                let Some(engines) = weak.upgrade() else {
                    return;
                };
                match event {
                    Ok(ViewEvent::Opened(_) | ViewEvent::Closed(_) | ViewEvent::LayoutChanged)
                    | Err(RecvError::Lagged(_)) => engines.views_changed(),
                    Ok(_) => {}
                    Err(RecvError::Closed) => return,
                }
            }
        });
    }
}

/// Where the sync sends a view's events: to its engine while its window runs, nowhere while
/// it is away. A view whose engine is not available stays with Chromium, which shows it today.
fn route_of(view: &ExternalView) -> Option<(ViewId, ViewRoute)> {
    match view.status {
        ExternalStatus::Unavailable => None,
        ExternalStatus::Running => Some((view.view, ViewRoute::Engine(view.engine))),
        ExternalStatus::Starting
        | ExternalStatus::Exited
        | ExternalStatus::Closed
        | ExternalStatus::Restarting
        | ExternalStatus::Failed => Some((view.view, ViewRoute::Stopped)),
    }
}

/// The title of an external view's window: `rBrowser - <Label> - <Engine>` with middle dots;
/// WebKit says it is not Safari.
pub fn window_title(label: &str, engine: EngineKind) -> String {
    let engine = match engine {
        EngineKind::Webkit => rb_i18n::t("engines.name.webkit"),
        other => engine_name(other).to_owned(),
    };
    rb_i18n::t_with(
        "engines.external.windowTitle",
        &[("label", label), ("engine", &engine)],
    )
}

/// The breakpoint of a view the config no longer names (a free size): its label and size.
fn stand_in(info: &ViewInfo) -> Breakpoint {
    Breakpoint {
        id: info.label.clone(),
        label: Some(info.label.clone()),
        width: Some(info.width),
        height: Some(info.height),
        device: None,
        engine: info.engine,
        session: Some(info.session.clone()),
        sync_navigation: true,
        shortcut: None,
        color: None,
        emulation: EmulationFields::default(),
    }
}

#[cfg(test)]
#[path = "glue_tests.rs"]
mod tests;
