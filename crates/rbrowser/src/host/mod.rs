//! The view manager: the app's [`ViewHost`] (contract 0.1). It owns the views' state, builds
//! the grid through the window ([`UiBackend`]), and owns their emulation over CDP: it captures
//! each view's target at creation, attaches a flat session, sends the device metrics that fit
//! the breakpoint into the cell, and sends them again after every resize and every reconnect
//! (ADR 0001, Decision 3).
//!
//! Every method returns quickly: window work is posted to the CEF UI thread by the backend,
//! CDP work runs as tasks on the tokio runtime.

mod apply_queue;
mod attach;
pub mod focus_cycle;
mod focused;
pub mod footer;
pub mod nav;
pub mod path_pick;
mod placed;
mod pop_out;
mod rebuild;
mod sessions;
pub mod shell_state;
pub mod state;
mod text_scale;
mod throttle;
pub mod ui;
mod view_host;

use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, Weak};

use rb_config::{Layout, ProjectConfig};
use rb_engine::{CellRect, EngineError, Target, ViewEvent, ViewHost, ViewId};
use tokio::runtime::Handle;
use tokio::sync::broadcast;

pub use focused::{FocusRequest, keeps_focus};
pub use pop_out::PopOutOptions;
pub use text_scale::label_height;

use crate::cdp::{CdpRouter, lock};
use crate::fit::zoom_step;
use state::{HostState, ViewState, layout_for};
use ui::{UiBackend, UiGrid};

/// The height of the label strip above each view, in DIP at 100 % text size ([`label_height`]).
pub const LABEL_HEIGHT: u32 = 20;
/// Events kept for a subscriber that falls behind.
const EVENT_BUFFER: usize = 256;

/// The view manager.
pub struct Host {
    me: Weak<Host>,
    ui: Arc<dyn UiBackend>,
    cdp: Arc<CdpRouter>,
    runtime: Handle,
    state: Mutex<HostState>,
    events: broadcast::Sender<ViewEvent>,
    /// Serialises sending metrics, so a later size never loses against an earlier one; the
    /// views share it for the permission grants of their browser context.
    apply: tokio::sync::Mutex<()>,
    /// One latest-wins metrics worker per view ([`apply_queue`]).
    applying: apply_queue::ApplyQueue,
    root: Mutex<Option<PathBuf>>,
    /// What the shell shows beside the views (`shell_state`, contract 0.1-w1b).
    shell: Mutex<shell_state::ShellState>,
    /// The sync agents of the views (RB-020-01).
    agents: Arc<crate::sync_agent::SyncAgents>,
    /// The page API of the views (RB-020-08).
    page: Arc<crate::sync_agent::PageApi>,
    /// The Framework DevTools scripts of the views (RB-080-24).
    frameworks: Arc<crate::sync_agent::FrameworkHooks>,
    /// What the views are throttled to, and by which settings (RB-020-09).
    throttle: Mutex<crate::perf::Throttle>,
    throttle_settings: Mutex<crate::perf::ThrottleSettings>,
}

impl Host {
    /// A host without views, talking to `ui` and `cdp`, running its tasks on `runtime`.
    pub fn new(ui: Arc<dyn UiBackend>, cdp: Arc<CdpRouter>, runtime: Handle) -> Arc<Self> {
        let (events, _) = broadcast::channel(EVENT_BUFFER);
        let agents = crate::sync_agent::SyncAgents::new(cdp.clone(), runtime.clone());
        let page = crate::sync_agent::PageApi::new(cdp.clone(), agents.clone());
        let frameworks = crate::sync_agent::FrameworkHooks::new(cdp.clone());
        let host = Arc::new_cyclic(|me| Self {
            me: me.clone(),
            ui,
            cdp,
            runtime,
            state: Mutex::new(HostState::default()),
            events,
            apply: tokio::sync::Mutex::new(()),
            applying: apply_queue::ApplyQueue::default(),
            root: Mutex::new(None),
            shell: Mutex::new(shell_state::ShellState::default()),
            agents,
            page,
            frameworks,
            throttle: Mutex::new(crate::perf::Throttle::default()),
            throttle_settings: Mutex::new(crate::perf::ThrottleSettings::default()),
        });
        attach::watch_cdp(&host);
        footer::watch(&host);
        crate::sync_agent::page_tasks::start(&host, &host.runtime);
        host
    }

    /// Opens `config` (of the project at `root`, if any): its breakpoints, start URL and grid.
    /// Views whose cell keeps its breakpoint stay as they are (config reload).
    pub fn open_config(
        &self,
        config: ProjectConfig,
        root: Option<PathBuf>,
    ) -> Result<(), EngineError> {
        let layout = layout_for(&config);
        *lock(&self.root) = root;
        self.state().config = config;
        self.rebuild(&layout).map(|_| ())
    }

    /// The project root, when a project is open.
    pub fn project_root(&self) -> Option<PathBuf> {
        lock(&self.root).clone()
    }

    /// The sync agents of the views (RB-020-01): their reports and commands (RB-020-02).
    pub fn sync_agents(&self) -> &Arc<crate::sync_agent::SyncAgents> {
        &self.agents
    }

    /// The page API of the views (RB-020-08): its switches, states and bus.
    pub fn page_api(&self) -> &Arc<crate::sync_agent::PageApi> {
        &self.page
    }

    /// The Framework DevTools scripts of the views (RB-080-24): the probe and the hooks.
    pub fn framework_hooks(&self) -> &Arc<crate::sync_agent::FrameworkHooks> {
        &self.frameworks
    }

    /// The project config in effect.
    pub fn config(&self) -> ProjectConfig {
        self.state().config.clone()
    }

    /// Closes the view of an ad-hoc breakpoint (`open_view`, one not in the config): its cell
    /// leaves the grid in effect (RB-080-41). A view of a config breakpoint is refused: the
    /// selection (`view.remove`) hides those.
    pub fn close_extra_view(&self, view: ViewId) -> Result<(), EngineError> {
        let layout = {
            let state = self.state();
            let entry = state
                .views
                .get(&view)
                .ok_or_else(|| EngineError::Invalid(format!("unknown view {view}")))?;
            let breakpoint = entry.cell_breakpoint.clone().ok_or_else(|| {
                EngineError::Invalid(format!("view {view} has no cell of its own"))
            })?;
            if state
                .config
                .breakpoints
                .iter()
                .any(|known| known.id == breakpoint)
            {
                return Err(EngineError::Invalid(format!(
                    "`{breakpoint}` is a breakpoint of the config: view.remove hides it"
                )));
            }
            state::layout_without_extra_column(&state.layout, &breakpoint)
        };
        self.rebuild(&layout).map(|_| ())
    }

    /// Puts `config` in effect without touching the grid - for a change the grid does not show
    /// (the bookmarks, RB-080-16); the shell gets the new state.
    pub fn replace_config(&self, config: ProjectConfig) {
        self.state().config = config;
        self.footer_config_changed();
        self.emit(ViewEvent::StateChanged);
    }

    /// The window laid the grid area out at `area`: the cells are computed for it.
    pub fn on_grid_area(&self, area: CellRect) {
        let mut state = self.state();
        if state.grid_area == Some(area) {
            return;
        }
        state.grid_area = Some(area);
        state.cells = footer::grid_cells(&state, &state.visible_layout(), area);
        // A layout mode places its views itself (RB-080-04): they follow the area.
        let placed = placed::place(&mut state);
        drop(state);
        if let Some(placed::Placement {
            area,
            placed,
            moved,
        }) = placed
        {
            self.ui.place_views(area, placed);
            self.rethrottle();
            for view in moved {
                self.spawn_apply(view, false);
            }
        }
        self.emit(ViewEvent::LayoutChanged);
    }

    /// The window laid `view` out at `rect` (DIP, client area): its metrics follow, and a view
    /// without area is throttled (`performance.throttleHidden`, RB-020-09).
    pub fn on_view_bounds(&self, view: ViewId, rect: CellRect) {
        let mut state = self.state();
        let Some(entry) = state.views.get_mut(&view) else {
            return;
        };
        if entry.measured == Some(rect) {
            return;
        }
        // A popped-out window of another width: its footer may need another line
        // (RB-080-53). A view in the grid is fitted to its cell.
        let refit = entry.detached.is_some()
            && entry.measured.map(|measured| measured.width) != Some(rect.width);
        entry.measured = Some(rect);
        drop(state);
        self.rethrottle();
        self.spawn_apply(view, false);
        if refit {
            self.refit_footers();
        }
    }

    /// A load ended in `view`'s main frame. The marker load attaches the CDP session; the
    /// first other one is the start-up time (RB-090-32).
    pub fn on_load_end(&self, view: ViewId, url: &str) {
        if nav::marker_view(url) == Some(view) {
            attach::spawn_attach(self, view);
            return;
        }
        crate::perf::counters::first_view_loaded();
        self.update(view, |entry| {
            let ended = set_loading(entry, false);
            ended || url != entry.url
        });
    }

    /// `view`'s main frame committed `url`.
    pub fn on_address_change(&self, view: ViewId, url: &str) {
        if nav::marker_view(url).is_some() {
            return;
        }
        let changed = self.update(view, |entry| {
            let changed = entry.url != url;
            entry.url = url.to_owned();
            changed
        });
        if changed {
            // A view with a position grants it to its new origin (RB-080-10).
            let position = self.state().views.get(&view).is_some_and(|entry| {
                entry.emulation.geolocation.is_some() || entry.geo_origin.is_some()
            });
            if position {
                self.spawn_apply(view, false);
            }
            self.emit(ViewEvent::Navigated {
                view,
                url: url.to_owned(),
            });
        }
    }

    /// `view`'s title changed.
    pub fn on_title_change(&self, view: ViewId, title: &str) {
        self.update(view, |entry| {
            let changed = entry.title != title;
            entry.title = title.to_owned();
            changed
        });
    }

    /// `view` started or stopped loading.
    pub fn on_loading_state(&self, view: ViewId, loading: bool) {
        self.update(view, |entry| set_loading(entry, loading));
    }

    /// How many loads of `view` have ended; `None` for an unknown view.
    pub fn loads_ended(&self, view: ViewId) -> Option<u64> {
        self.state().views.get(&view).map(|entry| entry.loads_ended)
    }

    /// `view.zoom_fit_all`: every view back to its cell's zoom.
    pub fn zoom_fit_all(&self) {
        let views: Vec<ViewId> = {
            let mut state = self.state();
            state
                .views
                .values_mut()
                .filter_map(|entry| entry.zoom_override.take().map(|_| entry.id))
                .collect()
        };
        for view in views {
            self.spawn_apply(view, false);
        }
    }

    /// `view.zoom_in` / `view.zoom_out`: the active view one zoom step up or down.
    pub fn zoom_active(&self, up: bool) -> Result<(), EngineError> {
        let view = self.active_or_first()?;
        {
            let mut state = self.state();
            let current = state
                .views
                .get(&view)
                .map(|entry| state.metrics_of(entry).scale)
                .ok_or(EngineError::UnknownView(view))?;
            if let Some(entry) = state.views.get_mut(&view) {
                entry.zoom_override = Some(zoom_step(current, up));
            }
        }
        self.spawn_apply(view, false);
        Ok(())
    }

    /// `view.focus_next` / `view.focus_prev`: the next or previous view in grid order becomes
    /// active and gets the focus.
    pub fn focus_step(&self, forward: bool) -> Result<ViewId, EngineError> {
        let next = {
            let state = self.state();
            let count = state.order.len();
            if count == 0 {
                return Err(EngineError::Invalid("there is no view".to_owned()));
            }
            let position = state
                .active
                .and_then(|active| state.order.iter().position(|id| *id == active));
            let index = match (position, forward) {
                (None, true) => 0,
                (None, false) => count - 1,
                (Some(at), true) => (at + 1) % count,
                (Some(at), false) => (at + count - 1) % count,
            };
            state.order[index]
        };
        self.set_active_view(next)?;
        Ok(next)
    }

    /// The active view, else the first in grid order (`target_views(None)`, a plugin's
    /// `views.active`, RB-090-82); [`EngineError::Invalid`] without a view.
    pub fn active_or_first(&self) -> Result<ViewId, EngineError> {
        let state = self.state();
        state
            .active
            .or_else(|| state.order.first().copied())
            .ok_or_else(|| EngineError::Invalid("there is no view".to_owned()))
    }

    /// Rebuilds the grid for `layout` and returns the view of each cell.
    fn rebuild(&self, layout: &Layout) -> Result<Vec<ViewId>, EngineError> {
        let mut state = self.state();
        let mut next = state.clone();
        // A grid leaves a layout mode (RB-080-04).
        placed::leave_mode(&mut next);
        let rebuilt = rebuild::rebuild_state(&mut next, layout)?;
        *state = next;
        drop(state);
        Ok(self.finish_rebuild(layout, rebuilt))
    }

    /// The effects of a committed rebuild: the grid, metrics and events.
    fn finish_rebuild(&self, layout: &Layout, rebuilt: rebuild::Rebuilt) -> Vec<ViewId> {
        let rebuild::Rebuilt {
            order,
            plan,
            mut cells,
            kept_views,
            closed,
            opened,
        } = rebuilt;
        for id in &closed {
            self.agents.forget(*id);
        }
        // In a layout mode the views are placed, not laid out by the plan (RB-080-04).
        let footer_height = self.with_footers(&mut cells);
        let (area, placed) = self.placed_views().unzip();
        self.ui.apply_grid(UiGrid {
            plan,
            gap: layout.gap,
            label_height: label_height(),
            footer_height,
            cells,
            placed,
            area,
            // Views in a window of their own stay there (RB-080-14).
            detached: self.detached_cells(),
        });
        // A kept view whose zoom or emulation changed gets new metrics; unchanged ones send none.
        for id in kept_views {
            self.spawn_apply(id, false);
        }
        for id in closed {
            self.emit(ViewEvent::Closed(id));
        }
        for id in opened {
            self.emit(ViewEvent::Opened(id));
        }
        self.emit(ViewEvent::LayoutChanged);
        self.rethrottle();
        order
    }

    /// Sends `view`'s metrics (all of it with `force`). A layout mode places its views again
    /// first when the view's size changed (RB-080-54, [`Self::place_if_resized`]); the views
    /// that moved with it get theirs once too, this one only here.
    fn spawn_apply(&self, view: ViewId, force: bool) {
        for moved in self.place_if_resized(view) {
            if moved != view {
                self.spawn_metrics(moved, false);
            }
        }
        self.spawn_metrics(view, force);
    }

    /// The text of the label strip of `view` ([`cell_label`]), which the canvas board draws
    /// itself (RB-080-22); `None` for a view that does not exist.
    pub fn cell_label_text(&self, view: ViewId) -> Option<String> {
        self.state().views.get(&view).map(cell_label)
    }

    /// Changes `view` with `change`; emits `Updated` when it says something changed.
    fn update(&self, view: ViewId, change: impl FnOnce(&mut ViewState) -> bool) -> bool {
        let changed = self.state().views.get_mut(&view).is_some_and(change);
        if changed {
            self.emit(ViewEvent::Updated(view));
        }
        changed
    }

    fn targets(&self, target: Target) -> Result<Vec<ViewId>, EngineError> {
        let state = self.state();
        match target {
            Target::All => Ok(state.order.clone()),
            Target::View(view) if state.views.contains_key(&view) => Ok(vec![view]),
            Target::View(view) => Err(EngineError::UnknownView(view)),
        }
    }

    fn state(&self) -> MutexGuard<'_, HostState> {
        lock(&self.state)
    }

    fn emit(&self, event: ViewEvent) {
        // No subscriber is not an error.
        let _ = self.events.send(event);
    }
}

/// Sets `entry`'s loading state and counts an ended load; `true` when it changed.
fn set_loading(entry: &mut ViewState, loading: bool) -> bool {
    if entry.loading == loading {
        return false;
    }
    if !loading {
        entry.loads_ended += 1;
    }
    entry.loading = loading;
    true
}

/// The text of a cell's label strip: the breakpoint, its size and - when it is set and not 1 -
/// its pixel ratio, as in `Mobile  393 x 852 @3x` with a multiplication sign (RB-010-07).
fn cell_label(entry: &ViewState) -> String {
    let name = entry
        .breakpoint
        .as_ref()
        .map(|breakpoint| {
            breakpoint
                .label
                .clone()
                .unwrap_or_else(|| breakpoint.id.clone())
        })
        .unwrap_or_default();
    let emulation = &entry.emulation;
    // 0 leaves the pixel ratio to the device: no suffix, as for 1.
    let ratio = if emulation.dpr > 0.0 && (emulation.dpr - 1.0).abs() > f64::EPSILON {
        format!(" @{}x", emulation.dpr)
    } else {
        String::new()
    };
    // U+00D7, the multiplication sign.
    format!(
        "{name}  {} \u{d7} {}{ratio}",
        emulation.width, emulation.height
    )
}

#[cfg(test)]
#[path = "host_tests.rs"]
mod tests;
