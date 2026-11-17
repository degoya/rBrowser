//! The render views of a sitemap capture over the app's own CDP connection
//! ([`CdpRenderBackend`], `rb_engine::chromium::hidden::RenderBackend`): each view is a hidden
//! Chromium target (`Target.createTarget { hidden: true }`, a background target where hidden
//! ones are refused, as the analyser's page), so it never shows in the grid, takes no input and
//! is never seen by an outer CDP client - its id is registered with the app's hidden targets at
//! once. A named session's target is made in that session's browser context, found through a
//! grid view showing the session; a session no view shows is refused, never swapped for the
//! default one.
//!
//! A view gets its breakpoint's emulation once (device metrics at scale 1, then the calls of
//! `crate::emulation::cdp`); a page loads with `Page.navigate` and its `load` lifecycle event;
//! its HTTP status and where it ended come from the page's navigation timing. Waiting follows
//! [`WaitSpec`]: network idle (at most [`NETWORK_IDLE_MAX`], a page that never goes quiet is
//! still captured), a selector, scrolling to the end and back for lazy content, a further delay.
//! The screenshot is `crate::capture::cdp::capture` on the view's session, on a page prepared
//! by `crate::capture::clean` as the run's [`CleanPlan`] says (RB-080-12: banners hidden,
//! animations frozen, undone right after).

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

/// Finds a grid view's target showing `session`, for its browser context.
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
    target: String,
    session: String,
    metrics: Value,
    /// The emulated media of the breakpoint, put back after a capture's preparation.
    media: Value,
    height: u32,
    dpr: f64,
    navigation: Option<Navigation>,
}

/// Render views as hidden targets of `cdp`.
pub struct CdpRenderBackend {
    cdp: Arc<CdpRouter>,
    session_target: SessionTarget,
    views: Mutex<BTreeMap<RenderId, Arc<tokio::sync::Mutex<View>>>>,
    next: AtomicU32,
    clean: CleanPlan,
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
    /// Render views on `cdp`; `session_target` names a grid view's target per session.
    pub fn new(cdp: Arc<CdpRouter>, session_target: SessionTarget) -> Self {
        Self {
            cdp,
            session_target,
            views: Mutex::new(BTreeMap::new()),
            next: AtomicU32::new(1),
            clean: CleanPlan::default(),
        }
    }

    /// The same backend, each screenshot on a page prepared as `plan` says.
    #[must_use]
    pub fn with_clean(mut self, plan: CleanPlan) -> Self {
        self.clean = plan;
        self
    }

    fn view(&self, id: RenderId) -> Result<Arc<tokio::sync::Mutex<View>>, EngineError> {
        lock(&self.views)
            .get(&id)
            .cloned()
            .ok_or(EngineError::Closed)
    }

    /// The browser context of `session`: none for the default one.
    async fn context(&self, session: &str) -> Result<Option<String>, EngineError> {
        if session == DEFAULT_SESSION {
            return Ok(None);
        }
        let target = (self.session_target)(session)
            .ok_or_else(|| invalid(format!("no view shows the session {session}")))?;
        let info = self
            .cdp
            .call("Target.getTargetInfo", json!({ "targetId": target }), None)
            .await?;
        text_of(&info, "/targetInfo/browserContextId")
            .map(Some)
            .ok_or_else(|| invalid(format!("the session {session} has no browser context")))
    }

    /// A hidden target at `marker`, in `context`.
    async fn create(&self, marker: &str, context: Option<&str>) -> Result<String, EngineError> {
        let with = |mut params: Value| {
            if let Some(context) = context {
                params["browserContextId"] = json!(context);
            }
            params
        };
        let hidden = with(json!({ "url": marker, "hidden": true, "background": true }));
        let created = match self.cdp.call("Target.createTarget", hidden, None).await {
            Ok(created) => created,
            Err(error) => {
                tracing::info!(%error, "no hidden target; the render view is a background one");
                let background = with(json!({ "url": marker, "background": true }));
                self.cdp
                    .call("Target.createTarget", background, None)
                    .await?
            }
        };
        text_of(&created, "/targetId").ok_or_else(|| invalid("createTarget gave no targetId"))
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

    async fn close_target(&self, target: &str) {
        match self
            .cdp
            .call("Target.closeTarget", json!({ "targetId": target }), None)
            .await
        {
            Ok(_) => crate::devtools::hidden::forget(target),
            Err(error) => tracing::warn!(%error, "a render view was not closed"),
        }
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
        let context = self.context(&spec.session).await?;
        let marker = format!("{MARKER_PREFIX}{}", id.0);
        let target = self.create(&marker, context.as_deref()).await?;
        crate::devtools::hidden::register(&target);
        let session = match self.prepare(&target, spec).await {
            Ok(session) => session,
            Err(error) => {
                self.close_target(&target).await;
                return Err(error);
            }
        };
        let view = View {
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
        let shot = capture(&self.cdp, &target, &shot, 1.0, &restore);
        clean::around(&self.cdp, &view.session, &self.clean, &view.media, shot).await
    }

    async fn close(&self, view: RenderId) {
        let Some(view) = lock(&self.views).remove(&view) else {
            return;
        };
        let target = view.lock().await.target.clone();
        self.close_target(&target).await;
    }
}

#[cfg(test)]
#[path = "backend_tests.rs"]
mod tests;
