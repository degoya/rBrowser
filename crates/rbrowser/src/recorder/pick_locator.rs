//! The element a person picks in a view, or the one at a point, named as a recorded click names
//! it (RB-090-60, RB-092-01): the element under the pointer is resolved as "Inspect element"
//! resolves it, past rBrowser's own layers ([`crate::devtools::inspect::node_at`]), or picked
//! with the capture's picker ([`crate::capture::pick`]); the sync agent names it (`targetOf`,
//! the selector candidates of a recorded click), and every other view is asked whether its page
//! has that element. The page menu's check steps ([`super::check`]) and the recording editor's
//! "Pick in view" (`recorder.pick_element`) both take it from here.
//!
//! - `recorder.pick_element { recordingId, view?, x?, y?, dip? }`: sends the Recorder panel
//!   `recorderElementPicked` ([`answer_of`]) - the locator as a recording holds it, the element's visible text (at most
//!   [`super::check::MAX_TEXT`] characters, never a field's value), the breakpoints whose views
//!   have the element (empty: all) and a `<select>`'s options (at most [`MAX_OPTIONS`]).

use std::sync::Arc;

use rb_commands::CommandError;
use rb_engine::{EngineError, ViewHost as _, ViewId};
use rb_ipc::{CoreToShell, PickedElement};
use rb_sync::ElementTarget;
use rb_sync::recorder::Locator;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::capture::pick::{PICK_TIMEOUT, pick};
use crate::cdp::CdpRouter;
use crate::control::Core;
use crate::devtools::inspect::{Point, node_at};

/// `recorder.pick_element`: the locator of the element picked in a view (RB-092-01).
pub const PICK_ELEMENT: &str = "recorder.pick_element";
/// The most options of a `<select>` the answer names.
pub const MAX_OPTIONS: usize = 200;

/// The object group of the element described, released after it.
const GROUP: &str = "rbrowser-recorder-check";
/// In the agent's world: the target of the element `this`, its visible text and, for a
/// `<select>`, its options - never a field's value.
const DESCRIBE: &str = "function () {
  var text = (this.innerText || this.textContent || '').replace(/\\s+/g, ' ').trim();
  var options = this.tagName === 'SELECT'
    ? Array.prototype.slice.call(this.options, 0, 200).map(function (option) {
        return { value: option.value, label: (option.label || option.text || '').trim() };
      })
    : [];
  return { target: globalThis.__rbAgent.targetOf(this), text: text, options: options };
}";

/// An option of a picked `<select>`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectOption {
    /// Its value.
    pub value: String,
    /// What it shows.
    pub label: String,
}

/// The element picked: the view it was picked in, the agent's target, its text, the views
/// - the source first - whose page has it, and a `<select>`'s options.
#[derive(Clone, Debug)]
pub struct Picked {
    pub source: ViewId,
    pub target: ElementTarget,
    pub text: String,
    pub views: Vec<ViewId>,
    pub options: Vec<SelectOption>,
}

/// The element at `point` (in DIP of the view's widget when its flag is set) of `view` - the
/// active one when `None` - or, without a point, the one the next click picks. Blocks on the
/// core's runtime (a pick waits for a click): never on the UI thread.
///
/// # Errors
///
/// No view, a view without CDP or sync agent, the pick timed out, the agent has no selector.
pub fn pick_element(
    core: &Core,
    view: Option<ViewId>,
    point: Option<(Point, bool)>,
) -> Result<Picked, CommandError> {
    let failed = CommandError::Failed;
    let host = core.host();
    let source = view
        .or_else(|| host.active_view())
        .ok_or_else(|| failed("there is no view".to_owned()))?;
    let page = host
        .capture_target(source)
        .map(|target| target.session)
        .ok_or_else(|| {
            failed(format!(
                "view {source} has no CDP target: only Chromium picks"
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
    if point.is_none() {
        // The pick waits for a click: say so, as a capture's pick does (RB-090-84).
        core.notify(super::check::pick_notice());
    }
    let (target, text, options, views) = crate::analyzer::plugins::block_on(async move {
        let backend = match point {
            Some((point, dip)) => {
                node_at(&cdp, &page, point, if dip { scale } else { 1.0 }).await?
            }
            None => pick(&cdp, &page, PICK_TIMEOUT).await?,
        };
        let (answer, text) = describe(&cdp, &agent, context, backend).await?;
        let target = answer.get("target").cloned().unwrap_or(Value::Null);
        let views = views_with(&cdp, source, &others, &target).await;
        Ok::<_, EngineError>((target, text, options_of(&answer), views))
    })
    .map_err(|error| failed(error.to_string()))?
    .map_err(|error| failed(error.to_string()))?;
    let target: ElementTarget = serde_json::from_value(target)
        .map_err(|_| failed("the sync agent has no selector for the element".to_owned()))?;
    Ok(Picked {
        source,
        target,
        text,
        views,
        options,
    })
}

/// The breakpoints of `views` among `all` (view id, breakpoint id); empty when they are all.
#[must_use]
pub fn only_in(views: &[ViewId], all: &[(ViewId, String)]) -> Vec<String> {
    if all.iter().all(|(view, _)| views.contains(view)) {
        return Vec::new();
    }
    all.iter()
        .filter(|(view, _)| views.contains(view))
        .map(|(_, breakpoint)| breakpoint.clone())
        .collect()
}

/// `recorder.pick_element`'s arguments; `recordingId` says the Recorder panel asks.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PickArgs {
    #[serde(default)]
    recording_id: Option<String>,
    view: Option<u32>,
    x: Option<f64>,
    y: Option<f64>,
    dip: Option<bool>,
}

/// `recorder.pick_element`. Runs on a command thread of the core's runtime. From the Recorder
/// panel (`recordingId`) the element goes back to it as `recorderElementPicked`; a shortcut's
/// call goes to the panel, which picks for the step it adds or edits.
///
/// # Errors
///
/// Bad arguments, and [`pick_element`]'s.
pub fn run(core: &Core, args: &Value) -> Result<Value, CommandError> {
    if super::steps::forward(core, PICK_ELEMENT, args) {
        return Ok(json!({ "forwarded": true }));
    }
    let raw = pick_args(args)?;
    let point = point_of(raw.x, raw.y, raw.dip)?;
    let picked = pick_element(core, raw.view.map(ViewId), point)?;
    let answer = answer_of(&picked, &breakpoints_of(core));
    core.send(&CoreToShell::RecorderElementPicked(answer.clone()));
    serde_json::to_value(answer).map_err(|error| CommandError::Failed(error.to_string()))
}

/// `recorder.pick_element`'s arguments, read: the recording the panel picks for has a valid name.
///
/// # Errors
///
/// An unknown field, a wrong type, a name no recording can have.
fn pick_args(args: &Value) -> Result<PickArgs, CommandError> {
    let invalid = |error: String| CommandError::InvalidArgs(error);
    let raw: PickArgs =
        serde_json::from_value(args.clone()).map_err(|error| invalid(error.to_string()))?;
    if let Some(name) = &raw.recording_id {
        rb_sync::recorder::check_name(name).map_err(|error| invalid(error.to_string()))?;
    }
    Ok(raw)
}

/// The panel's answer: the locator, the text cut to [`super::check::MAX_TEXT`], `onlyIn` from
/// the views that have the element among `all`, and the options.
#[must_use]
pub fn answer_of(picked: &Picked, all: &[(ViewId, String)]) -> PickedElement {
    PickedElement {
        view: picked.source.0,
        locator: serde_json::to_value(Locator::from_target(&picked.target, None))
            .unwrap_or(Value::Null),
        text: picked.text.chars().take(super::check::MAX_TEXT).collect(),
        only_in: only_in(&picked.views, all),
        options: picked
            .options
            .iter()
            .filter_map(|option| serde_json::to_value(option).ok())
            .collect(),
    }
}

/// `x` and `y` together (a point) or neither (a pick).
///
/// # Errors
///
/// A lone, negative or not finite coordinate.
pub fn point_of(
    x: Option<f64>,
    y: Option<f64>,
    dip: Option<bool>,
) -> Result<Option<(Point, bool)>, CommandError> {
    match (x, y) {
        (Some(x), Some(y)) if x.is_finite() && y.is_finite() && x >= 0.0 && y >= 0.0 => {
            Ok(Some((Point { x, y }, dip.unwrap_or(false))))
        }
        (None, None) => Ok(None),
        _ => Err(CommandError::InvalidArgs(
            "`x` and `y` are non-negative numbers and come together, or neither".to_owned(),
        )),
    }
}

/// Every view with its breakpoint id as a recording names it (the size of a free-size view).
#[must_use]
pub fn breakpoints_of(core: &Core) -> Vec<(ViewId, String)> {
    core.host()
        .views()
        .into_iter()
        .map(|info| {
            let breakpoint = info
                .breakpoint
                .clone()
                .unwrap_or_else(|| format!("{}x{}", info.width, info.height));
            (info.id, breakpoint)
        })
        .collect()
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

/// The agent's answer for the element `backend` - its target, text and options - and its text,
/// in the agent's world `context`.
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
        tracing::debug!(%error, "recorder pick: the object group stayed");
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
    let (target, text) = described(&answer)?;
    let options = answer
        .pointer("/result/value/options")
        .cloned()
        .unwrap_or(Value::Null);
    Ok((json!({ "target": target, "options": options }), text))
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

/// A `<select>`'s options from the described element, at most [`MAX_OPTIONS`]; none for any
/// other element or an answer without them.
#[must_use]
pub fn options_of(described: &Value) -> Vec<SelectOption> {
    described
        .get("options")
        .and_then(|options| serde_json::from_value::<Vec<SelectOption>>(options.clone()).ok())
        .map(|mut options| {
            options.truncate(MAX_OPTIONS);
            options
        })
        .unwrap_or_default()
}

#[cfg(test)]
#[path = "pick_locator_tests.rs"]
mod tests;
