//! [`ExternalWindows`]: the external views on tokio. A view starts when the layout first wants
//! it (its engine starts with its first view), its window goes where the placement says, and a
//! view the layout keeps keeps its window across every sync. Its end shows at once: closed by
//! the person it stays `Closed` until they open it again; crashed it is `Exited` and, with the
//! restart on crash on, comes back with its last URL after the backoff. Every start is logged
//! with its reason (RB-080-38). All windows close with the app.

#[path = "lifecycle.rs"]
mod lifecycle;

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, Weak};

use rb_config::EngineKind;
use rb_util::lock;
use tokio::runtime::Handle;

use self::lifecycle::close;
use super::placement::{Placement, Screen, WindowRect, place};
use super::registry::{Backoff, ExternalSpec, ExternalStatus, ExternalView, Registry};
use crate::ViewId;
use crate::engine::{Engine, EngineHealth, ViewHandle, ViewSpec, engine_name};
use crate::external::scale::CellScale;

/// Called after the external views' state changed (status, URL, window).
pub type OnChange = Arc<dyn Fn() + Send + Sync>;

/// Why a window starts; logged with every start, so a diagnostics ZIP shows it (RB-080-38).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartReason {
    /// The layout has a new external view.
    NewView,
    /// The view's breakpoint moved to another engine.
    EngineChanged,
    /// The window crashed and `restartOnCrash` is on.
    Crashed,
    /// The person (or MCP) asked for it: "Open again", `engine.restart`.
    Requested,
}

impl std::fmt::Display for StartReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::NewView => "new view",
            Self::EngineChanged => "engine changed",
            Self::Crashed => "crashed",
            Self::Requested => "reopened by user",
        })
    }
}

/// Why a restart could not be done.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum RestartError {
    /// The layout has no view of this engine: nothing runs that could restart.
    #[error(
        "no {} view is open; add a breakpoint with this engine to the layout first",
        engine_name(*.0)
    )]
    NotRunning(EngineKind),
    /// The view is not an external view of this engine.
    #[error("view {view} is not a {} view", engine_name(*.engine))]
    UnknownView {
        /// The engine asked for.
        engine: EngineKind,
        /// The view asked for.
        view: ViewId,
    },
    /// The engine cannot run.
    #[error("{reason}")]
    Unavailable {
        /// The engine.
        engine: EngineKind,
        /// Why; names the engine.
        reason: String,
    },
}

/// The reason an engine without an implementation gives.
pub fn not_available_yet(engine: EngineKind) -> String {
    format!("{} is not available yet", engine_name(engine))
}

/// Where new windows go.
#[derive(Clone, Debug, Default)]
struct Placing {
    placement: Placement,
    screen: Screen,
    /// The last rectangle per breakpoint (`remember`).
    remembered: BTreeMap<String, WindowRect>,
}

/// The external views of one window.
pub struct ExternalWindows {
    me: Weak<ExternalWindows>,
    engines: Mutex<HashMap<EngineKind, Arc<dyn Engine>>>,
    registry: Mutex<Registry>,
    specs: Mutex<HashMap<ViewId, ViewSpec>>,
    handles: Mutex<HashMap<ViewId, Arc<dyn ViewHandle>>>,
    /// The views whose start is under way, each with the start asked for meanwhile.
    starting: Mutex<HashMap<ViewId, Option<StartReason>>>,
    placing: Mutex<Placing>,
    runtime: Handle,
    on_change: OnChange,
}

impl ExternalWindows {
    /// No external views yet; crashed views restart with `backoff` while `restart_on_crash`
    /// (`engines.external.restartOnCrash`) is on.
    pub fn new(
        runtime: Handle,
        backoff: Backoff,
        restart_on_crash: bool,
        on_change: OnChange,
    ) -> Arc<Self> {
        Arc::new_cyclic(|me| Self {
            me: me.clone(),
            engines: Mutex::new(HashMap::new()),
            registry: Mutex::new(Registry::new(backoff, restart_on_crash)),
            specs: Mutex::new(HashMap::new()),
            handles: Mutex::new(HashMap::new()),
            starting: Mutex::new(HashMap::new()),
            placing: Mutex::new(Placing::default()),
            runtime,
            on_change,
        })
    }

    /// The engine of its kind from now on (RB-040-02 and RB-040-04 plug in here).
    pub fn set_engine(&self, engine: Arc<dyn Engine>) {
        lock(&self.engines).insert(engine.kind(), engine);
    }

    /// How the engine of `kind` is doing: its own word, or "not available yet" while none of
    /// that kind is plugged in (RB-080-33: the engine badge and its menu read it).
    pub fn engine_health(&self, kind: EngineKind) -> EngineHealth {
        lock(&self.engines).get(&kind).map_or_else(
            || EngineHealth::Unavailable(not_available_yet(kind)),
            |engine| engine.health(),
        )
    }

    /// The restart after a crash on or off.
    pub fn set_restart_on_crash(&self, on: bool) {
        lock(&self.registry).set_restart_on_crash(on);
    }

    /// The placement of new windows and of `arrange`.
    pub fn set_placement(&self, placement: Placement) {
        lock(&self.placing).placement = placement;
    }

    /// The main window and the monitors.
    pub fn set_screen(&self, screen: Screen) {
        lock(&self.placing).screen = screen;
    }

    /// The last window rectangles by breakpoint (`remember`).
    pub fn set_remembered(&self, remembered: BTreeMap<String, WindowRect>) {
        lock(&self.placing).remembered = remembered;
    }

    /// Every external view, by id.
    pub fn list(&self) -> Vec<ExternalView> {
        lock(&self.registry).list()
    }

    /// One external view.
    pub fn view(&self, view: ViewId) -> Option<ExternalView> {
        lock(&self.registry).get(view).cloned()
    }

    /// The handle of a running external view.
    pub fn handle(&self, view: ViewId) -> Option<Arc<dyn ViewHandle>> {
        lock(&self.handles).get(&view).cloned()
    }

    /// `view`'s window sits in its grid cell (`true`) or is a window of its own (RB-080-39);
    /// answers whether that changed. The caller publishes the change: nothing is notified here,
    /// because the app decides this inside the change handler itself.
    pub fn set_embedded(&self, view: ViewId, embedded: bool) -> bool {
        lock(&self.registry).set_embedded(view, embedded)
    }

    /// The scale `view` opens with from now on (`ViewSpec.cell`, RB-080-40): a restart takes
    /// it, a running view is told through its handle.
    pub fn set_spec_cell(&self, view: ViewId, cell: Option<CellScale>) {
        if let Some(spec) = lock(&self.specs).get_mut(&view) {
            spec.cell = cell;
        }
    }

    /// The engine built `view`'s window anew (RB-080-40): it left its cell; the app hears it
    /// through the change and [`ExternalWindows::take_replaced`].
    pub(super) fn window_replaced(&self, view: ViewId) {
        lock(&self.registry).window_replaced(view);
        (self.on_change)();
    }

    /// The views whose window the engine built anew since the last call: each needs binding
    /// to its cell again.
    pub fn take_replaced(&self) -> Vec<ViewId> {
        lock(&self.registry).take_replaced()
    }

    /// Where `view`'s window goes as a window of its own by the placement: the slot `arrange`
    /// gives it (its position among the running windows that are not in their cells), so the
    /// two never disagree; `None` before the screen is known or while it is in its cell.
    pub fn placement_rect(&self, view: ViewId) -> Option<WindowRect> {
        let placing = lock(&self.placing).clone();
        let index = self
            .arranged()
            .iter()
            .position(|(running, _)| *running == view)?;
        self.rect_for(&placing, view, index, false)
    }

    /// The running windows `arrange` places, in cascade order (by view id); one in its cell
    /// (embedded, RB-080-39) is left out.
    fn arranged(&self) -> Vec<(ViewId, Arc<dyn ViewHandle>)> {
        let handles = lock(&self.handles);
        let registry = lock(&self.registry);
        let mut running: Vec<_> = handles
            .iter()
            .filter(|(view, _)| !registry.get(**view).is_some_and(|known| known.embedded))
            .map(|(view, handle)| (*view, handle.clone()))
            .collect();
        running.sort_by_key(|(view, _)| *view);
        running
    }

    /// Makes the external views exactly `wanted` (each with its `view` set): new ones start,
    /// gone ones close, kept ones stay as they are - window, page and status included. A view
    /// whose engine changed closes its old window first, then starts on the new engine.
    pub fn sync(&self, wanted: Vec<ViewSpec>) {
        let wanted: Vec<(ViewId, ViewSpec)> = wanted
            .into_iter()
            .filter_map(|spec| spec.view.map(|view| (view, spec)))
            .collect();
        let externals: Vec<ExternalSpec> = wanted
            .iter()
            .map(|(view, spec)| ExternalSpec {
                view: *view,
                label: spec
                    .breakpoint
                    .label
                    .clone()
                    .unwrap_or_else(|| spec.breakpoint.id.clone()),
                breakpoint: spec.breakpoint.id.clone(),
                engine: spec.breakpoint.engine,
                url: spec.url.clone(),
            })
            .collect();
        let (added, gone) = lock(&self.registry).sync(&externals);
        {
            let mut specs = lock(&self.specs);
            specs.retain(|view, _| !gone.contains(view));
            for (view, spec) in wanted {
                specs.insert(view, spec);
            }
        }
        for view in &gone {
            let handle = lock(&self.handles).remove(view);
            if added.contains(view) {
                self.spawn_close_then_start(*view, handle, StartReason::EngineChanged);
            } else if let Some(handle) = handle {
                self.runtime.spawn(close(handle));
            }
        }
        for view in added {
            if !gone.contains(&view) {
                self.spawn_start(view, StartReason::NewView);
            }
        }
        (self.on_change)();
    }

    /// Restarts the external views of `engine` - all, or only `view` - with their last URLs,
    /// a window the person closed included ("Open again", `engine.restart`). Answers the
    /// views restarted.
    pub fn restart(
        &self,
        engine: EngineKind,
        view: Option<ViewId>,
    ) -> Result<Vec<ViewId>, RestartError> {
        let views = {
            let registry = lock(&self.registry);
            match view {
                Some(view)
                    if registry
                        .get(view)
                        .is_some_and(|known| known.engine == engine) =>
                {
                    vec![view]
                }
                Some(view) => return Err(RestartError::UnknownView { engine, view }),
                None => registry.of_engine(engine),
            }
        };
        self.restart_views(engine, views, StartReason::Requested)
    }

    /// `engine` was rebuilt (a changed path, profile or setting): its views start again on the
    /// new one - except a window the person closed, which stays closed (RB-080-38).
    pub fn restart_after_rebuild(&self, engine: EngineKind) -> Result<Vec<ViewId>, RestartError> {
        let views = {
            let registry = lock(&self.registry);
            registry
                .of_engine(engine)
                .into_iter()
                .filter(|view| {
                    registry
                        .get(*view)
                        .is_some_and(|known| known.status != ExternalStatus::Closed)
                })
                .collect()
        };
        self.restart_views(engine, views, StartReason::EngineChanged)
    }

    fn restart_views(
        &self,
        engine: EngineKind,
        views: Vec<ViewId>,
        reason: StartReason,
    ) -> Result<Vec<ViewId>, RestartError> {
        if views.is_empty() {
            return Err(RestartError::NotRunning(engine));
        }
        match lock(&self.engines)
            .get(&engine)
            .map(|engine| engine.health())
        {
            None => {
                return Err(RestartError::Unavailable {
                    engine,
                    reason: not_available_yet(engine),
                });
            }
            Some(EngineHealth::Unavailable(reason)) => {
                return Err(RestartError::Unavailable { engine, reason });
            }
            Some(_) => {}
        }
        for view in &views {
            // Known views only, checked above.
            let _known = lock(&self.registry).restart_requested(*view);
            let handle = lock(&self.handles).remove(view);
            self.spawn_close_then_start(*view, handle, reason);
        }
        (self.on_change)();
        Ok(views)
    }

    /// Brings every external window to the front.
    pub fn raise_all(&self) -> usize {
        let handles: Vec<Arc<dyn ViewHandle>> = lock(&self.handles).values().cloned().collect();
        let count = handles.len();
        for handle in handles {
            self.runtime.spawn(async move {
                if let Err(error) = handle.raise().await {
                    tracing::debug!(view = %handle.id(), %error, "raise failed");
                }
            });
        }
        count
    }

    /// Places every running external window again by the placement, cascaded in view order;
    /// answers the views moved. A window embedded in its cell stays there (RB-080-39).
    pub fn arrange(&self) -> Vec<ViewId> {
        let running = self.arranged();
        let placing = lock(&self.placing).clone();
        let mut moved = Vec::new();
        for (index, (view, handle)) in running.into_iter().enumerate() {
            let Some(rect) = self.rect_for(&placing, view, index, false) else {
                continue;
            };
            lock(&self.registry).moved(view, rect);
            moved.push(view);
            self.runtime.spawn(async move {
                if let Err(error) = handle.set_window(rect).await {
                    tracing::debug!(%view, %error, "arranging failed");
                }
            });
        }
        (self.on_change)();
        moved
    }

    /// Closes every external view and stops every engine (the app quits).
    pub async fn shutdown(&self) {
        let handles: Vec<Arc<dyn ViewHandle>> = lock(&self.handles)
            .drain()
            .map(|(_, handle)| handle)
            .collect();
        for handle in handles {
            close(handle).await;
        }
        let engines: Vec<Arc<dyn Engine>> = lock(&self.engines).values().cloned().collect();
        for engine in engines {
            engine.shutdown().await;
        }
    }

    /// The window of view number `index`, or `None` when the screen is not known yet.
    fn rect_for(
        &self,
        placing: &Placing,
        view: ViewId,
        index: usize,
        new: bool,
    ) -> Option<WindowRect> {
        if placing.screen.main.width == 0 || placing.screen.main.height == 0 {
            return None;
        }
        let spec = lock(&self.specs).get(&view).cloned()?;
        let remembered = if new {
            placing.remembered.get(&spec.breakpoint.id).copied()
        } else {
            None
        };
        let size = (spec.emulation.width, spec.emulation.height);
        Some(place(
            placing.placement,
            &placing.screen,
            size,
            index,
            remembered,
        ))
    }

    fn changed(&self) {
        (self.on_change)();
    }
}

#[cfg(test)]
#[path = "manager_tests.rs"]
mod tests;
