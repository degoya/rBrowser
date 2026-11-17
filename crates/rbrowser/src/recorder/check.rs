//! Check steps from the page menu (RB-090-60): "Check: element visible", "Check: text visible"
//! and "Wait for element" - `recorder.check { kind, view?, x?, y?, dip? }` (`Leader F K`, a
//! visible check on the element the next click picks). The element under the pointer is
//! resolved as "Inspect element" resolves it, past rBrowser's own layers (RB-090-51,
//! [`crate::devtools::inspect::node_at`]); the sync agent names it as the click sync does
//! (`targetOf`, the same selector candidates as a recorded click), and the step runs in the
//! views of the recording whose page has that element (`onlyIn`). A recording must run.

use std::sync::Arc;

use rb_commands::CommandError;
use rb_engine::{EngineError, ViewHost as _, ViewId};
use rb_sync::ElementTarget;
use rb_sync::recorder::{Action, Assertion, Locator};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::capture::pick::{PICK_TIMEOUT, pick};
use crate::cdp::CdpRouter;
use crate::control::Core;
use crate::devtools::inspect::{Point, node_at};

/// `Leader F K`: add a check step for the element under the pointer (or the one clicked).
pub const CHECK: &str = "recorder.check";
/// The longest text a text check keeps, in characters.
pub const MAX_TEXT: usize = 500;

/// The object group of the element described, released after it.
const GROUP: &str = "rbrowser-recorder-check";
/// In the agent's world: the target of the element `this` and its visible text.
const DESCRIBE: &str = "function () {
  var text = (this.innerText || this.textContent || '').replace(/\\s+/g, ' ').trim();
  return { target: globalThis.__rbAgent.targetOf(this), text: text };
}";

/// What a check step checks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CheckKind {
    /// The element is visible.
    #[default]
    Visible,
    /// The element shows its text as it does now.
    Text,
    /// Wait until the element is there.
    WaitFor,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    #[serde(default)]
    kind: CheckKind,
    view: Option<u32>,
    x: Option<f64>,
    y: Option<f64>,
    dip: Option<bool>,
}

/// `recorder.check`'s arguments, read.
#[derive(Clone, Debug, PartialEq)]
pub struct CheckArgs {
    pub kind: CheckKind,
    /// The view; the active one when absent.
    pub view: Option<ViewId>,
    /// The point and whether it is in DIP of the view's widget; a pick without.
    pub point: Option<(Point, bool)>,
}

/// Reads `recorder.check`'s arguments: `x` and `y` together or neither (a pick).
///
/// # Errors
///
/// An unknown field or kind, a lone or negative coordinate.
pub fn parse(args: &Value) -> Result<CheckArgs, CommandError> {
    let invalid = CommandError::InvalidArgs;
    let raw: Raw = if args.is_null() {
        Raw::default()
    } else {
        serde_json::from_value(args.clone()).map_err(|error| invalid(error.to_string()))?
    };
    let point = match (raw.x, raw.y) {
        (Some(x), Some(y)) if x.is_finite() && y.is_finite() && x >= 0.0 && y >= 0.0 => {
            Some((Point { x, y }, raw.dip.unwrap_or(false)))
        }
        (None, None) => None,
        _ => {
            return Err(invalid(
                "`x` and `y` are non-negative numbers and come together, or neither".to_owned(),
            ));
        }
    };
    Ok(CheckArgs {
        kind: raw.kind,
        view: raw.view.map(ViewId),
        point,
    })
}

/// The step for `kind` on `target` showing `text`.
///
/// # Errors
///
/// A text check on an element without text.
pub fn action_of(kind: CheckKind, target: &ElementTarget, text: &str) -> Result<Action, String> {
    let target = Locator::from_target(target, None);
    Ok(match kind {
        CheckKind::Visible => Action::Assert {
            assertion: Assertion::Visible { target },
        },
        CheckKind::Text => {
            let text: String = text.chars().take(MAX_TEXT).collect();
            if text.trim().is_empty() {
                return Err("the element shows no text to check".to_owned());
            }
            Action::Assert {
                assertion: Assertion::Text { target, text },
            }
        }
        CheckKind::WaitFor => Action::WaitFor { target },
    })
}

/// `recorder.check`. Runs on a command thread of the core's runtime, never on the UI thread: the
/// resolution blocks on that runtime (a pick waits for a click).
pub fn run(core: &Core, args: &Value) -> Result<Value, CommandError> {
    let args = parse(args)?;
    let failed = CommandError::Failed;
    let recorder = super::recorder_of(core);
    if recorder.outcome().name.is_none() {
        return Err(failed(
            "no recording is running: start one, then add checks".to_owned(),
        ));
    }
    let host = core.host();
    let source = args
        .view
        .or_else(|| host.active_view())
        .ok_or_else(|| failed("there is no view".to_owned()))?;
    let page = host
        .capture_target(source)
        .map(|target| target.session)
        .ok_or_else(|| {
            failed(format!(
                "view {source} has no CDP target: only Chromium checks"
            ))
        })?;
    let (agent, context) = host.sync_agents().top_context(source).map_err(|_| {
        failed(format!(
            "view {source} has no sync agent to name the element"
        ))
    })?;
    let scale = host.view(source).map_or(1.0, |info| info.scale);
    let others: Vec<(ViewId, Option<(String, i64)>)> = host
        .views()
        .iter()
        .filter(|info| info.id != source)
        .map(|info| (info.id, host.sync_agents().top_context(info.id).ok()))
        .collect();
    let cdp = host.cdp().clone();
    let point = args.point;
    let (target, text, views) = crate::analyzer::plugins::block_on(async move {
        let backend = match point {
            Some((point, dip)) => {
                node_at(&cdp, &page, point, if dip { scale } else { 1.0 }).await?
            }
            None => pick(&cdp, &page, PICK_TIMEOUT).await?,
        };
        let (target, text) = describe(&cdp, &agent, context, backend).await?;
        let views = views_with(&cdp, source, &others, &target).await;
        Ok::<_, EngineError>((target, text, views))
    })
    .map_err(|error| failed(error.to_string()))?
    .map_err(|error| failed(error.to_string()))?;
    let target: ElementTarget = serde_json::from_value(target)
        .map_err(|_| failed("the sync agent has no selector for the element".to_owned()))?;
    let action = action_of(args.kind, &target, &text).map_err(CommandError::InvalidArgs)?;
    let outcome = recorder
        .add_check(source, action, &views)
        .map_err(|error| failed(error.to_string()))?;
    Ok(json!({ "steps": outcome.steps, "views": views }))
}

/// The views - `source` first - whose page has the element `target` names; a view whose agent
/// does not answer is left out.
async fn views_with(
    cdp: &Arc<CdpRouter>,
    source: ViewId,
    others: &[(ViewId, Option<(String, i64)>)],
    target: &Value,
) -> Vec<ViewId> {
    let mut views = vec![source];
    for (view, agent) in others {
        let Some((session, context)) = agent else {
            continue;
        };
        if let Ok(Some(_)) =
            crate::force_state::cdp::agent_find(cdp, session, *context, target).await
        {
            views.push(*view);
        }
    }
    views
}

/// The agent's target of the element `backend` and its text, in the agent's world `context`.
async fn describe(
    cdp: &Arc<CdpRouter>,
    session: &str,
    context: i64,
    backend: i64,
) -> Result<(Value, String), EngineError> {
    let described = describe_in(cdp, session, context, backend).await;
    if let Err(error) = cdp
        .call(
            "Runtime.releaseObjectGroup",
            json!({ "objectGroup": GROUP }),
            Some(session),
        )
        .await
    {
        tracing::debug!(%error, "recorder check: the object group stayed");
    }
    described
}

async fn describe_in(
    cdp: &Arc<CdpRouter>,
    session: &str,
    context: i64,
    backend: i64,
) -> Result<(Value, String), EngineError> {
    let gone = || EngineError::Invalid("the element is gone".to_owned());
    let on = Some(session);
    let resolved = cdp
        .call(
            "DOM.resolveNode",
            json!({ "backendNodeId": backend, "executionContextId": context, "objectGroup": GROUP }),
            on,
        )
        .await?;
    let element = resolved
        .pointer("/object/objectId")
        .and_then(Value::as_str)
        .ok_or_else(gone)?;
    let answer = cdp
        .call(
            "Runtime.callFunctionOn",
            json!({
                "objectId": element,
                "functionDeclaration": DESCRIBE,
                "returnByValue": true,
                "silent": true,
            }),
            on,
        )
        .await?;
    described(&answer)
}

/// The target and the text of `DESCRIBE`'s answer.
///
/// # Errors
///
/// The page threw, or the agent has no selector for the element.
pub fn described(answer: &Value) -> Result<(Value, String), EngineError> {
    let none = || EngineError::Invalid("the sync agent has no selector for the element".to_owned());
    if answer.get("exceptionDetails").is_some() {
        return Err(none());
    }
    let value = answer.pointer("/result/value").ok_or_else(none)?;
    let target = value
        .get("target")
        .filter(|target| target.is_object())
        .cloned()
        .ok_or_else(none)?;
    let text = value
        .get("text")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    Ok((target, text))
}

#[cfg(test)]
#[path = "check_tests.rs"]
mod tests;
