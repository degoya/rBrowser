//! The final URL after a navigation: `navigate`, `reload`, `back` and `forward` answer each
//! view's URL once its load has finished, or what it is when `mcp.navigationTimeoutMs` runs out
//! (`settled: false`). A view counts as started once it loads or shows another URL; a view that
//! shows no sign of a load within [`START_GRACE`] (`back` without history) counts as settled.

use std::time::{Duration, Instant};

use rb_engine::{ViewId, ViewInfo};
use serde::Serialize;
use serde_json::Value;

use crate::backend::Backend;

/// How long a view may take to start loading before it counts as one that will not.
pub const START_GRACE: Duration = Duration::from_millis(750);
/// How often the views are read while waiting.
const POLL: Duration = Duration::from_millis(50);

/// One view after a navigation.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewUrl {
    /// The view.
    pub view: ViewId,
    /// The breakpoint it shows.
    pub breakpoint: Option<String>,
    /// The final URL.
    pub url: String,
    /// The page title.
    pub title: String,
    /// Still loading (only when not settled).
    pub loading: bool,
}

/// The answer of a navigation tool.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Navigation {
    /// Every load finished within the timeout.
    pub settled: bool,
    /// The views, in grid order.
    pub views: Vec<ViewUrl>,
    /// The views of `target: "all"` the navigation left alone (their navigation is decoupled).
    pub skipped: Vec<ViewId>,
    /// The views it failed in, with the reason; the others went ahead.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub failed: Vec<Value>,
}

/// Waits until the views `ids` have finished the load the caller started after reading
/// `before`, at most `timeout`.
pub async fn settle(
    backend: &dyn Backend,
    ids: &[ViewId],
    before: &[ViewInfo],
    timeout: Duration,
) -> Navigation {
    let start = Instant::now();
    let grace = START_GRACE.min(timeout);
    let mut started = vec![false; ids.len()];
    loop {
        let now = backend.views();
        let current: Vec<Option<&ViewInfo>> = ids
            .iter()
            .map(|id| now.iter().find(|view| view.id == *id))
            .collect();
        let mut all_done = true;
        for (index, info) in current.iter().enumerate() {
            let Some(info) = info else {
                // Closed meanwhile: nothing to wait for.
                continue;
            };
            let old_url = before
                .iter()
                .find(|view| view.id == info.id)
                .map(|view| view.url.as_str());
            if info.loading || old_url != Some(info.url.as_str()) {
                started[index] = true;
            }
            let done = !info.loading && (started[index] || start.elapsed() >= grace);
            all_done &= done;
        }
        let elapsed = start.elapsed();
        if all_done || elapsed >= timeout {
            return Navigation {
                skipped: Vec::new(),
                failed: Vec::new(),
                settled: all_done,
                views: current
                    .into_iter()
                    .flatten()
                    .map(|info| ViewUrl {
                        view: info.id,
                        breakpoint: info.breakpoint.clone(),
                        url: info.url.clone(),
                        title: info.title.clone(),
                        loading: info.loading,
                    })
                    .collect(),
            };
        }
        tokio::time::sleep(POLL.min(timeout.saturating_sub(elapsed))).await;
    }
}

#[cfg(test)]
#[path = "settle_tests.rs"]
mod tests;
