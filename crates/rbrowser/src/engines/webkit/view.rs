//! [`WebkitView`]: one WebKit window, a `ViewHandle` over the sidecar's `webkit.view.*`. In its
//! grid cell (RB-080-40) the view's context renders the breakpoint's width at the cell's factor
//! (`set_cell`); a factor WebKit cannot change live means a new context and a new window, which
//! the app binds to the cell again (`CellApplied::window_replaced`, `HandleEvent::WindowReplaced`).
//! On macOS the window always stays a window of its own (RB-097-02, [`EMBEDDABLE`]): macOS lets
//! no app take another app's window into its own (RB-110-10), so the view names no window to
//! embed and its cell says why (`cell_info::text`).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak};

use async_trait::async_trait;
use rb_config::EngineKind;
use rb_engine::emulation_report::{EmulationReport, FieldSupport};
use rb_engine::external::{CellScale, NativeWindow, WindowRect};
use rb_engine::{
    CellApplied, Emulation, EngineError, HandleEvent, HandleEvents, InputEvent, Screenshot,
    ScreenshotRequest, ViewHandle, ViewId,
};
use rb_sync::{AgentCommand, CommandResult};
use serde_json::{Value, json};
use tokio::sync::broadcast;

use super::engine::WebkitEngine;
use super::port::SidecarPort;
use super::rpc::{self, context_options_in, engine_error};
use crate::cdp::lock;

/// Events buffered per view before the oldest are dropped.
const EVENT_CAPACITY: usize = 64;

/// Whether a WebKit window can be shown in its grid cell here: not on macOS (RB-097-02).
pub const EMBEDDABLE: bool = !cfg!(target_os = "macos");

/// The window the cell embeds: the top-level window of the view's own WebKit process
/// (`process_id`), where windows can be [`EMBEDDABLE`] and the process is known.
pub fn window_hint(process_id: Option<u32>, embeddable: bool) -> Option<NativeWindow> {
    Some(NativeWindow {
        process_id: process_id.filter(|_| embeddable)?,
        window_class: None,
    })
}

/// One WebKit view.
pub struct WebkitView {
    id: ViewId,
    port: Arc<dyn SidecarPort>,
    /// Forgets the view when it is closed.
    engine: Weak<WebkitEngine>,
    events: broadcast::Sender<HandleEvent>,
    closed: AtomicBool,
    emulation: Mutex<Emulation>,
    url: Mutex<String>,
    /// The view's own WebKit process, the owner of its window (RB-080-40); `None` until the
    /// sidecar answers the open, or when it did not know it.
    process_id: Mutex<Option<u32>>,
    /// The scale into the grid cell; `None` as a window of its own.
    cell: Mutex<Option<CellScale>>,
}

impl WebkitView {
    /// The handle of view `id` of `engine`, opened with `emulation` at `url`, scaled into
    /// `cell` when the app knows it; its process is named once the sidecar answers
    /// ([`WebkitView::set_process_id`]).
    pub(super) fn new(
        id: ViewId,
        port: Arc<dyn SidecarPort>,
        engine: Weak<WebkitEngine>,
        emulation: Emulation,
        url: String,
        cell: Option<CellScale>,
    ) -> Arc<Self> {
        let (events, _) = broadcast::channel(EVENT_CAPACITY);
        Arc::new(Self {
            id,
            port,
            engine,
            events,
            closed: AtomicBool::new(false),
            emulation: Mutex::new(emulation),
            url: Mutex::new(url),
            process_id: Mutex::new(None),
            cell: Mutex::new(cell),
        })
    }

    /// The sidecar named the view's browser process.
    pub(super) fn set_process_id(&self, process_id: Option<u32>) {
        *lock(&self.process_id) = process_id;
    }

    /// The emulation in effect.
    pub fn emulation(&self) -> Emulation {
        lock(&self.emulation).clone()
    }

    /// The scale into the grid cell the view has; `None` as a window of its own.
    pub fn cell(&self) -> Option<CellScale> {
        *lock(&self.cell)
    }

    /// Sends `emulation` in `cell` to the sidecar; answers whether the context (and so the
    /// window) was built anew, which the app hears as [`HandleEvent::WindowReplaced`].
    async fn emulate(
        &self,
        emulation: &Emulation,
        cell: Option<CellScale>,
    ) -> Result<bool, EngineError> {
        let params = json!({ "view": self.id, "options": context_options_in(emulation, cell) });
        let answer = self.call(rpc::EMULATE, params).await?;
        let recreated = answer
            .get("recreated")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if recreated {
            let _ = self.events.send(HandleEvent::WindowReplaced);
        }
        Ok(recreated)
    }

    /// The last committed URL.
    pub fn url(&self) -> String {
        lock(&self.url).clone()
    }

    /// Whether the view is gone.
    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }

    /// Runs `command` in the view's sync agent.
    pub async fn agent(&self, command: &AgentCommand) -> Result<CommandResult, EngineError> {
        let answer = self
            .call(rpc::AGENT, json!({ "view": self.id, "command": command }))
            .await?;
        serde_json::from_value(answer)
            .map_err(|error| EngineError::Invalid(format!("agent answer: {error}")))
    }

    /// Replays `events`, after `before` in the agent (the replay marker); with `block_submit_ms`
    /// a submit in the view is prevented that long (leader-only submit).
    pub async fn input(
        &self,
        events: &[InputEvent],
        before: Option<&AgentCommand>,
        block_submit_ms: u32,
    ) -> Result<(), EngineError> {
        let mut params = json!({ "view": self.id, "events": events });
        if let Some(before) = before {
            params["before"] = json!(before);
        }
        if block_submit_ms > 0 {
            params["blockSubmitMs"] = json!(block_submit_ms);
        }
        self.call(rpc::INPUT, params).await.map(drop)
    }

    /// Scrolls the document to the leader's position, scaled to this view's range.
    pub async fn scroll(&self, x: f64, y: f64, max_x: f64, max_y: f64) -> Result<(), EngineError> {
        let params = json!({ "view": self.id, "x": x.max(0.0), "y": y.max(0.0), "maxX": max_x.max(0.0), "maxY": max_y.max(0.0) });
        self.call(rpc::SCROLL, params).await.map(drop)
    }

    /// The sidecar says the view committed `url`.
    pub(super) fn navigated(&self, url: String) {
        lock(&self.url).clone_from(&url);
        let _ = self.events.send(HandleEvent::Navigated { url });
    }

    /// The sidecar says the page finished loading.
    pub(super) fn updated(&self) {
        let _ = self.events.send(HandleEvent::Updated);
    }

    /// The window ended (closed by the user, crashed, or the sidecar ended).
    pub(super) fn ended(&self, crashed: bool) {
        if !self.closed.swap(true, Ordering::AcqRel) {
            let _ = self.events.send(HandleEvent::Exited { crashed });
        }
    }

    /// The engine stops: the window closes and its end is announced (`Exited`), so the
    /// external windows show it; never starts a sidecar to do so.
    pub(super) async fn end(&self) {
        if self.closed.swap(true, Ordering::AcqRel) {
            return;
        }
        let _ = self.events.send(HandleEvent::Exited { crashed: false });
        if let Some(Err(error)) = self.close_window().await {
            tracing::debug!(view = %self.id, %error, "closing the WebKit window failed");
        }
    }

    async fn close_window(&self) -> Option<Result<Value, rb_sidecar::SidecarError>> {
        self.port
            .request_if_running(rpc::CLOSE, json!({ "view": self.id }))
            .await
    }

    async fn call(&self, method: &str, params: Value) -> Result<Value, EngineError> {
        if self.is_closed() {
            return Err(EngineError::Closed);
        }
        self.port
            .request(method, params)
            .await
            .map_err(|error| engine_error(method, error))
    }

    async fn simple(&self, method: &str) -> Result<(), EngineError> {
        self.call(method, json!({ "view": self.id }))
            .await
            .map(drop)
    }
}

#[async_trait]
impl ViewHandle for WebkitView {
    fn id(&self) -> ViewId {
        self.id
    }

    fn engine(&self) -> EngineKind {
        EngineKind::Webkit
    }

    fn is_external(&self) -> bool {
        true
    }

    async fn navigate(&self, url: &str) -> Result<(), EngineError> {
        self.call(rpc::NAVIGATE, json!({ "view": self.id, "url": url }))
            .await
            .map(drop)
    }

    /// Playwright's reload has no cache switch; a reload always revalidates.
    async fn reload(&self, _ignore_cache: bool) -> Result<(), EngineError> {
        self.simple(rpc::RELOAD).await
    }

    async fn back(&self) -> Result<(), EngineError> {
        self.simple(rpc::BACK).await
    }

    async fn forward(&self) -> Result<(), EngineError> {
        self.simple(rpc::FORWARD).await
    }

    /// In its cell the page's `devicePixelRatio` is the cell's factor (Playwright's one knob
    /// for the ratio and the rendering scale), so the report marks the breakpoint's ratio as
    /// approximated there.
    async fn apply_emulation(&self, emulation: &Emulation) -> Result<EmulationReport, EngineError> {
        let cell = self.cell();
        self.emulate(emulation, cell).await?;
        lock(&self.emulation).clone_from(emulation);
        let mut report = EmulationReport::for_engine(EngineKind::Webkit, emulation);
        if cell.is_some() {
            for field in &mut report.fields {
                if field.field == "dpr" {
                    field.support = FieldSupport::Approximated(
                        "the page reports its cell's scale factor as the ratio while it sits in its cell"
                            .to_owned(),
                    );
                }
            }
        }
        Ok(report)
    }

    async fn dispatch_input(&self, input: &InputEvent) -> Result<(), EngineError> {
        self.input(std::slice::from_ref(input), None, 0).await
    }

    /// In its cell the page is scaled (RB-080-40): the screenshot is taken in CSS px then, so it
    /// keeps the breakpoint's size.
    async fn screenshot(&self, request: ScreenshotRequest) -> Result<Screenshot, EngineError> {
        let mut params = json!({ "view": self.id, "fullPage": request.full_page });
        if self.cell().is_some() {
            params["scale"] = json!("css");
        }
        let answer = self.call(rpc::SCREENSHOT, params).await?;
        serde_json::from_value(answer)
            .map_err(|error| EngineError::Invalid(format!("screenshot answer: {error}")))
    }

    /// Playwright cannot move or size a WebKit window; the window stays where WebKit put it.
    async fn set_window(&self, rect: WindowRect) -> Result<(), EngineError> {
        if self.is_closed() {
            return Err(EngineError::Closed);
        }
        tracing::debug!(view = %self.id, ?rect, "a WebKit window cannot be placed");
        Ok(())
    }

    async fn raise(&self) -> Result<(), EngineError> {
        self.simple(rpc::RAISE).await
    }

    /// The context takes the cell's height and factor (RB-080-40); a changed factor is a new
    /// context in the same process, so the window is a new one.
    async fn set_cell(&self, scale: Option<CellScale>) -> Result<CellApplied, EngineError> {
        if self.is_closed() {
            return Err(EngineError::Closed);
        }
        // Written first: an emulation applied meanwhile sends the new cell, never the old.
        let before = std::mem::replace(&mut *lock(&self.cell), scale);
        let emulation = self.emulation();
        match self.emulate(&emulation, scale).await {
            Ok(window_replaced) => Ok(CellApplied { window_replaced }),
            Err(error) => {
                *lock(&self.cell) = before;
                Err(error)
            }
        }
    }

    /// The window is the top-level window of the view's own WebKit process (RB-080-40); the
    /// class of the shell's window is not relied on. None on macOS ([`window_hint`]).
    fn native_window(&self) -> Option<NativeWindow> {
        window_hint(*lock(&self.process_id), EMBEDDABLE)
    }

    fn events(&self) -> HandleEvents {
        HandleEvents::direct(self.events.subscribe())
    }

    /// Closes the window, ends the view's events and leaves the engine; never starts a sidecar
    /// to do so.
    async fn close(&self) -> Result<(), EngineError> {
        if self.closed.swap(true, Ordering::AcqRel) {
            return Ok(());
        }
        let _ = self.events.send(HandleEvent::Exited { crashed: false });
        if let Some(engine) = self.engine.upgrade() {
            engine.forget(self);
        }
        match self.close_window().await {
            None | Some(Ok(_)) => Ok(()),
            Some(Err(error)) => match engine_error(rpc::CLOSE, error) {
                EngineError::Closed => Ok(()),
                other => Err(other),
            },
        }
    }
}

#[cfg(test)]
#[path = "view_tests.rs"]
mod tests;
