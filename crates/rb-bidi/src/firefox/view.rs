//! [`FirefoxView`]: one Firefox window as an `rb_engine::ViewHandle` - navigation, history,
//! emulation with its report, real input, screenshots, the window's place, the scale into its
//! grid cell (RB-080-40: the viewport takes the cell's height; the factor is the profile's
//! `layout.css.devPixelsPerPx`, so the handle only keeps the cell), close.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use async_trait::async_trait;
use rb_engine::external::{CellScale, NativeWindow, WindowRect};
use rb_engine::{
    CellApplied, Emulation, EmulationReport, EngineError, EngineKind, HandleEvent, HandleEvents,
    InputEvent, Screenshot, ScreenshotRequest, ViewHandle, ViewId,
};
use tokio::sync::broadcast;

use super::engine::FirefoxEngine;
use super::engine_error;
use crate::browser::set_client_window_state;
use crate::browsing_context::{self as context_cmd, Wait, client_window, screenshot_data};
use crate::client::{BidiClient, lock};
use crate::emulation::{overrides, report, viewport_in, wanted_viewport};
use crate::error::BidiError;
use crate::input::{HeldKeys, actions_for, perform_actions, release_actions};
use crate::script::{evaluate, string_result};

/// How long `setViewport` may take before rBrowser reads the size back instead (ADR 0001,
/// Decision 7: Firefox never answers it under fractional OS scaling).
pub const VIEWPORT_GRACE: Duration = Duration::from_millis(300);

/// The state of one Firefox view, shared by its handle, the engine's event pump and the replay.
pub struct ViewState {
    /// The view.
    pub id: ViewId,
    /// Its top-level browsing context.
    pub context: String,
    /// The session.
    pub client: BidiClient,
    pub(crate) events: broadcast::Sender<HandleEvent>,
    navigations: broadcast::Sender<(ViewId, String)>,
    pub(crate) closed: AtomicBool,
    pub(crate) url: Mutex<String>,
    client_window: Mutex<Option<String>>,
    /// The modifier keys the replay holds down in the view.
    pub(crate) held: Mutex<HeldKeys>,
    /// The view whose events the replay played last.
    pub(crate) leader: Mutex<Option<ViewId>>,
    /// The emulation last sent, as it reached Firefox (after the engine's filter).
    applied: Mutex<Option<Emulation>>,
    /// The scale into the grid cell (RB-080-40); `None` as a window of its own.
    pub(crate) cell: Mutex<Option<CellScale>>,
    /// The Firefox process that owns the window; `None` for a Firefox started elsewhere.
    process_id: Option<u32>,
}

/// The window class of a Firefox top-level window on Windows.
const WINDOW_CLASS: &str = "MozillaWindowClass";

impl ViewState {
    pub(crate) fn new(
        id: ViewId,
        context: String,
        client: BidiClient,
        navigations: broadcast::Sender<(ViewId, String)>,
        process_id: Option<u32>,
    ) -> Self {
        Self {
            id,
            context,
            client,
            events: broadcast::channel(64).0,
            navigations,
            process_id,
            closed: AtomicBool::new(false),
            url: Mutex::new(String::new()),
            client_window: Mutex::new(None),
            held: Mutex::new(HeldKeys::default()),
            leader: Mutex::new(None),
            applied: Mutex::new(None),
            cell: Mutex::new(None),
        }
    }

    /// The emulation last sent, as it reached Firefox; `None` before the first.
    pub fn applied_emulation(&self) -> Option<Emulation> {
        lock(&self.applied).clone()
    }

    /// The scale into the grid cell the view has; `None` as a window of its own.
    pub fn cell(&self) -> Option<CellScale> {
        *lock(&self.cell)
    }

    /// Lets go of every key and button the replay holds in the view (`input.releaseActions`):
    /// another view leads now, or the person takes over this one.
    pub async fn release_input(&self) {
        lock(&self.held).clear();
        *lock(&self.leader) = None;
        if self.closed.load(Ordering::Acquire) {
            return;
        }
        if let Err(error) = self.client.send(release_actions(&self.context)).await {
            tracing::debug!(view = %self.id, %error, "input not released");
        }
    }

    /// The last committed URL.
    pub fn url(&self) -> String {
        lock(&self.url).clone()
    }

    /// A navigation committed: announced (to the view and engine-wide) when the URL changed.
    pub(crate) fn navigated(&self, url: &str) {
        let changed = {
            let mut current = lock(&self.url);
            let changed = *current != url;
            url.clone_into(&mut current);
            changed
        };
        if changed {
            // No subscriber is fine.
            let _sent = self.events.send(HandleEvent::Navigated {
                url: url.to_owned(),
            });
            let _sent = self.navigations.send((self.id, url.to_owned()));
        }
    }

    /// The view ended; `true` the first time.
    pub(crate) fn ended(&self, crashed: bool) -> bool {
        if self.closed.swap(true, Ordering::AcqRel) {
            return false;
        }
        let _sent = self.events.send(HandleEvent::Exited { crashed });
        true
    }

    fn open(&self) -> Result<(), EngineError> {
        if self.closed.load(Ordering::Acquire) {
            Err(EngineError::Closed)
        } else {
            Ok(())
        }
    }

    async fn send(
        &self,
        command: crate::protocol::Command,
    ) -> Result<serde_json::Value, EngineError> {
        self.open()?;
        self.client.send(command).await.map_err(engine_error)
    }
}

/// A Firefox view.
pub struct FirefoxView {
    pub(crate) state: Arc<ViewState>,
    pub(crate) engine: Weak<FirefoxEngine>,
}

impl FirefoxView {
    /// Navigates and waits until the new document is interactive (the first load). The URL
    /// Firefox answers is the one it ended at (after a redirect); the one asked for only when
    /// it names none.
    pub(crate) async fn first_load(&self, url: &str) -> Result<(), EngineError> {
        let answer = self
            .state
            .send(context_cmd::navigate(
                &self.state.context,
                url,
                Wait::Interactive,
            ))
            .await?;
        let loaded = answer
            .get("url")
            .and_then(serde_json::Value::as_str)
            .filter(|loaded| !loaded.is_empty())
            .unwrap_or(url);
        self.state.navigated(loaded);
        Ok(())
    }

    /// Sends the viewport of `emulation` in the view's cell (if any) with the short grace and
    /// reads it back.
    async fn send_viewport(&self, emulation: &Emulation) -> Result<(), EngineError> {
        let cell = self.state.cell();
        let command = viewport_in(&self.state.context, emulation, cell);
        match self.state.client.send_within(command, VIEWPORT_GRACE).await {
            Ok(_) | Err(BidiError::Timeout { .. }) => {}
            Err(error) => return Err(engine_error(error)),
        }
        self.verify_viewport(emulation, cell).await;
        Ok(())
    }

    /// Reads the viewport back after `setViewport`; a mismatch is logged, not fatal.
    async fn verify_viewport(&self, emulation: &Emulation, cell: Option<CellScale>) {
        let probe = evaluate(
            &self.state.context,
            true,
            "JSON.stringify([innerWidth, innerHeight, devicePixelRatio])",
        );
        let read = self
            .state
            .client
            .send(probe)
            .await
            .and_then(|answer| string_result(&answer))
            .ok()
            .and_then(|text| serde_json::from_str::<(f64, f64, f64)>(&text).ok());
        // A DPR of 0 leaves the ratio to the display: only the size is held to the emulation.
        let (width, height, dpr) = wanted_viewport(emulation, cell);
        let wanted = |found: (f64, f64, f64)| {
            (
                f64::from(width),
                f64::from(height),
                if dpr > 0.0 { dpr } else { found.2 },
            )
        };
        match read {
            Some(found) if found == wanted(found) => {}
            Some(found) => tracing::warn!(
                view = %self.state.id,
                wanted = ?wanted(found),
                ?found,
                "Firefox's viewport differs from the emulation"
            ),
            None => tracing::debug!(view = %self.state.id, "the viewport could not be read back"),
        }
    }
}

#[async_trait]
impl ViewHandle for FirefoxView {
    fn id(&self) -> ViewId {
        self.state.id
    }

    fn engine(&self) -> EngineKind {
        EngineKind::Firefox
    }

    fn is_external(&self) -> bool {
        true
    }

    async fn navigate(&self, url: &str) -> Result<(), EngineError> {
        self.state
            .send(context_cmd::navigate(&self.state.context, url, Wait::None))
            .await
            .map(drop)
    }

    async fn reload(&self, ignore_cache: bool) -> Result<(), EngineError> {
        self.state
            .send(context_cmd::reload(&self.state.context, ignore_cache))
            .await
            .map(drop)
    }

    async fn back(&self) -> Result<(), EngineError> {
        self.state
            .send(context_cmd::traverse_history(&self.state.context, -1))
            .await
            .map(drop)
    }

    async fn forward(&self) -> Result<(), EngineError> {
        self.state
            .send(context_cmd::traverse_history(&self.state.context, 1))
            .await
            .map(drop)
    }

    async fn apply_emulation(&self, emulation: &Emulation) -> Result<EmulationReport, EngineError> {
        self.state.open()?;
        let emulation = &match self.engine.upgrade() {
            Some(engine) => engine.filtered(self.state.id, emulation),
            None => emulation.clone(),
        };
        let context = &self.state.context;
        let client = &self.state.client;
        self.send_viewport(emulation).await?;
        let mut missing = Vec::new();
        for item in overrides(context, emulation) {
            match client.send(item.command.clone()).await {
                Ok(_) => {}
                Err(BidiError::Closed) => return Err(EngineError::Closed),
                Err(error) => {
                    tracing::debug!(view = %self.state.id, %error, "an emulation override failed");
                    missing.extend(item.field);
                }
            }
        }
        *lock(&self.state.applied) = Some(emulation.clone());
        Ok(report(emulation, &missing))
    }

    async fn dispatch_input(&self, input: &InputEvent) -> Result<(), EngineError> {
        let sources = actions_for(input).map_err(|feature| EngineError::Unsupported {
            engine: EngineKind::Firefox,
            feature,
        })?;
        self.state
            .send(perform_actions(&self.state.context, sources))
            .await
            .map(drop)
    }

    async fn screenshot(&self, request: ScreenshotRequest) -> Result<Screenshot, EngineError> {
        let answer = self
            .state
            .send(context_cmd::capture_screenshot(
                &self.state.context,
                request.full_page,
            ))
            .await?;
        let png_base64 = screenshot_data(&answer).map_err(engine_error)?;
        Ok(Screenshot { png_base64 })
    }

    async fn set_window(&self, rect: WindowRect) -> Result<(), EngineError> {
        let known = lock(&self.state.client_window).clone();
        let window = match known {
            Some(window) => window,
            None => {
                let tree = self
                    .state
                    .send(context_cmd::get_tree(Some(&self.state.context)))
                    .await?;
                let window = client_window(&tree).ok_or_else(|| {
                    EngineError::Invalid("Firefox named no window for the view".to_owned())
                })?;
                *lock(&self.state.client_window) = Some(window.clone());
                window
            }
        };
        self.state
            .send(set_client_window_state(&window, rect))
            .await
            .map(drop)
    }

    async fn raise(&self) -> Result<(), EngineError> {
        self.state
            .send(context_cmd::activate(&self.state.context))
            .await
            .map(drop)
    }

    /// The viewport takes the cell's height (RB-080-40); the factor is the profile's and needs
    /// no call: the app starts Firefox anew for a changed one. Nothing is sent before the first
    /// emulation (the start sends it with the cell).
    async fn set_cell(&self, scale: Option<CellScale>) -> Result<CellApplied, EngineError> {
        self.state.open()?;
        *lock(&self.state.cell) = scale;
        let applied = self.state.applied_emulation();
        if let Some(emulation) = applied {
            self.send_viewport(&emulation).await?;
        }
        Ok(CellApplied::default())
    }

    /// The window is one of the started Firefox's `MozillaWindowClass` windows (RB-080-39).
    fn native_window(&self) -> Option<NativeWindow> {
        Some(NativeWindow {
            process_id: self.state.process_id?,
            window_class: Some(WINDOW_CLASS.to_owned()),
        })
    }

    fn events(&self) -> HandleEvents {
        HandleEvents::direct(self.state.events.subscribe())
    }

    async fn close(&self) -> Result<(), EngineError> {
        if !self.state.ended(false) {
            return Ok(());
        }
        if let Some(engine) = self.engine.upgrade() {
            engine.forget(self.state.id, &self.state);
        }
        match self
            .state
            .client
            .send(context_cmd::close(&self.state.context))
            .await
        {
            Ok(_) | Err(BidiError::Closed) => Ok(()),
            Err(error) => {
                tracing::debug!(view = %self.state.id, %error, "closing the window failed");
                Ok(())
            }
        }
    }
}
