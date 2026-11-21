//! The run-time source (RB-010-13): the page loads in a hidden Chromium target of the app's own
//! CDP connection, and every stylesheet the page ends up with - cross-origin and generated ones
//! included - is read with `CSS.styleSheetAdded` and `CSS.getStyleSheetText`.
//!
//! The target is made with `Target.createTarget { hidden: true }` (no tab, no window; it lives
//! as long as the session) and closed again in every case. It starts at the marker URL
//! [`ANALYZER_MARKER_URL`] and is registered as hidden by its id at once, so no outer CDP
//! client sees it (RB-030-06). Only the domains `Target`, `Page`,
//! `DOM`, `CSS` and `Runtime` (the root font size) are used, on this target alone; the grid's
//! views are not touched.

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};
use tokio::sync::broadcast::error::RecvError;
use tokio::time::Instant;

use super::source::{Collected, Sheet};
use crate::cdp::{CdpEvent, CdpRouter};
use crate::devtools::hidden::{self, ANALYZER_MARKER_URL};

/// How long one page may take to load and settle.
pub const PAGE_TIMEOUT: Duration = Duration::from_secs(20);
/// How long after the load no new stylesheet may appear before the page counts as settled.
pub const SETTLE: Duration = Duration::from_millis(600);

/// The stylesheets of `url`, read in a hidden target.
///
/// # Errors
///
/// The target could not be made or attached, or the page did not load; the message is English.
pub async fn collect(router: &CdpRouter, url: &str) -> Result<Collected, String> {
    let mut events = router.subscribe();
    let target = create_target(router).await?;
    hidden::register(&target);
    let result = read_page(router, &mut events, &target, url).await;
    if let Err(error) = router
        .call("Target.closeTarget", json!({ "targetId": target }), None)
        .await
    {
        tracing::warn!(%error, "the analyser's hidden target was not closed");
    } else {
        hidden::forget(&target);
    }
    result
}

async fn create_target(router: &CdpRouter) -> Result<String, String> {
    let hidden = json!({ "url": ANALYZER_MARKER_URL, "hidden": true, "background": true });
    let created = match router.call("Target.createTarget", hidden, None).await {
        Ok(created) => created,
        // A browser without hidden targets: a background target instead.
        Err(error) => {
            tracing::info!(%error, "no hidden target; the analyser uses a background target");
            let background = json!({ "url": ANALYZER_MARKER_URL, "background": true });
            router
                .call("Target.createTarget", background, None)
                .await
                .map_err(|error| format!("Target.createTarget: {error}"))?
        }
    };
    created
        .get("targetId")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| "Target.createTarget gave no targetId".to_owned())
}

async fn read_page(
    router: &CdpRouter,
    events: &mut tokio::sync::broadcast::Receiver<Arc<CdpEvent>>,
    target: &str,
    url: &str,
) -> Result<Collected, String> {
    let call = |method: &'static str, params: Value, session: Option<String>| async move {
        router
            .call(method, params, session.as_deref())
            .await
            .map_err(|error| format!("{method}: {error}"))
    };
    let attached = call(
        "Target.attachToTarget",
        json!({ "targetId": target, "flatten": true }),
        None,
    )
    .await?;
    let session = attached
        .get("sessionId")
        .and_then(Value::as_str)
        .ok_or_else(|| "Target.attachToTarget gave no sessionId".to_owned())?
        .to_owned();
    for method in ["Page.enable", "DOM.enable", "CSS.enable"] {
        call(method, json!({}), Some(session.clone())).await?;
    }
    let navigated = call(
        "Page.navigate",
        json!({ "url": url }),
        Some(session.clone()),
    )
    .await?;
    if let Some(error) = navigated.get("errorText").and_then(Value::as_str) {
        return Err(format!("{url}: {error}"));
    }
    let headers = wait_for_sheets(events, &session).await;
    let mut collected = Collected::default();
    let mut inline = 0;
    for header in headers {
        let Some(id) = header.get("styleSheetId").and_then(Value::as_str) else {
            continue;
        };
        let text = match call(
            "CSS.getStyleSheetText",
            json!({ "styleSheetId": id }),
            Some(session.clone()),
        )
        .await
        {
            Ok(answer) => answer
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            Err(error) => {
                tracing::warn!(%error, "a stylesheet was not read");
                continue;
            }
        };
        let source = header
            .get("sourceURL")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let is_inline = header
            .get("isInline")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let sheet_url = if source.is_empty() || is_inline {
            inline += 1;
            format!("{url}#inline-{inline}")
        } else {
            source.to_owned()
        };
        collected.push(Sheet {
            url: sheet_url,
            text,
            media: None,
        });
    }
    collected.root_font_size_px = root_font_size(router, &session).await;
    Ok(collected)
}

/// The headers of `CSS.styleSheetAdded` (the page's own sheets) until the page has loaded and
/// no sheet came for [`SETTLE`], or [`PAGE_TIMEOUT`] passed. Only this `session`'s load and
/// sheets move the settle time; the other views' events do not.
async fn wait_for_sheets(
    events: &mut tokio::sync::broadcast::Receiver<Arc<CdpEvent>>,
    session: &str,
) -> Vec<Value> {
    let deadline = Instant::now() + PAGE_TIMEOUT;
    let mut headers = Vec::new();
    // Set by the load, moved on by each sheet after it.
    let mut settled_at: Option<Instant> = None;
    loop {
        let until = settled_at.map_or(deadline, |at| at.min(deadline));
        let now = Instant::now();
        if now >= until {
            break;
        }
        let event = match tokio::time::timeout(until - now, events.recv()).await {
            Ok(Ok(event)) => event,
            Ok(Err(RecvError::Lagged(missed))) => {
                tracing::warn!(missed, "the analyser missed CDP events");
                continue;
            }
            Ok(Err(RecvError::Closed)) | Err(_) => break,
        };
        if event.session_id.as_deref() != Some(session) {
            continue;
        }
        match event.method.as_str() {
            "CSS.styleSheetAdded" => {
                let header = event.params.get("header").cloned().unwrap_or(Value::Null);
                // The browser's own sheets and DevTools' are not the site's.
                if header.get("origin").and_then(Value::as_str) == Some("regular") {
                    headers.push(header);
                }
                if settled_at.is_some() {
                    settled_at = Some(Instant::now() + SETTLE);
                }
            }
            "Page.loadEventFired" => settled_at = Some(Instant::now() + SETTLE),
            _ => {}
        }
    }
    headers
}

async fn root_font_size(router: &CdpRouter, session: &str) -> Option<f64> {
    let expression = "parseFloat(getComputedStyle(document.documentElement).fontSize)";
    let answer = router
        .call(
            "Runtime.evaluate",
            json!({ "expression": expression, "returnByValue": true }),
            Some(session),
        )
        .await
        .ok()?;
    answer
        .pointer("/result/value")
        .and_then(Value::as_f64)
        .filter(|size| (1.0..=200.0).contains(size))
}

#[cfg(test)]
#[path = "cdp_source_tests.rs"]
mod tests;
