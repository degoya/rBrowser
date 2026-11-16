//! Carrying a reload out in the views over the [`ViewHost`]: a full reload, or the swap of
//! stylesheets and images with `Runtime.evaluate` (`swap.js`) in an isolated world of the
//! page's main frame. Pages with their own HMR client
//! are left to it when the change came from the watch (`reload.respectHmr`). Firefox and WebKit
//! views have no CDP: they always reload.

use std::time::Duration;

use rb_engine::{EngineKind, Target, ViewHost, ViewId};
use serde_json::Value;

use super::classify::Plan;
use super::mcp::{ViewAction, ViewReload};
use super::script::{HMR_PROBE, RELOAD_WORLD, evaluate_params, evaluate_value, swap_expression};

/// How long one script in one view may take; a hung view does not hold up the others for
/// the CDP router's 30 s.
const SCRIPT_TIMEOUT: Duration = Duration::from_secs(3);

/// The work in each view.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Work {
    /// Reload the page.
    Full,
    /// Swap without a reload.
    Swap {
        /// Stylesheet file names.
        css: Vec<String>,
        /// Image file names.
        images: Vec<String>,
        /// Every stylesheet, whatever its name ("reload CSS only").
        all_css: bool,
    },
}

impl Work {
    /// The work of a watch's [`Plan`]; `None` for nothing.
    #[must_use]
    pub fn from_plan(plan: Plan) -> Option<Self> {
        match plan {
            Plan::Nothing => None,
            Plan::Full => Some(Self::Full),
            Plan::Swap { css, images } => Some(Self::Swap {
                css,
                images,
                all_css: false,
            }),
        }
    }

    /// "Reload CSS only": every stylesheet.
    #[must_use]
    pub fn all_css() -> Self {
        Self::Swap {
            css: Vec::new(),
            images: Vec::new(),
            all_css: true,
        }
    }
}

/// How the work is done.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HowTo {
    /// Leave a page with its own HMR client alone (changes from the watch).
    pub respect_hmr: bool,
    /// A swap that found nothing to swap reloads instead (changes from the watch: the
    /// stylesheet may come in through JS).
    pub fallback_full: bool,
    /// A full reload bypasses the cache.
    pub ignore_cache: bool,
}

/// Does `work` in the `target` views; what happened in each.
pub async fn perform(
    host: &dyn ViewHost,
    target: Target,
    work: &Work,
    how: HowTo,
) -> Vec<ViewReload> {
    let views: Vec<(ViewId, EngineKind)> = match target {
        Target::All => host.views().iter().map(|v| (v.id, v.engine)).collect(),
        Target::View(id) => match host.view(id) {
            Some(view) => vec![(view.id, view.engine)],
            None => {
                return vec![ViewReload {
                    view: id,
                    action: ViewAction::Failed {
                        error: format!("unknown view {id}"),
                    },
                }];
            }
        },
    };
    let mut done = Vec::with_capacity(views.len());
    for (view, engine) in views {
        let action = in_view(host, view, engine, work, how).await;
        done.push(ViewReload { view, action });
    }
    done
}

async fn in_view(
    host: &dyn ViewHost,
    view: ViewId,
    engine: EngineKind,
    work: &Work,
    how: HowTo,
) -> ViewAction {
    let cdp = engine == EngineKind::Chromium;
    if cdp && how.respect_hmr && evaluate(host, view, HMR_PROBE).await == Some(Value::Bool(true)) {
        return ViewAction::LeftToHmr;
    }
    let (css, images, all_css) = match work {
        Work::Swap {
            css,
            images,
            all_css,
        } if cdp => (css, images, *all_css),
        Work::Full | Work::Swap { .. } => return reload(host, view, how.ignore_cache),
    };
    let expression = swap_expression(css, images, all_css);
    match evaluate_isolated(host, view, &expression).await {
        Some(count) => {
            let count = count.as_u64().unwrap_or(0);
            if count == 0 && how.fallback_full {
                reload(host, view, how.ignore_cache)
            } else {
                ViewAction::Swapped { count }
            }
        }
        None => ViewAction::Failed {
            error: "the stylesheets could not be swapped".into(),
        },
    }
}

fn reload(host: &dyn ViewHost, view: ViewId, ignore_cache: bool) -> ViewAction {
    match host.reload(Target::View(view), ignore_cache) {
        Ok(()) => ViewAction::Reloaded,
        Err(error) => ViewAction::Failed {
            error: error.to_string(),
        },
    }
}

/// The value of `expression` in `view`'s page; `None` when it failed, threw or took too long.
pub async fn evaluate(host: &dyn ViewHost, view: ViewId, expression: &str) -> Option<Value> {
    run(host, view, evaluate_params(expression)).await
}

/// The value of `expression` in [`RELOAD_WORLD`] of `view`'s main frame, out of the page's
/// reach; `None` when there is no such world, or as [`evaluate`]. Never the page's world.
async fn evaluate_isolated(host: &dyn ViewHost, view: ViewId, expression: &str) -> Option<Value> {
    let context = async {
        let tree = host
            .cdp_send(view, "Page.getFrameTree", serde_json::json!({}))
            .await?;
        let frame = tree
            .pointer("/frameTree/frame/id")
            .and_then(Value::as_str)
            .ok_or_else(|| rb_engine::EngineError::Invalid("no main frame".into()))?
            .to_owned();
        let world = host
            .cdp_send(
                view,
                "Page.createIsolatedWorld",
                serde_json::json!({ "frameId": frame, "worldName": RELOAD_WORLD }),
            )
            .await?;
        world
            .get("executionContextId")
            .and_then(Value::as_i64)
            .ok_or_else(|| rb_engine::EngineError::Invalid("no isolated world".into()))
    };
    let context = match tokio::time::timeout(SCRIPT_TIMEOUT, context).await {
        Ok(Ok(context)) => context,
        Ok(Err(error)) => {
            tracing::debug!(%view, %error, "no isolated world for the swap");
            return None;
        }
        Err(_) => {
            tracing::debug!(%view, "the isolated world took too long");
            return None;
        }
    };
    let mut params = evaluate_params(expression);
    params["contextId"] = Value::from(context);
    run(host, view, params).await
}

/// `Runtime.evaluate` with `params` in `view`, its value; `None` when it failed, threw or took
/// too long.
async fn run(host: &dyn ViewHost, view: ViewId, params: Value) -> Option<Value> {
    let call = host.cdp_send(view, "Runtime.evaluate", params);
    match tokio::time::timeout(SCRIPT_TIMEOUT, call).await {
        Ok(Ok(result)) => evaluate_value(&result).cloned(),
        Ok(Err(error)) => {
            tracing::debug!(%view, %error, "a hot reload script failed");
            None
        }
        Err(_) => {
            tracing::debug!(%view, "a hot reload script took too long");
            None
        }
    }
}

#[cfg(test)]
#[path = "perform_tests.rs"]
mod tests;
