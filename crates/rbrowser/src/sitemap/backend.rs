//! The render views of a sitemap capture and an environment comparison over the app's own CDP
//! connection ([`CdpRenderBackend`], `rb_engine::chromium::hidden::RenderBackend`): each view is
//! a windowless Chromium browser the window makes at the view's marker URL
//! (`super::render_views`, RB-090-83) - it paints, so `Page.captureScreenshot` gets its frames,
//! but it never shows in the grid and takes no input. Its target is found by that URL among
//! `Target.getTargets` and registered with the app's hidden targets at once, so no outer CDP
//! client sees it; it closes through its browser's host. A named session's view lives in that
//! session's request context; a session no grid view shows is refused, never swapped for the
//! default one.
//!
//! A view gets its breakpoint's emulation once (device metrics at scale 1, then the calls of
//! `crate::emulation::cdp`); a page loads with `Page.navigate` and its `load` lifecycle event;
//! its HTTP status and where it ended come from the page's navigation timing. Waiting follows
//! [`WaitSpec`]: network idle (at most [`NETWORK_IDLE_MAX`], a page that never goes quiet is
//! still captured), a selector, scrolling to the end and back for lazy content, a further delay.
//! The screenshot is `crate::capture::cdp::capture` on the view's session, on a page prepared
//! by `crate::capture::clean` as the run's [`CleanPlan`] says (RB-080-12: banners hidden,
//! animations frozen, undone right after), within its own deadline ([`SHOT_TIMEOUT`], the
//! setting `sitemap.shotTimeoutSec`) instead of the router's 30 s.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use rb_engine::chromium::hidden::{Loaded, RenderBackend, RenderId, RenderSpec, WaitSpec};
use rb_engine::{Emulation, EngineError};
use rb_visual::CleanPlan;
use serde_json::{Value, json};
use tokio::sync::broadcast::Receiver;
use tokio::sync::broadcast::error::RecvError;

use super::render_views::{
    FIND_DEADLINE, RenderView, RenderViews, find_target, forget_when_closed, next_marker,
};
use super::scripts;
use crate::capture::cdp::{CaptureTarget, Shot, capture};
use crate::capture::clean;
use crate::cdp::{CdpEvent, CdpRouter, lock};
use crate::emulation::cdp::{emulation_calls, media_params, metrics_params};
use crate::fit::DeviceMetrics;

/// The URL a render view starts with, the view's number after it.
pub const MARKER_PREFIX: &str = "about:blank#rb-render-";
/// The longest wait for network idle before the page is captured anyway.
pub const NETWORK_IDLE_MAX: Duration = Duration::from_secs(10);
/// How often a selector is looked for.
const SELECTOR_POLL: Duration = Duration::from_millis(100);
/// The session of the grid's default browser context.
pub const DEFAULT_SESSION: &str = "default";
/// The longest one screenshot may take (`sitemap.shotTimeoutSec`'s default).
pub const SHOT_TIMEOUT: Duration = Duration::from_secs(15);

/// A grid view's target showing a session: its render views live in that view's browser
/// context, and a target in any other is never taken for one.
pub type SessionTarget = Box<dyn Fn(&str) -> Option<String> + Send + Sync>;

/// Where a page's load stands, kept from `navigate` for `wait`.
struct Navigation {
    events: Receiver<CdpEvent>,
    frame: String,
    loader: String,
    network_idle: bool,
}

/// One open render view.
struct View {
    /// The URL its browser started at: how the window closes it.
    marker: String,
    target: String,
    session: String,
    metrics: Value,
    /// The emulated media of the breakpoint, put back after a capture's preparation.
    media: Value,
    height: u32,
    dpr: f64,
    navigation: Option<Navigation>,
}

/// Render views as windowless browsers of `window`, driven over `cdp`.
pub struct CdpRenderBackend {
    cdp: Arc<CdpRouter>,
    session_target: SessionTarget,
    window: Arc<dyn RenderViews>,
    views: Mutex<BTreeMap<RenderId, Arc<tokio::sync::Mutex<View>>>>,
    next: AtomicU32,
    clean: CleanPlan,
    shot_timeout: Duration,
}

fn invalid(text: impl Into<String>) -> EngineError {
    EngineError::Invalid(text.into())
}

fn text_of(value: &Value, pointer: &str) -> Option<String> {
    value
        .pointer(pointer)
        .and_then(Value::as_str)
        .map(str::to_owned)
}

/// The device metrics of `emulation` in a render view: its own size at scale 1.
pub fn render_metrics(emulation: &Emulation) -> Value {
    let metrics = DeviceMetrics {
        width: emulation.width.max(1),
        height: emulation.height.max(1),
        dpr: if emulation.dpr > 0.0 {
            emulation.dpr
        } else {
            0.0
        },
        mobile: emulation.mobile,
        scale: 1.0,
    };
    metrics_params(&metrics, emulation.orientation)
}

impl CdpRenderBackend {
    /// Render views made by `window`, driven over `cdp`; `session_target` names a grid view's
    /// target per session.
    pub fn new(
        cdp: Arc<CdpRouter>,
        session_target: SessionTarget,
        window: Arc<dyn RenderViews>,
    ) -> Self {
        Self {
            cdp,
            session_target,
            window,
            views: Mutex::new(BTreeMap::new()),
            next: AtomicU32::new(1),
            clean: CleanPlan::default(),
            shot_timeout: SHOT_TIMEOUT,
        }
    }

    /// The same backend, each screenshot on a page prepared as `plan` says.
    #[must_use]
    pub fn with_clean(mut self, plan: CleanPlan) -> Self {
        self.clean = plan;
        self
    }

    /// The same backend, each screenshot given up after `timeout`.
    #[must_use]
    pub fn with_shot_timeout(mut self, timeout: Duration) -> Self {
        self.shot_timeout = timeout;
        self
    }

    fn view(&self, id: RenderId) -> Result<Arc<tokio::sync::Mutex<View>>, EngineError> {
        lock(&self.views)
            .get(&id)
            .cloned()
            .ok_or(EngineError::Closed)
    }

    /// The browser context `session`'s render views must be in: that of a grid view showing
    /// it. A named session no view shows is refused; the default one without a view is held to
    /// no context (its random marker and no opener still are).
    async fn context(&self, session: &str) -> Result<Option<String>, EngineError> {
        let Some(grid) = (self.session_target)(session) else {
            if session == DEFAULT_SESSION {
                return Ok(None);
            }
            return Err(invalid(format!("no view shows the session {session}")));
        };
        let info = self
            .cdp
            .call("Target.getTargetInfo", json!({ "targetId": grid }), None)
            .await?;
        match text_of(&info, "/targetInfo/browserContextId") {
            Some(context) => Ok(Some(context)),
            None if session == DEFAULT_SESSION => Ok(None),
            None => Err(invalid(format!(
                "the session {session} has no browser context"
            ))),
        }
    }

    /// Has the window make `spec`'s browser at `marker` and finds its target by that URL in
    /// the session's context; the browser is closed again when it is not found.
    async fn create(&self, marker: &str, spec: &RenderSpec) -> Result<String, EngineError> {
        let context = self.context(&spec.session).await?;
        let (sender, receiver) = tokio::sync::oneshot::channel();
        let view = RenderView::new(
            marker,
            &spec.session,
            spec.emulation.width,
            spec.emulation.height,
        );
        self.window.open(
            view,
            Box::new(move |made| {
                // The open gave up already: nothing waits for the answer.
                let _ = sender.send(made);
            }),
        );
        let made = match tokio::time::timeout(FIND_DEADLINE, receiver).await {
            Ok(Ok(made)) => made,
            Ok(Err(_)) => Err("the window dropped the render view".to_owned()),
            Err(_) => Err("the window made no render view in time".to_owned()),
        };
        if let Err(error) = made {
            // A browser made after the deadline goes again.
            self.window.close(marker.to_owned());
            return Err(invalid(error));
        }
        match find_target(&self.cdp, marker, context.as_deref()).await {
            Ok(target) => Ok(target),
            Err(error) => {
                self.window.close(marker.to_owned());
                Err(error)
            }
        }
    }

    /// Attaches `target` and gives it `spec`'s emulation; its session.
    async fn prepare(&self, target: &str, spec: &RenderSpec) -> Result<String, EngineError> {
        let attached = self
            .cdp
            .call(
                "Target.attachToTarget",
                json!({ "targetId": target, "flatten": true }),
                None,
            )
            .await?;
        let session =
            text_of(&attached, "/sessionId").ok_or_else(|| invalid("attach gave no sessionId"))?;
        let on = Some(session.as_str());
        self.cdp.call("Page.enable", json!({}), on).await?;
        self.cdp
            .call(
                "Page.setLifecycleEventsEnabled",
                json!({ "enabled": true }),
                on,
            )
            .await?;
        self.cdp
            .call(
                "Emulation.setDeviceMetricsOverride",
                render_metrics(&spec.emulation),
                on,
            )
            .await?;
        for call in emulation_calls(None, &spec.emulation, true) {
            self.cdp.call(call.method, call.params, on).await?;
        }
        Ok(session)
    }

    /// Closes the browser started at `marker`; its target `target` is forgotten as hidden
    /// once it is gone.
    async fn close_view(&self, marker: &str, target: &str) {
        self.window.close(marker.to_owned());
        forget_when_closed(&self.cdp, target).await;
    }

    async fn evaluate(&self, session: &str, expression: &str) -> Result<Value, EngineError> {
        let answer = self
            .cdp
            .call(
                "Runtime.evaluate",
                json!({
                    "expression": expression,
                    "returnByValue": true,
                    "awaitPromise": true,
                }),
                Some(session),
            )
            .await?;
        if let Some(exception) = answer.get("exceptionDetails") {
            let text = text_of(exception, "/exception/description")
                .or_else(|| text_of(exception, "/text"))
                .unwrap_or_default();
            return Err(invalid(format!(
                "the page script failed: {}",
                text.chars().take(200).collect::<String>()
            )));
        }
        Ok(answer
            .pointer("/result/value")
            .cloned()
            .unwrap_or(Value::Null))
    }
}

/// Whether `event` is the lifecycle event `name` of `navigation`'s document.
fn is_lifecycle(event: &CdpEvent, session: &str, navigation: &Navigation, name: &str) -> bool {
    event.session_id.as_deref() == Some(session)
        && event.method == "Page.lifecycleEvent"
        && event.params.get("name").and_then(Value::as_str) == Some(name)
        && event.params.get("frameId").and_then(Value::as_str) == Some(navigation.frame.as_str())
        && event.params.get("loaderId").and_then(Value::as_str) == Some(navigation.loader.as_str())
}

/// Reads `navigation`'s events until its `name` lifecycle event; `false` when the connection
/// lost events on the way (the caller then asks the page).
async fn until_lifecycle(navigation: &mut Navigation, session: &str, name: &str) -> bool {
    loop {
        match navigation.events.recv().await {
            Ok(event) => {
                if is_lifecycle(&event, session, navigation, "networkIdle") {
                    navigation.network_idle = true;
                }
                if is_lifecycle(&event, session, navigation, name) {
                    return true;
                }
            }
            Err(RecvError::Lagged(missed)) => {
                tracing::debug!(missed, "a render view missed CDP events");
                return false;
            }
            Err(RecvError::Closed) => return false,
        }
    }
}

/// The page's HTTP status (`0`: none) and where it ended, as its script answers them.
fn loaded_of(answer: &Value, asked: &str) -> Loaded {
    let status = answer
        .get("status")
        .and_then(Value::as_u64)
        .and_then(|status| u16::try_from(status).ok())
        .filter(|status| *status > 0);
    let href = answer.get("url").and_then(Value::as_str).unwrap_or(asked);
    let same = href == asked || href.strip_suffix('/') == Some(asked);
    Loaded {
        status,
        final_url: (!same).then(|| href.to_owned()),
    }
}

#[async_trait]
impl RenderBackend for CdpRenderBackend {
    async fn open(&self, spec: &RenderSpec) -> Result<RenderId, EngineError> {
        let id = RenderId(self.next.fetch_add(1, Ordering::Relaxed));
        let marker = next_marker()?;
        let target = self.create(&marker, spec).await?;
        crate::devtools::hidden::register(&target);
        let session = match self.prepare(&target, spec).await {
            Ok(session) => session,
            Err(error) => {
                self.close_view(&marker, &target).await;
                return Err(error);
            }
        };
        let view = View {
            marker,
            target,
            session,
            metrics: render_metrics(&spec.emulation),
            media: media_params(&spec.emulation),
            height: spec.emulation.height,
            dpr: spec.emulation.dpr,
            navigation: None,
        };
        lock(&self.views).insert(id, Arc::new(tokio::sync::Mutex::new(view)));
        Ok(id)
    }

    async fn navigate(&self, view: RenderId, url: &str) -> Result<Loaded, EngineError> {
        let view = self.view(view)?;
        let mut view = view.lock().await;
        let session = view.session.clone();
        view.navigation = None;
        // Every page starts at the breakpoint's size, whatever a shot before left (one given up
        // mid-way keeps a full page's height).
        self.cdp
            .call(
                "Emulation.setDeviceMetricsOverride",
                view.metrics.clone(),
                Some(&session),
            )
            .await?;
        let events = self.cdp.subscribe();
        let answer = self
            .cdp
            .call("Page.navigate", json!({ "url": url }), Some(&session))
            .await?;
        if let Some(error) = text_of(&answer, "/errorText").filter(|text| !text.is_empty()) {
            return Err(invalid(error));
        }
        let mut navigation = Navigation {
            events,
            frame: text_of(&answer, "/frameId").unwrap_or_default(),
            loader: text_of(&answer, "/loaderId").unwrap_or_default(),
            network_idle: false,
        };
        if !until_lifecycle(&mut navigation, &session, "load").await {
            // Events were lost: the page says whether it has loaded.
            while self.evaluate(&session, scripts::READY_STATE).await? != json!("complete") {
                tokio::time::sleep(SELECTOR_POLL).await;
            }
            navigation.network_idle = true;
        }
        let state = self.evaluate(&session, scripts::NAVIGATION).await?;
        view.navigation = Some(navigation);
        Ok(loaded_of(&state, url))
    }

    async fn wait(&self, view: RenderId, wait: &WaitSpec) -> Result<(), EngineError> {
        let view = self.view(view)?;
        let mut view = view.lock().await;
        let session = view.session.clone();
        if wait.network_idle
            && let Some(navigation) = view.navigation.as_mut()
            && !navigation.network_idle
        {
            // A page that never goes quiet (a socket, polling) is captured all the same.
            let _ = tokio::time::timeout(
                NETWORK_IDLE_MAX,
                until_lifecycle(navigation, &session, "networkIdle"),
            )
            .await;
        }
        view.navigation = None;
        if let Some(selector) = &wait.selector {
            let expression = scripts::has_selector(selector);
            while self.evaluate(&session, &expression).await? != json!(true) {
                tokio::time::sleep(SELECTOR_POLL).await;
            }
        }
        if wait.auto_scroll {
            self.evaluate(&session, scripts::AUTO_SCROLL).await?;
        }
        if !wait.delay.is_zero() {
            tokio::time::sleep(wait.delay).await;
        }
        Ok(())
    }

    async fn screenshot(&self, view: RenderId, full_page: bool) -> Result<Vec<u8>, EngineError> {
        let view = self.view(view)?;
        let view = view.lock().await;
        let target = CaptureTarget {
            session: view.session.clone(),
            metrics: view.metrics.clone(),
            height: view.height,
            dpr: view.dpr,
            hidden: false,
        };
        let shot = if full_page {
            Shot::FullPage
        } else {
            Shot::Viewport
        };
        let metrics = view.metrics.clone();
        let restore = move || Some(metrics.clone());
        let deadline = self.shot_timeout;
        let shot = async {
            let captured = capture(&self.cdp, &target, &shot, 1.0, &restore);
            match tokio::time::timeout(deadline, captured).await {
                Ok(result) => result,
                Err(_) => {
                    // Dropped mid-way, the capture did not put the breakpoint's size back.
                    let back = Some(view.session.as_str());
                    let metrics = view.metrics.clone();
                    if let Err(error) = self
                        .cdp
                        .call("Emulation.setDeviceMetricsOverride", metrics, back)
                        .await
                    {
                        tracing::debug!(%error, "a render view kept a shot's size");
                    }
                    Err(invalid(format!("no frame within {} s", deadline.as_secs())))
                }
            }
        };
        clean::around(&self.cdp, &view.session, &self.clean, &view.media, shot).await
    }

    async fn close(&self, view: RenderId) {
        let Some(view) = lock(&self.views).remove(&view) else {
            return;
        };
        let (marker, target) = {
            let view = view.lock().await;
            (view.marker.clone(), view.target.clone())
        };
        self.close_view(&marker, &target).await;
    }
}

#[cfg(test)]
#[path = "backend_tests.rs"]
mod tests;
