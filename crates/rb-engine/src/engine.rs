//! The engine contract (RB-040-01): [`Engine`] starts and stops one browser engine and makes its
//! views, [`ViewHandle`] is one view of it. The SyncHub, emulation, screenshots and MCP talk to a
//! view through its handle and never know the engine.
//!
//! | Engine | Implementation | Where its views live |
//! | --- | --- | --- |
//! | Chromium | [`crate::chromium::ChromiumEngine`] over the CEF app's [`crate::ViewHost`] | the grid |
//! | Firefox | RB-040-02 (`rb-bidi`, WebDriver BiDi) | an external window, shown in its cell (RB-080-39) |
//! | WebKit | RB-040-04 (Playwright through the Node sidecar) | an external window, shown in its cell (RB-080-39) |
//! | none yet | [`crate::unsupported::UnsupportedEngine`]: "not available yet" | - |
//!
//! Input is replayed in CSS px ([`InputEvent`]); the engine scales. Where the sync agent resolves
//! an element first (`docs/sync-protocol.md`), the engine's `rb_sync::Replay` does that and then
//! calls [`ViewHandle::dispatch_input`]; the hub reaches every engine through
//! `rb_sync::EngineRouter`.

use std::sync::Arc;

use async_trait::async_trait;
use rb_config::{Breakpoint, EngineKind};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

use crate::emulation_report::EmulationReport;
use crate::external::{CellScale, NativeWindow, WindowRect};
use crate::{Emulation, EngineError, ViewEvent, ViewId};

/// The engine's name for people: `Chromium`, `Firefox`, `WebKit`.
pub fn engine_name(kind: EngineKind) -> &'static str {
    match kind {
        EngineKind::Chromium => "Chromium",
        EngineKind::Firefox => "Firefox",
        EngineKind::Webkit => "WebKit",
    }
}

/// How an engine is doing.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "state", content = "reason")]
pub enum EngineHealth {
    /// Started and answering.
    Running,
    /// Not started (yet), or stopped by [`Engine::shutdown`]; starts on the next view.
    Stopped,
    /// Cannot run: not installed, not implemented yet. The reason is shown to the user.
    Unavailable(String),
}

/// What a new view is: the breakpoint, its resolved emulation, the first URL and the window.
#[derive(Clone, Debug, PartialEq)]
pub struct ViewSpec {
    /// The id the view should have (an external view keeps its cell's id); `None` lets the
    /// engine's host assign one (Chromium's grid does).
    pub view: Option<ViewId>,
    /// The breakpoint.
    pub breakpoint: Breakpoint,
    /// The resolved emulation ([`crate::resolve_emulation`]).
    pub emulation: Emulation,
    /// The first URL; empty for none.
    pub url: String,
    /// The window title of an external view (`rBrowser - <Label> - <Engine>`, with middle dots).
    pub title: String,
    /// Where an external view's window goes; ignored for the grid.
    pub window: Option<WindowRect>,
    /// The scale into the grid cell an external view opens with (RB-080-40), when the cell is
    /// known already; `None` opens at the breakpoint's size. Ignored for the grid.
    pub cell: Option<CellScale>,
}

/// A mouse button.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Button {
    /// No button (a move).
    None,
    /// The main button.
    Left,
    /// The wheel button.
    Middle,
    /// The context button.
    Right,
}

/// The phase of a pointer or key event.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Phase {
    /// Pressed.
    Down,
    /// Released.
    Up,
    /// Moved (pointers only).
    Move,
}

/// One input event in CSS px of the view's viewport; the engine scales and replays it as real
/// input (CDP `Input.*`, BiDi `input.performActions`, Playwright's mouse and keyboard).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "type"
)]
pub enum InputEvent {
    /// A mouse event.
    Mouse {
        /// Down, up or move.
        phase: Phase,
        /// CSS px from the viewport's left.
        x: f64,
        /// CSS px from the viewport's top.
        y: f64,
        /// The button.
        button: Button,
        /// The click count (2 for a double click).
        clicks: u32,
        /// CDP modifier bits (`rb_sync::MOD_*`): Alt 1, Ctrl 2, Meta 4, Shift 8.
        modifiers: u8,
    },
    /// A wheel turn at a point.
    Wheel {
        /// CSS px from the viewport's left.
        x: f64,
        /// CSS px from the viewport's top.
        y: f64,
        /// Horizontal delta in CSS px.
        delta_x: f64,
        /// Vertical delta in CSS px.
        delta_y: f64,
    },
    /// A key.
    Key {
        /// Down or up.
        phase: Phase,
        /// `KeyboardEvent.key`.
        key: String,
        /// `KeyboardEvent.code`.
        code: String,
        /// The text a key down types, if any.
        text: Option<String>,
        /// CDP modifier bits (`rb_sync::MOD_*`): Alt 1, Ctrl 2, Meta 4, Shift 8.
        modifiers: u8,
    },
    /// Text typed at the focus (IME commit, paste-like input).
    Text {
        /// The text.
        text: String,
    },
}

/// What a screenshot covers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenshotRequest {
    /// The whole page instead of the viewport.
    pub full_page: bool,
}

/// A screenshot: PNG, base64 as every engine's protocol delivers it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Screenshot {
    /// The PNG, base64.
    pub png_base64: String,
}

/// What happens to a view, as its engine reports it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HandleEvent {
    /// A navigation committed.
    Navigated {
        /// The new URL.
        url: String,
    },
    /// The title, loading state or size changed.
    Updated,
    /// The engine built the view's window anew (a WebKit context at another cell factor,
    /// RB-080-40): the old window is gone and the app binds the new one to the cell again.
    WindowReplaced,
    /// The view is gone: closed by the user (`crashed: false`) or its engine died.
    Exited {
        /// The engine process ended or stopped answering.
        crashed: bool,
    },
}

/// What [`ViewHandle::set_cell`] did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CellApplied {
    /// The engine built the view's window anew for the scale (WebKit: a new context), so the
    /// app has to bind the new window to the cell.
    pub window_replaced: bool,
}

/// The events of one view, as [`ViewHandle::events`] hands them out.
pub struct HandleEvents {
    source: Source,
}

enum Source {
    /// The engine sends the view's events itself.
    Direct(broadcast::Receiver<HandleEvent>),
    /// The view's share of a [`crate::ViewHost`]'s events (Chromium).
    Host {
        view: ViewId,
        events: broadcast::Receiver<ViewEvent>,
    },
}

impl HandleEvents {
    /// Events an engine sends on its own channel.
    pub fn direct(events: broadcast::Receiver<HandleEvent>) -> Self {
        Self {
            source: Source::Direct(events),
        }
    }

    /// `view`'s events among a [`crate::ViewHost`]'s.
    pub fn from_host(view: ViewId, events: broadcast::Receiver<ViewEvent>) -> Self {
        Self {
            source: Source::Host { view, events },
        }
    }

    /// The next event; `None` once the view's events end. Missed events are skipped.
    pub async fn recv(&mut self) -> Option<HandleEvent> {
        use broadcast::error::RecvError;
        loop {
            match &mut self.source {
                Source::Direct(events) => match events.recv().await {
                    Ok(event) => return Some(event),
                    Err(RecvError::Lagged(_)) => {}
                    Err(RecvError::Closed) => return None,
                },
                Source::Host { view, events } => match events.recv().await {
                    Ok(ViewEvent::Navigated { view: from, url }) if from == *view => {
                        return Some(HandleEvent::Navigated { url });
                    }
                    Ok(ViewEvent::Updated(from)) if from == *view => {
                        return Some(HandleEvent::Updated);
                    }
                    Ok(ViewEvent::Closed(from)) if from == *view => {
                        return Some(HandleEvent::Exited { crashed: false });
                    }
                    Ok(_) | Err(RecvError::Lagged(_)) => {}
                    Err(RecvError::Closed) => return None,
                },
            }
        }
    }
}

/// One view of an engine. Every method returns quickly or awaits the engine; none blocks a
/// thread. After [`ViewHandle::close`] every other operation is [`EngineError::Closed`].
#[async_trait]
pub trait ViewHandle: Send + Sync {
    /// The view's id.
    fn id(&self) -> ViewId;

    /// The view's engine.
    fn engine(&self) -> EngineKind;

    /// Whether the view lives in a window of its own outside the grid.
    fn is_external(&self) -> bool;

    /// Loads `url`.
    async fn navigate(&self, url: &str) -> Result<(), EngineError>;

    /// Reloads; `ignore_cache` bypasses the cache.
    async fn reload(&self, ignore_cache: bool) -> Result<(), EngineError>;

    /// History back.
    async fn back(&self) -> Result<(), EngineError>;

    /// History forward.
    async fn forward(&self) -> Result<(), EngineError>;

    /// Applies `emulation`, live where the engine allows; the report says per field what was
    /// applied, approximated or not at all (the "not emulable" marker at the view).
    async fn apply_emulation(&self, emulation: &Emulation) -> Result<EmulationReport, EngineError>;

    /// Replays one input event as real input.
    async fn dispatch_input(&self, input: &InputEvent) -> Result<(), EngineError>;

    /// A screenshot of the viewport or the whole page.
    async fn screenshot(&self, request: ScreenshotRequest) -> Result<Screenshot, EngineError>;

    /// Moves and sizes an external view's window; [`EngineError::Unsupported`] in the grid.
    async fn set_window(&self, rect: WindowRect) -> Result<(), EngineError>;

    /// Brings an external view's window to the front; [`EngineError::Unsupported`] in the grid.
    async fn raise(&self) -> Result<(), EngineError>;

    /// How to find an external view's native window, so the app can show it in its grid cell
    /// (RB-080-39): `None` in the grid, or while the engine does not know its process.
    fn native_window(&self) -> Option<NativeWindow> {
        None
    }

    /// Scales an external view's page into its grid cell (RB-080-40): the viewport and factor
    /// of `scale`, or with `None` the breakpoint's size again (the window is its own). The
    /// answer says whether the engine had to build the window anew for it (the app binds it to
    /// the cell again). [`EngineError::Unsupported`] in the grid, which scales itself.
    async fn set_cell(&self, scale: Option<CellScale>) -> Result<CellApplied, EngineError> {
        let _ = scale;
        Err(EngineError::Unsupported {
            engine: self.engine(),
            feature: "scaling into the cell".to_owned(),
        })
    }

    /// The view's events from now on.
    fn events(&self) -> HandleEvents;

    /// Closes the view (an external window closes; a grid view is only let go - the layout
    /// owns it). Closing twice is fine.
    async fn close(&self) -> Result<(), EngineError>;
}

/// One browser engine: starts on its first view, makes views, stops with the app.
#[async_trait]
pub trait Engine: Send + Sync {
    /// Which engine.
    fn kind(&self) -> EngineKind;

    /// Whether its views live in external windows.
    fn external(&self) -> bool;

    /// How it is doing.
    fn health(&self) -> EngineHealth;

    /// Starts the engine if it is not running and opens a view for `spec`.
    async fn create_view(&self, spec: &ViewSpec) -> Result<Arc<dyn ViewHandle>, EngineError>;

    /// Closes every view and stops the engine (the app quits, a restart).
    async fn shutdown(&self);
}

#[cfg(test)]
#[path = "engine_tests.rs"]
mod tests;
