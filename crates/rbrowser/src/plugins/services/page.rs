//! `views`, `evaluate` and `cdp` of the plugins (Charge 6) over the window's views.
//!
//! - `views.list` answers every view of the grid (id, label, breakpoint, engine, size, DPR, URL,
//!   title).
//! - `evaluate` runs the plugin's script in the plugin's own isolated world `rbplugin-<id>` of
//!   the view's main frame (the world its page scripts live in; `Page.createIsolatedWorld` with
//!   that name, never the page's main world; Chromium answers the same world of the document
//!   for the same name in the view's one session, so a script's globals last from call to
//!   call, as hot reload's world does) with `Runtime.evaluate` - by value, a promise
//!   awaited, no user gesture - within the plugin's call time. Its answer is the value's JSON
//!   (`null` for `undefined` or a value JSON cannot hold); a throw is
//!   `plugins.error.host_failed` with `reason=exception` and the exception's first 200
//!   characters.
//! - `cdp` sends one command to the view's page target over the session the host holds; the
//!   domains, the method allowlist and its parameter rules are checked before
//!   (`rb_plugin_host::PLUGIN_CDP_METHODS`).
//!
//! Both reach Chromium views only: a Firefox or WebKit view answers `plugins.error.not_available`
//! naming its engine.

use std::time::Duration;

use async_trait::async_trait;
use rb_engine::{EngineError, EngineKind, ViewId};
use rb_plugin_types::{Failure, FailureKind, ViewInfo, codes};
use serde_json::{Value, json};

/// What the plugins' page services need of a window: its views and a CDP command to one.
#[async_trait]
pub trait Views: Send + Sync {
    /// The views in grid order.
    fn views(&self) -> Vec<rb_engine::ViewInfo>;
    /// The active view.
    fn active_view(&self) -> Option<ViewId>;
    /// One CDP command to `view`'s page target; its `result`.
    async fn cdp(&self, view: ViewId, method: &str, params: Value) -> Result<Value, EngineError>;
}

#[async_trait]
impl Views for crate::host::Host {
    fn views(&self) -> Vec<rb_engine::ViewInfo> {
        rb_engine::ViewHost::views(self)
    }

    fn active_view(&self) -> Option<ViewId> {
        rb_engine::ViewHost::active_view(self)
    }

    async fn cdp(&self, view: ViewId, method: &str, params: Value) -> Result<Value, EngineError> {
        rb_engine::ViewHost::cdp_send(self, view, method, params).await
    }
}

/// The isolated world of plugin `plugin_id`: the same name as its page scripts' world.
#[must_use]
pub fn world_of(plugin_id: &str) -> String {
    format!("rbplugin-{plugin_id}")
}

fn engine_name(engine: EngineKind) -> String {
    engine.name().to_owned()
}

/// A view as `views.list` answers it.
#[must_use]
pub fn plugin_view(info: &rb_engine::ViewInfo) -> ViewInfo {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a device pixel ratio is a small number; f32 holds it"
    )]
    let dpr = info.dpr as f32;
    ViewInfo {
        id: info.id.0,
        label: info.label.clone(),
        breakpoint: info.breakpoint.clone().unwrap_or_default(),
        engine: engine_name(info.engine),
        width: info.width,
        height: info.height,
        dpr,
        url: info.url.clone(),
        title: info.title.clone(),
    }
}

fn host_failed(reason: &str) -> Failure {
    Failure::coded(FailureKind::Other, codes::HOST_FAILED).with_param("reason", reason)
}

fn engine_failed(error: &EngineError) -> Failure {
    match error {
        EngineError::UnknownView(view) => Failure::coded(FailureKind::NotFound, codes::NOT_FOUND)
            .with_param("view", view.0.to_string()),
        other => host_failed(&other.to_string().chars().take(200).collect::<String>()),
    }
}

/// `view` when it is a Chromium view of `views`.
pub(super) fn chromium(views: &dyn Views, view: ViewId, service: &str) -> Result<(), Failure> {
    let info = views
        .views()
        .into_iter()
        .find(|info| info.id == view)
        .ok_or_else(|| engine_failed(&EngineError::UnknownView(view)))?;
    // The one check and its one failure, `Failure::engine_unserved` naming the view (RB-080-98).
    plugin_view(&info).require_chromium(service)
}

/// `future` within `timeout`; past it `plugins.error.host_failed` with `reason=timeout`.
async fn within<T>(
    timeout: Duration,
    future: impl Future<Output = Result<T, Failure>>,
) -> Result<T, Failure> {
    tokio::time::timeout(timeout, future)
        .await
        .unwrap_or_else(|_| Err(host_failed("timeout")))
}

/// `evaluate`: `script` in `world` of `view`'s main frame.
///
/// # Errors
///
/// The view is unknown or no Chromium view, the world cannot be made, the script threw, or it
/// took longer than `timeout`.
pub async fn evaluate(
    views: &dyn Views,
    view: ViewId,
    world: &str,
    script: &str,
    timeout: Duration,
) -> Result<String, Failure> {
    chromium(views, view, "evaluate")?;
    within(timeout, async {
        let send = |method: &'static str, params: Value| async move {
            views
                .cdp(view, method, params)
                .await
                .map_err(|error| engine_failed(&error))
        };
        let tree = send("Page.getFrameTree", json!({})).await?;
        let frame = tree
            .pointer("/frameTree/frame/id")
            .and_then(Value::as_str)
            .ok_or_else(|| host_failed("frame"))?
            .to_owned();
        let created = send(
            "Page.createIsolatedWorld",
            json!({ "frameId": frame, "worldName": world, "grantUniveralAccess": false }),
        )
        .await?;
        let context = created
            .get("executionContextId")
            .and_then(Value::as_i64)
            .ok_or_else(|| host_failed("world"))?;
        let millis = u64::try_from(timeout.as_millis()).unwrap_or(u64::MAX);
        let answer = send(
            "Runtime.evaluate",
            json!({
                "expression": script,
                "contextId": context,
                "returnByValue": true,
                "awaitPromise": true,
                "userGesture": false,
                "silent": true,
                "timeout": millis,
            }),
        )
        .await?;
        evaluated(&answer)
    })
    .await
}

/// The JSON text of a `Runtime.evaluate` answer's value.
fn evaluated(answer: &Value) -> Result<String, Failure> {
    if let Some(details) = answer.get("exceptionDetails") {
        let text = details
            .pointer("/exception/description")
            .or_else(|| details.get("text"))
            .and_then(Value::as_str)
            .unwrap_or_default();
        return Err(host_failed("exception")
            .with_param("message", text.chars().take(200).collect::<String>()));
    }
    let value = answer
        .pointer("/result/value")
        .cloned()
        .unwrap_or(Value::Null);
    Ok(value.to_string())
}

/// `cdp.send`: `method` with `params` (JSON text, an object or empty) to `view`.
///
/// # Errors
///
/// The view is unknown or no Chromium view, `params` is no JSON object, the command failed or
/// took longer than `timeout`.
pub async fn cdp(
    views: &dyn Views,
    view: ViewId,
    method: &str,
    params: &str,
    timeout: Duration,
) -> Result<String, Failure> {
    chromium(views, view, "cdp")?;
    let params: Value = if params.trim().is_empty() {
        json!({})
    } else {
        serde_json::from_str(params)
            .map_err(|_| Failure::coded(FailureKind::BadData, codes::INVALID))?
    };
    if !params.is_object() {
        return Err(Failure::coded(FailureKind::BadData, codes::INVALID));
    }
    within(timeout, async {
        // Chromium answers a resource's content only to a session with the Page domain on
        // ("Agent is not enabled"); on is harmless and stays (RB-094-13: html-validate, coverage).
        if method == RESOURCE_CONTENT {
            views
                .cdp(view, "Page.enable", json!({}))
                .await
                .map_err(|error| engine_failed(&error))?;
        }
        views
            .cdp(view, method, params)
            .await
            .map(|answer| answer.to_string())
            .map_err(|error| engine_failed(&error))
    })
    .await
}

/// The one method a plugin may send that needs the Page domain on in the view's session.
const RESOURCE_CONTENT: &str = "Page.getResourceContent";

#[cfg(test)]
#[path = "page_tests.rs"]
mod tests;
