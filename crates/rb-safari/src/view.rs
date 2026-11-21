//! [`SafariView`]: one Safari window - one WebDriver session - as an `rb_engine::ViewHandle`:
//! navigation and history (each followed by the URL Safari is at, the view's only events),
//! emulation through the window's size with its report, real input as WebDriver actions,
//! screenshots (the page through the `html` element), the window's corner, raise and close.
//! A session's commands run one at a time ([`SafariView::turn`]). Safari has no window rBrowser
//! may own (macOS has no reparenting): the view has no native window and no cell scale. An iOS
//! view (RB-095-04) is the same over a simulator's or a device's session: no window to size or
//! place, the device's size reported, a click replayed as a touch.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, Weak};

use async_trait::async_trait;
use rb_bidi::input::{HeldKeys, actions_for};
use rb_engine::external::WindowRect;
use rb_engine::{
    Emulation, EmulationReport, EngineError, EngineKind, HandleEvent, HandleEvents, InputEvent,
    Screenshot, ScreenshotRequest, ViewHandle, ViewId,
};
use rb_util::lock;
use tokio::sync::{MutexGuard, broadcast};

use crate::agent::Agent;
use crate::client::Session;
use crate::emulation::{
    VIEWPORT_PROBE, ios_report, parse_probe, position_body, report, size_body, window_size,
};
use crate::engine::SafariEngine;
use crate::error::{SafariError, engine_error_for};
use crate::ios::touch::touch_sources;

/// One Safari view.
pub struct SafariView {
    id: ViewId,
    /// Safari, or iOS (RB-095-04).
    kind: EngineKind,
    /// The simulator or device an iOS view runs on, for people (`iPhone 16 (iOS 18.2)`).
    device: Option<String>,
    session: Session,
    pub(crate) agent: Agent,
    engine: Weak<SafariEngine>,
    /// The driver the session lives on: its end ends the view.
    generation: u64,
    events: broadcast::Sender<HandleEvent>,
    closed: AtomicBool,
    url: Mutex<String>,
    /// The modifier keys the replay holds down in the window.
    pub(crate) held: Mutex<HeldKeys>,
    /// The view whose events the replay played last.
    pub(crate) leader: Mutex<Option<ViewId>>,
    turn: tokio::sync::Mutex<()>,
}

impl SafariView {
    pub(crate) fn new(
        id: ViewId,
        (kind, device): (EngineKind, Option<String>),
        session: Session,
        agent: Agent,
        engine: Weak<SafariEngine>,
        generation: u64,
    ) -> Self {
        Self {
            id,
            kind,
            device,
            session,
            agent,
            engine,
            generation,
            events: broadcast::channel(64).0,
            closed: AtomicBool::new(false),
            url: Mutex::new(String::new()),
            held: Mutex::new(HeldKeys::default()),
            leader: Mutex::new(None),
            turn: tokio::sync::Mutex::new(()),
        }
    }

    /// The session.
    pub fn session(&self) -> &Session {
        &self.session
    }

    /// The simulator or device of an iOS view (`iPhone 16 (iOS 18.2)`); `None` for Safari.
    pub fn device(&self) -> Option<&str> {
        self.device.as_deref()
    }

    /// Whether the view runs on iOS.
    pub(crate) fn ios(&self) -> bool {
        self.kind == EngineKind::Ios
    }

    /// The URL Safari was last found at.
    pub fn url(&self) -> String {
        lock(&self.url).clone()
    }

    /// The driver generation the view lives on.
    pub(crate) fn generation(&self) -> u64 {
        self.generation
    }

    /// The view's turn: one command (or one replayed event) at a time.
    pub(crate) async fn turn(&self) -> Result<MutexGuard<'_, ()>, EngineError> {
        self.open()?;
        Ok(self.turn.lock().await)
    }

    pub(crate) fn open(&self) -> Result<(), EngineError> {
        if self.closed.load(Ordering::Acquire) {
            Err(EngineError::Closed)
        } else {
            Ok(())
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

    /// `result` as the engine sees it: a gone driver ends every view of it as crashed, a session
    /// Safari no longer knows (its window was closed) ends this view as closed.
    pub(crate) fn checked<T>(&self, result: Result<T, SafariError>) -> Result<T, EngineError> {
        match result {
            Ok(value) => Ok(value),
            Err(error) if error.driver_gone() => {
                if let Some(engine) = self.engine.upgrade() {
                    engine.driver_ended(self.generation);
                }
                self.ended(true);
                Err(EngineError::Closed)
            }
            Err(SafariError::Command { error, .. }) if error == "invalid session id" => {
                if let Some(engine) = self.engine.upgrade() {
                    engine.forget(self);
                }
                self.ended(false);
                Err(EngineError::Closed)
            }
            Err(error) => Err(engine_error_for(self.kind, error)),
        }
    }

    /// Reads where Safari is and announces it when it changed.
    pub(crate) async fn read_url(&self) -> Result<(), EngineError> {
        let url = self.checked(self.session.current_url().await)?;
        let changed = {
            let mut current = lock(&self.url);
            let changed = *current != url;
            current.clone_from(&url);
            changed
        };
        if changed {
            // No subscriber is fine.
            let _sent = self.events.send(HandleEvent::Navigated { url });
        }
        Ok(())
    }

    /// Loads `url` (within the caller's turn) and reads where Safari ended up.
    pub(crate) async fn load(&self, url: &str) -> Result<(), EngineError> {
        self.checked(self.session.navigate(url).await)?;
        self.read_url().await
    }

    /// Sizes the window so the viewport is `emulation`'s, reads it back and corrects once.
    pub(crate) async fn fit(&self, emulation: &Emulation) -> Result<EmulationReport, EngineError> {
        if self.ios() {
            // The device's screen is the viewport: read it, never size it.
            let probe = self.checked(self.session.execute(VIEWPORT_PROBE, Vec::new()).await)?;
            let shown = parse_probe(&probe).map(|probe| probe.inner);
            return Ok(ios_report(emulation, self.device().unwrap_or("iOS"), shown));
        }
        let wanted = (emulation.width, emulation.height);
        let mut shown = None;
        for _ in 0..2 {
            let probe = self.checked(self.session.execute(VIEWPORT_PROBE, Vec::new()).await)?;
            let Some(probe) = parse_probe(&probe) else {
                tracing::debug!(view = %self.id, "the viewport could not be read");
                break;
            };
            shown = Some(probe.inner);
            if probe.inner == (i64::from(wanted.0), i64::from(wanted.1)) {
                break;
            }
            let size = size_body(window_size(wanted, &probe));
            self.checked(self.session.set_window_rect(size).await)?;
            shown = None;
        }
        if shown.is_none() {
            let probe = self.checked(self.session.execute(VIEWPORT_PROBE, Vec::new()).await)?;
            shown = parse_probe(&probe).map(|probe| probe.inner);
        }
        if let Some(found) =
            shown.filter(|found| *found != (i64::from(wanted.0), i64::from(wanted.1)))
        {
            tracing::warn!(view = %self.id, ?wanted, ?found, "Safari's viewport differs from the emulation");
        }
        Ok(report(emulation, shown))
    }

    /// The WebDriver input sources of `input`: touch in place of the mouse on iOS (hover and the
    /// wheel have none there, the list is then empty).
    pub(crate) fn sources_for(
        &self,
        input: &InputEvent,
    ) -> Result<Vec<serde_json::Value>, EngineError> {
        let sources = actions_for(input).map_err(|feature| EngineError::Unsupported {
            engine: self.kind,
            feature,
        })?;
        Ok(if self.ios() {
            touch_sources(sources)
        } else {
            sources
        })
    }

    /// Lets go of every key and button the replay holds (another view leads now).
    pub(crate) async fn release_input(&self) {
        lock(&self.held).clear();
        if self.open().is_err() {
            return;
        }
        if let Err(error) = self.session.release_actions().await {
            tracing::debug!(view = %self.id, %error, "input not released");
        }
    }

    /// Ends the session (the window closes); errors are only logged.
    pub(crate) async fn end_session(&self) {
        if let Err(error) = self.session.delete().await {
            tracing::debug!(view = %self.id, %error, "the Safari session did not end");
        }
    }
}

#[async_trait]
impl ViewHandle for SafariView {
    fn id(&self) -> ViewId {
        self.id
    }

    fn engine(&self) -> EngineKind {
        self.kind
    }

    fn is_external(&self) -> bool {
        true
    }

    async fn navigate(&self, url: &str) -> Result<(), EngineError> {
        let _turn = self.turn().await?;
        self.load(url).await
    }

    /// WebDriver reloads with the cache; `ignore_cache` cannot be honoured.
    async fn reload(&self, _ignore_cache: bool) -> Result<(), EngineError> {
        let _turn = self.turn().await?;
        self.checked(self.session.refresh().await)?;
        self.read_url().await
    }

    async fn back(&self) -> Result<(), EngineError> {
        let _turn = self.turn().await?;
        self.checked(self.session.back().await)?;
        self.read_url().await
    }

    async fn forward(&self) -> Result<(), EngineError> {
        let _turn = self.turn().await?;
        self.checked(self.session.forward().await)?;
        self.read_url().await
    }

    async fn apply_emulation(&self, emulation: &Emulation) -> Result<EmulationReport, EngineError> {
        let _turn = self.turn().await?;
        self.fit(emulation).await
    }

    async fn dispatch_input(&self, input: &InputEvent) -> Result<(), EngineError> {
        let sources = self.sources_for(input)?;
        if sources.is_empty() {
            return Ok(());
        }
        let _turn = self.turn().await?;
        self.checked(self.session.perform_actions(sources).await)
    }

    /// The page through the `html` element where Safari takes it; else the viewport.
    async fn screenshot(&self, request: ScreenshotRequest) -> Result<Screenshot, EngineError> {
        let _turn = self.turn().await?;
        if request.full_page {
            let page = match self.session.find_css("html").await {
                Ok(html) => self.session.element_screenshot(&html).await,
                Err(error) => Err(error),
            };
            match page {
                Ok(png_base64) => return Ok(Screenshot { png_base64 }),
                Err(error) if error.driver_gone() => return self.checked(Err(error)),
                Err(error) => {
                    tracing::debug!(view = %self.id, %error, "no page screenshot; the viewport");
                }
            }
        }
        let png_base64 = self.checked(self.session.screenshot().await)?;
        Ok(Screenshot { png_base64 })
    }

    /// Moves the window's corner; its size is the emulation's. An iOS view has no window here
    /// (the Simulator app's or the device's own screen).
    async fn set_window(&self, rect: WindowRect) -> Result<(), EngineError> {
        if self.ios() {
            return self.open();
        }
        let _turn = self.turn().await?;
        self.checked(self.session.set_window_rect(position_body(rect)).await)
    }

    async fn raise(&self) -> Result<(), EngineError> {
        let _turn = self.turn().await?;
        self.checked(self.session.focus_window().await)
    }

    fn events(&self) -> HandleEvents {
        HandleEvents::direct(self.events.subscribe())
    }

    async fn close(&self) -> Result<(), EngineError> {
        if !self.ended(false) {
            return Ok(());
        }
        if let Some(engine) = self.engine.upgrade() {
            engine.forget(self);
        }
        self.end_session().await;
        Ok(())
    }
}
