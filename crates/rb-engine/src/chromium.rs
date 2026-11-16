//! Chromium behind the engine contract: [`ChromiumEngine`] and [`ChromiumView`] wrap the CEF
//! app's [`ViewHost`] (RB-010-04) and call exactly what the host already does - navigation,
//! emulation (RB-010-05), CDP `Input.*` (in CSS px; the host scales, ADR 0001, Decision 3) and
//! `Page.captureScreenshot`. The host keeps owning the grid; nothing changes for it.
//!
//! Chromium beyond the grid's views (RB-050-05): hidden render views - CEF browsers of their
//! own with a breakpoint's emulation, never placed in the grid, so the visible views stay
//! usable while a long capture runs ([`hidden`]) - and the batch that drives them, a page in a
//! breakpoint at a time ([`batch`]). The CEF side implements [`hidden::RenderBackend`] in
//! `crates/rbrowser`; Lighthouse (RB-060-07) is the next user of the pool.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub mod batch;
pub mod hidden;

#[cfg(test)]
mod fake;

use async_trait::async_trait;
use rb_config::EngineKind;
use serde_json::{Value, json};

use crate::emulation_report::EmulationReport;
use crate::engine::{
    Button, Engine, EngineHealth, HandleEvents, InputEvent, Phase, Screenshot, ScreenshotRequest,
    ViewHandle, ViewSpec,
};
use crate::external::WindowRect;
use crate::{Emulation, EngineError, Target, ViewHost, ViewId};

/// The embedded Chromium of the grid.
pub struct ChromiumEngine {
    host: Arc<dyn ViewHost>,
}

impl ChromiumEngine {
    /// Chromium over `host`.
    pub fn new(host: Arc<dyn ViewHost>) -> Self {
        Self { host }
    }

    /// The handle of the grid view `view`.
    pub fn handle(&self, view: ViewId) -> Result<Arc<dyn ViewHandle>, EngineError> {
        if self.host.view(view).is_none() {
            return Err(EngineError::UnknownView(view));
        }
        Ok(Arc::new(ChromiumView {
            host: self.host.clone(),
            id: view,
            closed: AtomicBool::new(false),
        }))
    }
}

#[async_trait]
impl Engine for ChromiumEngine {
    fn kind(&self) -> EngineKind {
        EngineKind::Chromium
    }

    fn external(&self) -> bool {
        false
    }

    fn health(&self) -> EngineHealth {
        EngineHealth::Running
    }

    /// Opens a view outside the grid order (`ViewHost::open_view`); the host assigns its id.
    async fn create_view(&self, spec: &ViewSpec) -> Result<Arc<dyn ViewHandle>, EngineError> {
        let view = self.host.open_view(&spec.breakpoint)?;
        if !spec.url.is_empty() {
            self.host.navigate(Target::View(view), &spec.url)?;
        }
        self.handle(view)
    }

    /// CEF lives as long as the app; the app shuts it down.
    async fn shutdown(&self) {}
}

/// One grid view.
pub struct ChromiumView {
    host: Arc<dyn ViewHost>,
    id: ViewId,
    closed: AtomicBool,
}

impl ChromiumView {
    fn open(&self) -> Result<Target, EngineError> {
        if self.closed.load(Ordering::Acquire) {
            Err(EngineError::Closed)
        } else {
            Ok(Target::View(self.id))
        }
    }

    async fn cdp(&self, method: &str, params: Value) -> Result<Value, EngineError> {
        self.open()?;
        self.host.cdp_send(self.id, method, params).await
    }
}

#[async_trait]
impl ViewHandle for ChromiumView {
    fn id(&self) -> ViewId {
        self.id
    }

    fn engine(&self) -> EngineKind {
        EngineKind::Chromium
    }

    fn is_external(&self) -> bool {
        false
    }

    async fn navigate(&self, url: &str) -> Result<(), EngineError> {
        self.host.navigate(self.open()?, url)
    }

    async fn reload(&self, ignore_cache: bool) -> Result<(), EngineError> {
        self.host.reload(self.open()?, ignore_cache)
    }

    async fn back(&self) -> Result<(), EngineError> {
        self.host.go_back(self.open()?)
    }

    async fn forward(&self) -> Result<(), EngineError> {
        self.host.go_forward(self.open()?)
    }

    async fn apply_emulation(&self, emulation: &Emulation) -> Result<EmulationReport, EngineError> {
        self.open()?;
        self.host.apply_emulation(self.id, emulation)?;
        Ok(EmulationReport::for_engine(EngineKind::Chromium, emulation))
    }

    async fn dispatch_input(&self, input: &InputEvent) -> Result<(), EngineError> {
        let (method, params) = cdp_input(input)?;
        self.cdp(method, params).await.map(|_| ())
    }

    async fn screenshot(&self, request: ScreenshotRequest) -> Result<Screenshot, EngineError> {
        let answer = self
            .cdp(
                "Page.captureScreenshot",
                json!({ "format": "png", "captureBeyondViewport": request.full_page }),
            )
            .await?;
        match answer.get("data").and_then(Value::as_str) {
            Some(data) if !data.is_empty() => Ok(Screenshot {
                png_base64: data.to_owned(),
            }),
            _ => Err(EngineError::Invalid(
                "Page.captureScreenshot answered no image".to_owned(),
            )),
        }
    }

    async fn set_window(&self, _rect: WindowRect) -> Result<(), EngineError> {
        self.open()?;
        Err(grid_only("window placement"))
    }

    async fn raise(&self) -> Result<(), EngineError> {
        self.open()?;
        Err(grid_only("raising a window"))
    }

    fn events(&self) -> HandleEvents {
        HandleEvents::from_host(self.id, self.host.subscribe())
    }

    /// Lets the view go; the grid's layout keeps owning it.
    async fn close(&self) -> Result<(), EngineError> {
        self.closed.store(true, Ordering::Release);
        Ok(())
    }
}

fn grid_only(feature: &str) -> EngineError {
    EngineError::Unsupported {
        engine: EngineKind::Chromium,
        feature: format!("{feature} (Chromium views live in the grid)"),
    }
}

/// The CDP command that replays `input`.
pub fn cdp_input(input: &InputEvent) -> Result<(&'static str, Value), EngineError> {
    Ok(match input {
        InputEvent::Mouse {
            phase,
            x,
            y,
            button,
            clicks,
            modifiers,
        } => (
            "Input.dispatchMouseEvent",
            json!({
                "type": match phase {
                    Phase::Down => "mousePressed",
                    Phase::Up => "mouseReleased",
                    Phase::Move => "mouseMoved",
                },
                "x": x,
                "y": y,
                "button": match button {
                    Button::None => "none",
                    Button::Left => "left",
                    Button::Middle => "middle",
                    Button::Right => "right",
                },
                "clickCount": clicks,
                "modifiers": modifiers,
            }),
        ),
        InputEvent::Wheel {
            x,
            y,
            delta_x,
            delta_y,
        } => (
            "Input.dispatchMouseEvent",
            json!({ "type": "mouseWheel", "x": x, "y": y, "deltaX": delta_x, "deltaY": delta_y }),
        ),
        InputEvent::Key {
            phase,
            key,
            code,
            text,
            modifiers,
        } => {
            let kind = match (phase, text) {
                (Phase::Down, Some(_)) => "keyDown",
                (Phase::Down, None) => "rawKeyDown",
                (Phase::Up, _) => "keyUp",
                (Phase::Move, _) => {
                    return Err(EngineError::Invalid("a key does not move".to_owned()));
                }
            };
            let mut params =
                json!({ "type": kind, "key": key, "code": code, "modifiers": modifiers });
            if let (Some(text), Phase::Down) = (text, phase) {
                params["text"] = Value::from(text.as_str());
            }
            ("Input.dispatchKeyEvent", params)
        }
        InputEvent::Text { text } => ("Input.insertText", json!({ "text": text })),
    })
}

#[cfg(test)]
#[path = "chromium_tests.rs"]
mod tests;
