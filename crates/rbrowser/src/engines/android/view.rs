//! [`AndroidView`]: one tab of Chrome on the device, driven over CDP (RB-095-02). The view has
//! its own forward and connection; it opens a tab (`Target.createTarget`), attaches to it as a
//! flat session and reports its main frame's navigations. Input is CDP `Input.*` as in a
//! Chromium view (`rb_engine::chromium::cdp_input`), in the device's CSS px; nothing is
//! emulated - the view has the device's size ([`EmulationReport`] marks every other field).
//! A device has no window to place or raise. A loopback address the tab loads - opening,
//! navigating (the cell's follow included), reloading, a history step - first gets its port
//! reversed to this machine ([`super::reach`], RB-096-12), and the view holds only that port:
//! the one of its address before goes. Closing gives the view's reverses back first, then
//! closes the tab, the connection and the forward.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak};

use async_trait::async_trait;
use rb_config::EngineKind;
use rb_engine::chromium::cdp_input;
use rb_engine::external::WindowRect;
use rb_engine::{
    Emulation, EmulationReport, EngineError, HandleEvent, HandleEvents, InputEvent, Screenshot,
    ScreenshotRequest, ViewHandle, ViewId,
};
use serde_json::{Value, json};
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;

use super::adb::{self, AdbRun};
use super::cdp::{CdpClient, CdpEvent};
use super::forward::{history_entry, history_url};
use super::reach::ReverseLink;
use crate::cdp::lock;

/// Events a slow subscriber may fall behind.
const EVENT_BUFFER: usize = 64;

/// Where the view's tab runs.
pub struct Link {
    /// The device.
    pub serial: String,
    /// The forwarded port on 127.0.0.1.
    pub port: u16,
    /// adb, to remove the forward.
    pub adb: Arc<dyn AdbRun>,
    /// The connection to Chrome's browser socket.
    pub client: CdpClient,
    /// The view's reverses of loopback ports.
    pub reach: ReverseLink,
}

/// One view on an Android device.
pub struct AndroidView {
    id: ViewId,
    link: Link,
    target: String,
    session: String,
    url: Mutex<String>,
    events: broadcast::Sender<HandleEvent>,
    closed: AtomicBool,
}

impl AndroidView {
    /// Opens a tab at `url` (blank when empty) and attaches to it.
    ///
    /// # Errors
    ///
    /// Chrome refused the tab or the session.
    pub async fn open(id: ViewId, link: Link, url: &str) -> Result<Arc<Self>, EngineError> {
        let first = if url.is_empty() { "about:blank" } else { url };
        link.reach
            .prepare(&link.adb, &link.serial, first, false)
            .await;
        let created = link
            .client
            .call("Target.createTarget", json!({ "url": first }), None)
            .await
            .and_then(|created| text(&created, "targetId", "Target.createTarget"));
        let target = match created {
            Ok(target) => target,
            Err(error) => {
                link.reach.release(&link.serial).await;
                return Err(error);
            }
        };
        let session = match attach(&link.client, &target).await {
            Ok(session) => session,
            Err(error) => {
                link.reach.release(&link.serial).await;
                // The tab is the view's: it does not stay open on the device without it.
                let closed = link
                    .client
                    .call("Target.closeTarget", json!({ "targetId": target }), None)
                    .await;
                if let Err(close) = closed {
                    tracing::debug!(%close, "the Android tab stays open");
                }
                return Err(error);
            }
        };
        let (events, _) = broadcast::channel(EVENT_BUFFER);
        let view = Arc::new(Self {
            id,
            link,
            target,
            session,
            url: Mutex::new(first.to_owned()),
            events,
            closed: AtomicBool::new(false),
        });
        pump(&view);
        Ok(view)
    }

    /// The URL the tab shows.
    pub fn url(&self) -> String {
        lock(&self.url).clone()
    }

    /// The view's id.
    pub fn id_of(&self) -> ViewId {
        self.id
    }

    /// The device.
    pub fn serial(&self) -> &str {
        &self.link.serial
    }

    fn open_or_closed(&self) -> Result<(), EngineError> {
        if self.closed.load(Ordering::Acquire) {
            Err(EngineError::Closed)
        } else {
            Ok(())
        }
    }

    async fn page(&self, method: &str, params: Value) -> Result<Value, EngineError> {
        self.open_or_closed()?;
        self.link
            .client
            .call(method, params, Some(&self.session))
            .await
    }

    async fn step(&self, step: i64) -> Result<(), EngineError> {
        let history = self.page("Page.getNavigationHistory", json!({})).await?;
        let Some(entry) = history_entry(&history, step) else {
            return Ok(());
        };
        if let Some(url) = history_url(&history, step) {
            self.reach(url, false).await?;
        }
        self.page("Page.navigateToHistoryEntry", json!({ "entryId": entry }))
            .await
            .map(|_| ())
    }

    /// The loopback port of `url` reaches this machine before the tab loads it. A view closed
    /// meanwhile gives the port back and loads nothing (code review w33, item 6): its `close`
    /// may have released before the reverse was set.
    async fn reach(&self, url: &str, verify: bool) -> Result<(), EngineError> {
        let link = &self.link;
        link.reach
            .prepare(&link.adb, &link.serial, url, verify)
            .await;
        if self.closed.load(Ordering::Acquire) {
            link.reach.release(&link.serial).await;
            return Err(EngineError::Closed);
        }
        Ok(())
    }

    /// What an event of the connection means for this view.
    fn on_event(&self, event: &CdpEvent) {
        let ours = event.session.as_deref() == Some(self.session.as_str());
        match event.method.as_str() {
            "Page.frameNavigated" if ours => {
                let frame = &event.params["frame"];
                if frame.get("parentId").is_none()
                    && let Some(url) = frame.get("url").and_then(Value::as_str)
                {
                    *lock(&self.url) = url.to_owned();
                    let _sent = self.events.send(HandleEvent::Navigated {
                        url: url.to_owned(),
                    });
                }
            }
            "Target.detachedFromTarget" | "Target.targetDestroyed" => {
                let gone = event.params["sessionId"].as_str() == Some(self.session.as_str())
                    || event.params["targetId"].as_str() == Some(self.target.as_str());
                if gone && !self.closed.swap(true, Ordering::AcqRel) {
                    let _sent = self.events.send(HandleEvent::Exited { crashed: false });
                }
            }
            _ => {}
        }
    }
}

/// Reads `field` of `answer`.
/// Attaches to the tab `target` as a flat session with its page events on.
async fn attach(client: &CdpClient, target: &str) -> Result<String, EngineError> {
    let attached = client
        .call(
            "Target.attachToTarget",
            json!({ "targetId": target, "flatten": true }),
            None,
        )
        .await?;
    let session = text(&attached, "sessionId", "Target.attachToTarget")?;
    client
        .call("Page.enable", json!({}), Some(&session))
        .await?;
    Ok(session)
}

fn text(answer: &Value, field: &str, method: &str) -> Result<String, EngineError> {
    answer
        .get(field)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| EngineError::Invalid(format!("{method} answered no {field}")))
}

/// Feeds the connection's events to `view` until it ends: an end the view did not ask for is
/// a crash (the cable, adb or Chrome went away).
fn pump(view: &Arc<AndroidView>) {
    let mut events = view.link.client.events();
    let weak: Weak<AndroidView> = Arc::downgrade(view);
    tokio::spawn(async move {
        loop {
            let event = events.recv().await;
            let Some(view) = weak.upgrade() else {
                return;
            };
            match event {
                Ok(event) => view.on_event(&event),
                Err(RecvError::Lagged(_)) => {}
                Err(RecvError::Closed) => {
                    if !view.closed.swap(true, Ordering::AcqRel) {
                        let _sent = view.events.send(HandleEvent::Exited { crashed: true });
                    }
                    return;
                }
            }
        }
    });
}

#[async_trait]
impl ViewHandle for AndroidView {
    fn id(&self) -> ViewId {
        self.id
    }

    fn engine(&self) -> EngineKind {
        EngineKind::Android
    }

    fn is_external(&self) -> bool {
        true
    }

    async fn navigate(&self, url: &str) -> Result<(), EngineError> {
        self.open_or_closed()?;
        self.reach(url, false).await?;
        self.page("Page.navigate", json!({ "url": url }))
            .await
            .map(|_| ())
    }

    async fn reload(&self, ignore_cache: bool) -> Result<(), EngineError> {
        self.open_or_closed()?;
        // A reload reads the list again: a reverse that went (adb restarted) is set again.
        self.reach(&self.url(), true).await?;
        self.page("Page.reload", json!({ "ignoreCache": ignore_cache }))
            .await
            .map(|_| ())
    }

    async fn back(&self) -> Result<(), EngineError> {
        self.step(-1).await
    }

    async fn forward(&self) -> Result<(), EngineError> {
        self.step(1).await
    }

    async fn apply_emulation(&self, emulation: &Emulation) -> Result<EmulationReport, EngineError> {
        self.open_or_closed()?;
        Ok(EmulationReport::for_engine(EngineKind::Android, emulation))
    }

    async fn dispatch_input(&self, input: &InputEvent) -> Result<(), EngineError> {
        let (method, params) = cdp_input(input)?;
        self.page(method, params).await.map(|_| ())
    }

    async fn screenshot(&self, request: ScreenshotRequest) -> Result<Screenshot, EngineError> {
        let answer = self
            .page(
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
        Err(no_window("window placement"))
    }

    async fn raise(&self) -> Result<(), EngineError> {
        Err(no_window("raising a window"))
    }

    fn events(&self) -> HandleEvents {
        HandleEvents::direct(self.events.subscribe())
    }

    async fn close(&self) -> Result<(), EngineError> {
        if self.closed.swap(true, Ordering::AcqRel) {
            // Ended by the device (the cable, Chrome): its reverses go all the same.
            self.link.reach.release(&self.link.serial).await;
            return Ok(());
        }
        // The reverses first: the app's quit gives every external view a few seconds in all,
        // and a port left open on the device is the one thing that outlives the app.
        self.link.reach.release(&self.link.serial).await;
        let closed = self
            .link
            .client
            .call(
                "Target.closeTarget",
                json!({ "targetId": self.target }),
                None,
            )
            .await;
        if let Err(error) = closed {
            tracing::debug!(view = %self.id, %error, "the Android tab did not close");
        }
        self.link.client.close();
        let link = &self.link;
        if let Err(error) = adb::unforward(link.adb.as_ref(), &link.serial, link.port).await {
            tracing::debug!(view = %self.id, %error, "the adb forward stays until adb ends");
        }
        Ok(())
    }
}

fn no_window(feature: &str) -> EngineError {
    EngineError::Unsupported {
        engine: EngineKind::Android,
        feature: format!("{feature} (an Android view lives on its device)"),
    }
}

#[cfg(test)]
#[path = "view_tests.rs"]
mod tests;
