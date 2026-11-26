//! The DevTools of one window: which view they inspect and where (docked below, docked right,
//! a window of its own), the dock's preparation, following the active view, and the shell's
//! [`crate::host::shell_state::ShellState::devtools`]. One DevTools at a time.
//!
//! Docking happens in two steps (`session_dock.rs`). The dock is created at [`DOCK_MARKER_URL`];
//! once that loaded, its target is captured by the marker and hidden from outer CDP clients, the
//! screencast's setting is written for the hosted frontend (`devtools.screencast`,
//! [`super::screencast`]), and only then is the frontend URL with its ticket loaded. Later
//! retargets load the new URL into the prepared dock; a changed screencast setting makes a new
//! dock, since the frontend reads its settings once at start. Every dock has a generation
//! (`Inner::dock_generation`, raised when a dock is made and when it goes), so a preparation
//! that finishes for a dock that went meanwhile is dropped, not kept for the next dock.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, Weak};

use rb_config::DevtoolsMode;
use rb_engine::{EngineError, ViewEvent, ViewHost, ViewId};
use rb_ipc::DevtoolsState;
use serde_json::json;
use tokio::runtime::Handle;
use tokio::sync::broadcast::error::RecvError;

use super::endpoint::{Endpoint, Ticket};
use super::hidden::DOCK_MARKER_URL;
use super::layout::side_of;
use super::ui_state::{self, UiState, WindowBounds};
use crate::cdp::lock;
use crate::control::Core;
use crate::host::Host;
use crate::host::ui::DevtoolsDock;

/// How far the dock is.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
enum Dock {
    /// No dock.
    #[default]
    Closed,
    /// Created at the marker; preparing.
    Marker,
    /// Prepared; its own session on the pipe, when it got one.
    Ready(Option<String>),
}

#[derive(Debug)]
struct Open {
    view: ViewId,
    mode: DevtoolsMode,
    /// The docked frontend's connection; `None` in a window.
    ticket: Option<Ticket>,
}

#[derive(Default)]
struct Inner {
    endpoint: Option<Arc<Endpoint>>,
    open: Option<Open>,
    dock: Dock,
    /// Raised when a dock is made at its marker and when a dock goes: a preparation carries the
    /// generation it started for, and its result is dropped once the generation moved on.
    dock_generation: u64,
    /// Whether the dock was (or is being) prepared with the frontend's screencast on.
    screencast: bool,
    ui: UiState,
}

/// The DevTools of one window.
pub struct Devtools {
    me: Weak<Devtools>,
    host: Weak<Host>,
    core: Weak<Core>,
    runtime: Handle,
    /// Writes the UI state to its file; `None`: nothing is kept.
    saver: Option<Arc<ui_state::Saver>>,
    inner: Mutex<Inner>,
    /// The dock's size when the splitter's drag began (`super::splitter`).
    drag_start: Mutex<Option<u32>>,
}

impl Devtools {
    /// The DevTools of `core`'s window; the UI state is read from `state_path` (none: nothing
    /// is kept).
    pub fn new(core: &Core, runtime: Handle, state_path: Option<PathBuf>) -> Arc<Self> {
        let ui = state_path
            .as_deref()
            .map(ui_state::load)
            .unwrap_or_default();
        Arc::new_cyclic(|me| Self {
            me: me.clone(),
            host: Arc::downgrade(core.host()),
            core: core.arc().as_ref().map(Arc::downgrade).unwrap_or_default(),
            runtime,
            saver: state_path.map(ui_state::Saver::new),
            inner: Mutex::new(Inner {
                ui,
                ..Inner::default()
            }),
            drag_start: Mutex::new(None),
        })
    }

    fn host(&self) -> Result<Arc<Host>, EngineError> {
        self.host.upgrade().ok_or(EngineError::Closed)
    }

    /// The mode the settings give.
    pub fn configured_mode(&self) -> DevtoolsMode {
        self.core
            .upgrade()
            .map(|core| core.user_settings().devtools.mode)
            .unwrap_or_default()
    }

    fn follows(&self) -> bool {
        self.core
            .upgrade()
            .is_some_and(|core| core.user_settings().devtools.follow_active_view)
    }

    /// The inspected view and the mode, while DevTools are open.
    pub fn current(&self) -> Option<DevtoolsState> {
        lock(&self.inner).open.as_ref().map(|open| DevtoolsState {
            view: open.view,
            mode: open.mode,
        })
    }

    /// Opens DevTools for `view` in `mode`; open ones move there (one DevTools at a time).
    pub fn open(&self, view: ViewId, mode: DevtoolsMode) -> Result<(), EngineError> {
        let host = self.host()?;
        let sizes = self.dock_sizes();
        let screencast = self.screencast_wanted();
        let mut inner = lock(&self.inner);
        if let Some(open) = &inner.open
            && open.view == view
            && open.mode == mode
        {
            return Ok(());
        }
        let Some(side) = side_of(mode) else {
            if let Some(previous) = inner.open.take() {
                self.dismiss(&host, &mut inner, previous);
            }
            host.ui().show_devtools(view);
            inner.open = Some(Open {
                view,
                mode,
                ticket: None,
            });
            drop(inner);
            self.publish(&host);
            return Ok(());
        };
        let target = host
            .target_id(view)
            .ok_or_else(|| EngineError::Invalid(format!("view {view} has no CDP target yet")))?;
        let endpoint = self.endpoint(&host, &mut inner)?;
        let ticket = endpoint.grant(view, &target).ok_or_else(|| {
            EngineError::Invalid("no randomness for the DevTools token".to_owned())
        })?;
        if let Some(previous) = inner.open.take()
            && previous.mode == DevtoolsMode::Window
        {
            host.ui().close_devtools(previous.view);
        }
        let dock = DevtoolsDock {
            view,
            side,
            size: sizes.size_for(side),
            frontend_url: ticket.frontend_url().to_owned(),
        };
        // The previous connection ends with its ticket here; its view gets its emulation back.
        inner.open = Some(Open {
            view,
            mode,
            ticket: Some(ticket),
        });
        match inner.dock {
            Dock::Closed => {
                inner.dock = Dock::Marker;
                inner.dock_generation += 1;
                inner.screencast = screencast;
                host.ui().dock_devtools(Some(DevtoolsDock {
                    frontend_url: DOCK_MARKER_URL.to_owned(),
                    ..dock
                }));
            }
            // The prepared dock loads whatever is open then.
            Dock::Marker => host.ui().dock_devtools(Some(DevtoolsDock {
                frontend_url: DOCK_MARKER_URL.to_owned(),
                ..dock
            })),
            Dock::Ready(_) => host.ui().dock_devtools(Some(dock)),
        }
        drop(inner);
        self.publish(&host);
        Ok(())
    }

    /// Closes DevTools, wherever they are.
    pub fn close(&self) -> Result<(), EngineError> {
        let host = self.host()?;
        let mut inner = lock(&self.inner);
        if let Some(open) = inner.open.take() {
            self.dismiss(&host, &mut inner, open);
        }
        drop(inner);
        self.publish(&host);
        Ok(())
    }

    /// `devtools.toggle`: closes open DevTools, or opens them for the active view.
    pub fn toggle(&self) -> Result<Option<DevtoolsState>, EngineError> {
        if self.current().is_some() {
            self.close()?;
            return Ok(None);
        }
        let host = self.host()?;
        let view = host
            .active_view()
            .or_else(|| host.views().first().map(|info| info.id))
            .ok_or_else(|| EngineError::Invalid("there is no view".to_owned()))?;
        self.open(view, self.configured_mode())?;
        Ok(self.current())
    }

    /// The settings changed: open DevTools move to the mode they give now, a dock takes its
    /// size from them, and a changed `devtools.screencast` makes a new dock (the frontend reads
    /// its settings once, at its start).
    pub fn on_settings_changed(&self) -> Result<(), EngineError> {
        let mode = self.configured_mode();
        let Some(open) = self.current() else {
            return Ok(());
        };
        if open.mode != mode {
            return self.open(open.view, mode);
        }
        let docked = side_of(mode).is_some();
        if docked && lock(&self.inner).screencast != self.screencast_wanted() {
            self.close()?;
            return self.open(open.view, mode);
        }
        self.redock(None);
        Ok(())
    }

    /// The active view changed: docked DevTools follow it (`devtools.followActiveView`).
    pub fn on_active_changed(&self, view: ViewId) {
        let Some(open) = self.current() else {
            return;
        };
        if open.view == view || side_of(open.mode).is_none() || !self.follows() {
            return;
        }
        if let Err(error) = self.open(view, open.mode) {
            tracing::warn!(%view, %error, "DevTools did not follow the active view");
        }
    }

    /// A view was closed: DevTools inspecting it close too.
    pub fn on_view_closed(&self, view: ViewId) {
        if self.current().is_some_and(|open| open.view == view)
            && let Err(error) = self.close()
        {
            tracing::warn!(%view, %error, "DevTools of a closed view did not close");
        }
    }

    /// The DevTools window of `view` closed at `bounds`: they are kept, the view gets its
    /// emulation back, and the shell hears DevTools are closed.
    pub fn on_window_closed(&self, view: ViewId, bounds: Option<WindowBounds>) {
        let Ok(host) = self.host() else {
            return;
        };
        let state = {
            let mut inner = lock(&self.inner);
            if let Some(bounds) = bounds.filter(WindowBounds::usable) {
                inner.ui.window = Some(bounds);
            }
            if inner
                .open
                .as_ref()
                .is_some_and(|open| open.view == view && open.mode == DevtoolsMode::Window)
            {
                inner.open = None;
            }
            inner.ui
        };
        self.save(&state);
        host.reapply_emulation(view);
        self.publish(&host);
    }

    /// Where the DevTools window opens: its last usable bounds.
    pub fn window_bounds(&self) -> Option<WindowBounds> {
        lock(&self.inner).ui.window_bounds()
    }

    /// Asks the docked frontend of `view` to reveal and select `backend_node_id` (RB-080-43);
    /// `false` when DevTools are not open for `view` through the endpoint - in a window, CEF's
    /// own frontend takes no such request.
    pub fn request_inspect(&self, view: ViewId, backend_node_id: i64) -> bool {
        let inner = lock(&self.inner);
        let docked = inner
            .open
            .as_ref()
            .is_some_and(|open| open.view == view && open.ticket.is_some());
        match (&inner.endpoint, docked) {
            (Some(endpoint), true) => {
                endpoint.inspect().request(view, backend_node_id);
                true
            }
            _ => false,
        }
    }

    /// The node waiting to be revealed in `view`'s frontend, if any (tests).
    pub fn pending_inspect(&self, view: ViewId) -> Option<i64> {
        lock(&self.inner)
            .endpoint
            .as_ref()
            .and_then(|endpoint| endpoint.inspect().pending(view))
    }

    /// Ends `open`'s presentation: the window closes, the dock goes (and its ticket with it).
    fn dismiss(&self, host: &Arc<Host>, inner: &mut Inner, open: Open) {
        if let Some(endpoint) = &inner.endpoint {
            endpoint.inspect().clear(open.view);
        }
        match side_of(open.mode) {
            None => host.ui().close_devtools(open.view),
            Some(_) => {
                host.ui().dock_devtools(None);
                if let Dock::Ready(session) = std::mem::take(&mut inner.dock) {
                    self.detach(host, session);
                }
                inner.dock = Dock::Closed;
                inner.dock_generation += 1;
            }
        }
    }

    fn detach(&self, host: &Arc<Host>, session: Option<String>) {
        let Some(session) = session else {
            return;
        };
        let cdp = host.cdp().clone();
        self.runtime.spawn(async move {
            let _ = cdp
                .call(
                    "Target.detachFromTarget",
                    json!({ "sessionId": session }),
                    None,
                )
                .await;
        });
    }

    /// The endpoint, started on first use.
    fn endpoint(&self, host: &Arc<Host>, inner: &mut Inner) -> Result<Arc<Endpoint>, EngineError> {
        if let Some(endpoint) = &inner.endpoint {
            return Ok(endpoint.clone());
        }
        let weak = Arc::downgrade(host);
        let endpoint = Endpoint::start(
            &self.runtime,
            host.cdp().clone(),
            Arc::new(move |view| {
                if let Some(host) = weak.upgrade() {
                    host.reapply_emulation(view);
                }
            }),
        )
        .map_err(|error| EngineError::Invalid(format!("the DevTools endpoint: {error}")))?;
        inner.endpoint = Some(endpoint.clone());
        Ok(endpoint)
    }

    /// Tells the shell what is open.
    fn publish(&self, host: &Host) {
        let current = self.current();
        host.update_shell_state(|state| state.devtools = current);
    }

    /// Saves `state` off the calling thread, the newest winning ([`ui_state::Saver`],
    /// RB-099-19); without a file nothing.
    fn save(&self, state: &UiState) {
        if let Some(saver) = &self.saver {
            saver.save(&self.runtime, *state);
        }
    }

    /// Brings open DevTools in line with the host's views after the follower missed events:
    /// on a view that is gone they close, docked ones follow the active view.
    fn reconcile(&self, host: &Host) {
        let Some(open) = self.current() else {
            return;
        };
        if !host.views().iter().any(|info| info.id == open.view) {
            self.on_view_closed(open.view);
        } else if let Some(active) = host.active_view() {
            self.on_active_changed(active);
        }
    }

    /// Follows the host's views: the active view, closed views.
    pub fn follow(&self, host: &Host) {
        let mut events = host.subscribe();
        let me = self.me.clone();
        self.runtime.spawn(async move {
            loop {
                let event = events.recv().await;
                let Some(devtools) = me.upgrade() else {
                    return;
                };
                match event {
                    Ok(ViewEvent::ActiveChanged(Some(view))) => devtools.on_active_changed(view),
                    Ok(ViewEvent::Closed(view)) => devtools.on_view_closed(view),
                    Ok(_) => {}
                    // Events were dropped, a close or a change of the active view among them,
                    // maybe: the host's views say what they would have (RB-099-19).
                    Err(RecvError::Lagged(_)) => {
                        if let Ok(host) = devtools.host() {
                            devtools.reconcile(&host);
                        }
                    }
                    Err(RecvError::Closed) => return,
                }
            }
        });
    }
}

// The dock's preparation at its marker and its size (the settings', the splitter's while
// dragged).
#[path = "session_dock.rs"]
mod dock;

#[cfg(test)]
#[path = "session_tests.rs"]
pub(crate) mod tests;

#[cfg(test)]
#[path = "session_follow_tests.rs"]
mod follow_tests;
