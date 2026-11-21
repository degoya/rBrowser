//! The capture preparation on a view's flat CDP session (RB-080-12): before a capture the
//! noise goes and the motion stops, afterwards everything is put back - for every capture path:
//! the grid's views (`super::chromium::HostShooter`: screenshots, the overview, visual
//! regression) and the hidden render views (`crate::sitemap::backend`: sitemap capture and the
//! environment comparison). What is done is `rb_visual::prepare`'s [`CleanPlan`]:
//!
//! 1. freeze: the document timeline's rate is read and set to 0 (`Animation.setPlaybackRate`),
//!    the view's emulated media gets `prefers-reduced-motion: reduce`;
//! 2. the page script runs in rBrowser's own isolated world ([`WORLD`], out of the page's
//!    reach): the style element that hides the selectors and pauses CSS animations, transitions
//!    and the caret, the videos paused on their first frame; it answers a token;
//! 3. a short settle wait; then the capture;
//! 4. the undo, in reverse and whatever the capture gave: the script's undo in the same world,
//!    the view's own media, the timeline's own rate (a view the user stopped keeps 0,
//!    RB-094-07).
//!
//! Everything is async on the caller's runtime (never the UI thread). A step that fails is
//! logged and skipped: a banner left visible must not cost the screenshot. Each step has
//! [`STEP_TIMEOUT`], so a hung page cannot hold a capture for the router's 30 s.

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use rb_engine::EngineError;
use rb_visual::CleanPlan;
use rb_visual::prepare::{WORLD, reduced_motion_media, undo_expression};
use serde_json::{Value, json};

use crate::cdp::CdpRouter;

/// The longest one step of the preparation or the undo may take.
pub const STEP_TIMEOUT: Duration = Duration::from_secs(5);

/// What a preparation changed, for its undo.
#[derive(Debug, Default, PartialEq)]
pub struct Prepared {
    session: String,
    /// The isolated world the script ran in.
    context: Option<i64>,
    /// The script's token for its undo.
    token: Option<u64>,
    /// The timeline's rate before; set when the rate may have been changed.
    rate: Option<f64>,
    /// The view's emulated media before; set when it may have been changed.
    media: Option<Value>,
}

impl Prepared {
    /// Whether the page script prepared the page (and has something to undo).
    pub fn is_prepared(&self) -> bool {
        self.token.is_some()
    }
}

async fn call(
    cdp: &CdpRouter,
    method: &str,
    params: Value,
    session: &str,
) -> Result<Value, EngineError> {
    match tokio::time::timeout(STEP_TIMEOUT, cdp.call(method, params, Some(session))).await {
        Ok(result) => result,
        Err(_) => Err(EngineError::Cdp {
            method: method.to_owned(),
            code: 0,
            message: format!("no answer within {} s", STEP_TIMEOUT.as_secs()),
        }),
    }
}

/// The execution context of [`WORLD`] in the main frame of `session`'s page.
async fn world(cdp: &CdpRouter, session: &str) -> Result<i64, EngineError> {
    let tree = call(cdp, "Page.getFrameTree", json!({}), session).await?;
    let frame = tree
        .pointer("/frameTree/frame/id")
        .and_then(Value::as_str)
        .ok_or_else(|| EngineError::Invalid("no main frame".to_owned()))?
        .to_owned();
    let world = call(
        cdp,
        "Page.createIsolatedWorld",
        json!({ "frameId": frame, "worldName": WORLD }),
        session,
    )
    .await?;
    world
        .get("executionContextId")
        .and_then(Value::as_i64)
        .ok_or_else(|| EngineError::Invalid("no isolated world".to_owned()))
}

/// The value of `expression` in `context`; an exception is an error.
async fn evaluate(
    cdp: &CdpRouter,
    session: &str,
    context: i64,
    expression: &str,
) -> Result<Value, EngineError> {
    let params = json!({
        "expression": expression,
        "contextId": context,
        "returnByValue": true,
        "awaitPromise": true,
    });
    let answer = call(cdp, "Runtime.evaluate", params, session).await?;
    if answer.get("exceptionDetails").is_some() {
        return Err(EngineError::Invalid(
            "the capture preparation script threw".to_owned(),
        ));
    }
    Ok(answer
        .pointer("/result/value")
        .cloned()
        .unwrap_or(Value::Null))
}

/// Prepares the page of `session` as `plan` says; `media` is the view's own
/// `Emulation.setEmulatedMedia` parameters (put back by the undo).
pub async fn prepare(
    cdp: &Arc<CdpRouter>,
    session: &str,
    plan: &CleanPlan,
    media: &Value,
) -> Prepared {
    let mut prepared = Prepared {
        session: session.to_owned(),
        ..Prepared::default()
    };
    if plan.is_noop() {
        return prepared;
    }
    if plan.freeze {
        // A rate of 0 is a capture's that was never undone - unless the user stopped the view's
        // animations (RB-094-07): then the held rate goes back, else 1.
        let held = || crate::time_control::held_rate(session).unwrap_or(1.0);
        let rate = match call(cdp, "Animation.getPlaybackRate", json!({}), session).await {
            Ok(answer) => answer
                .get("playbackRate")
                .and_then(Value::as_f64)
                .filter(|rate| rate.is_finite() && *rate > 0.0)
                .unwrap_or_else(held),
            Err(error) => {
                tracing::debug!(%error, "the timeline's rate is unknown; the held one or 1 is put back");
                held()
            }
        };
        // Kept before the call: one that failed may still have taken effect.
        prepared.rate = Some(rate);
        let stop = json!({ "playbackRate": 0.0 });
        if let Err(error) = call(cdp, "Animation.setPlaybackRate", stop, session).await {
            tracing::warn!(%error, "the animations' timeline was not stopped");
        }
        prepared.media = Some(media.clone());
        let reduced = reduced_motion_media(media);
        if let Err(error) = call(cdp, "Emulation.setEmulatedMedia", reduced, session).await {
            tracing::warn!(%error, "prefers-reduced-motion was not set for the capture");
        }
    }
    match world(cdp, session).await {
        Ok(context) => {
            prepared.context = Some(context);
            match evaluate(cdp, session, context, &plan.prepare_expression()).await {
                Ok(answer) => {
                    prepared.token = answer.get("token").and_then(Value::as_u64);
                    tracing::debug!(%answer, "the page is prepared for the capture");
                }
                Err(error) => tracing::warn!(%error, "the page was not prepared"),
            }
        }
        Err(error) => tracing::warn!(%error, "no isolated world for the capture preparation"),
    }
    let settle = plan.settle();
    if !settle.is_zero() {
        tokio::time::sleep(settle).await;
    }
    prepared
}

impl Prepared {
    /// Puts the page back as it was before [`prepare`], in reverse order.
    pub async fn undo(self, cdp: &Arc<CdpRouter>) {
        let session = self.session.as_str();
        if let (Some(context), Some(token)) = (self.context, self.token)
            && let Err(error) = evaluate(cdp, session, context, &undo_expression(token)).await
        {
            tracing::warn!(%error, "the capture preparation was not undone");
        }
        if let Some(media) = self.media
            && let Err(error) = call(cdp, "Emulation.setEmulatedMedia", media, session).await
        {
            tracing::warn!(%error, "the view's emulated media was not put back");
        }
        if let Some(rate) = self.rate {
            let params = json!({ "playbackRate": rate });
            if let Err(error) = call(cdp, "Animation.setPlaybackRate", params, session).await {
                tracing::warn!(%error, "the animations' timeline was not started again");
            }
        }
    }
}

/// Runs `capture` on a page prepared as `plan` says and undoes the preparation afterwards,
/// whatever the capture gave - also when this future is dropped before (a caller's timeout, a
/// cancelled run): the undo then runs in a task of its own.
pub async fn around<T>(
    cdp: &Arc<CdpRouter>,
    session: &str,
    plan: &CleanPlan,
    media: &Value,
    capture: impl Future<Output = T>,
) -> T {
    let mut guard = UndoOnDrop {
        cdp: cdp.clone(),
        prepared: Some(prepare(cdp, session, plan, media).await),
    };
    let result = capture.await;
    if let Some(prepared) = guard.prepared.take() {
        prepared.undo(cdp).await;
    }
    result
}

/// Undoes a preparation whose capture was dropped (Charge 10 review L7): a page must not stay
/// frozen, in reduced motion and without its banners.
struct UndoOnDrop {
    cdp: Arc<CdpRouter>,
    prepared: Option<Prepared>,
}

impl Drop for UndoOnDrop {
    fn drop(&mut self) {
        let Some(prepared) = self.prepared.take() else {
            return;
        };
        let cdp = self.cdp.clone();
        match tokio::runtime::Handle::try_current() {
            Ok(runtime) => {
                runtime.spawn(async move { prepared.undo(&cdp).await });
            }
            Err(_) => tracing::warn!("a dropped capture's preparation could not be undone"),
        }
    }
}

#[cfg(test)]
#[path = "clean_tests.rs"]
mod tests;
