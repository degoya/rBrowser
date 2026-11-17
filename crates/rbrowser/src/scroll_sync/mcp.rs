//! MCP's `scroll` (RB-030-01 registers it): the arguments, the answer and the function behind
//! it. It builds the scroll the sync agent would have captured and applies it with the same
//! agent command as the sync ([`super::replay::apply`]), side by side in the views of `target`
//! (the active view when absent). The command sets the replay marker, so the active view's agent
//! does not capture the scroll and the sync does not carry it further. It always sets the
//! position, whatever `sync.scrollMode` says: the wheel mode replays a user's wheel.

use std::sync::Arc;
use std::time::Duration;

use futures_util::future::join_all;
use rb_commands::CommandError;
use rb_engine::{Target, ViewId};
use rb_sync::{ElementSelector, ElementTarget, Offset, ReplayOutcome, ScrollData};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::replay::apply;
use crate::host::Host;
use crate::replay::mcp::{InputAnswer, InputStatus, ViewInput};
use crate::replay::{HostPort, ReplayPort};

/// `scroll`'s name.
pub const SCROLL_TOOL: &str = "scroll";
/// `scroll`'s description.
pub const SCROLL_DESCRIPTION: &str = "Scroll the views of `target` (\"all\" or a view id; the \
     active view when absent) the way scroll sync does, so every view shows the same content: \
     `to` \"top\" or \"bottom\" (exactly), `element` (CSS selector) brought to the top edge, or \
     `percentY` / `percentX` (0 to 100) of the scroll range; the other axis goes to its start. \
     `container` (CSS selector) scrolls that element instead of the page. Answers per view \
     `done`, `notFound` (the container or the element is missing), `notVisible` or a failure.";

/// The longest selector taken.
const MAX_SELECTOR: usize = 2048;
/// The scroll range the request is expressed in: the agent's ends (within 1 of 0 and of the
/// range) are then 0.01 % of it.
const RANGE: f64 = 10_000.0;

/// An end of the scroll range.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScrollEdge {
    /// The start: `scrollTop` 0.
    Top,
    /// The end: the largest `scrollTop`.
    Bottom,
}

/// The arguments of `scroll`: one destination of `to`, `element` or the percentages.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScrollArgs {
    /// `"all"` or a view id; the active view when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<Target>,
    /// The scroll container (CSS selector); the page when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub container: Option<String>,
    /// The start or the end.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<ScrollEdge>,
    /// The element to bring to the top edge (CSS selector).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub element: Option<String>,
    /// Horizontal share of the scroll range, 0 to 100.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub percent_x: Option<f64>,
    /// Vertical share of the scroll range, 0 to 100.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub percent_y: Option<f64>,
}

fn invalid(message: impl Into<String>) -> CommandError {
    CommandError::InvalidArgs(message.into())
}

/// The target of the CSS selector `selector` at its top-left corner (`offset`), checked.
fn css(selector: &str, name: &str, offset: Offset) -> Result<ElementTarget, CommandError> {
    let selector = selector.trim();
    if selector.is_empty() || selector.len() > MAX_SELECTOR {
        return Err(invalid(format!(
            "`{name}` must have 1 to {MAX_SELECTOR} bytes"
        )));
    }
    Ok(ElementTarget {
        hosts: Vec::new(),
        element: ElementSelector::Css {
            path: selector.to_owned(),
        },
        offset,
    })
}

fn percent(value: Option<f64>, name: &str) -> Result<f64, CommandError> {
    match value {
        None => Ok(0.0),
        Some(value) if value.is_finite() && (0.0..=100.0).contains(&value) => Ok(value),
        Some(_) => Err(invalid(format!("`{name}` must be 0 to 100"))),
    }
}

/// The scroll `args` ask for, as a leader's scroll event would carry it, and the element that
/// must be found first (`element`).
pub fn scroll_request(
    args: &ScrollArgs,
) -> Result<(ScrollData, Option<ElementTarget>), CommandError> {
    let percentages = args.percent_x.is_some() || args.percent_y.is_some();
    let destinations = [args.to.is_some(), args.element.is_some(), percentages];
    if destinations.iter().filter(|given| **given).count() != 1 {
        return Err(invalid(
            "give exactly one of `to`, `element` or `percentX`/`percentY`",
        ));
    }
    let container = match &args.container {
        Some(selector) => Some(css(selector, "container", Offset { x: 0.5, y: 0.5 })?),
        None => None,
    };
    let at = |x: f64, y: f64| ScrollData {
        container: container.clone(),
        anchor: None,
        x,
        y,
        max_x: RANGE,
        max_y: RANGE,
    };
    if let Some(edge) = args.to {
        let y = match edge {
            ScrollEdge::Top => 0.0,
            ScrollEdge::Bottom => RANGE,
        };
        return Ok((at(0.0, y), None));
    }
    if let Some(selector) = &args.element {
        let anchor = css(selector, "element", Offset { x: 0.0, y: 0.0 })?;
        // Inside the range on both axes, so the follower takes the anchor, not an end.
        let mut data = at(RANGE / 2.0, RANGE / 2.0);
        data.anchor = Some(anchor.clone());
        return Ok((data, Some(anchor)));
    }
    let x = percent(args.percent_x, "percentX")?;
    let y = percent(args.percent_y, "percentY")?;
    Ok((at(x / 100.0 * RANGE, y / 100.0 * RANGE), None))
}

fn view_input(view: ViewId, outcome: ReplayOutcome) -> ViewInput {
    let (status, message) = match outcome {
        ReplayOutcome::Done => (InputStatus::Done, None),
        ReplayOutcome::NotFound => (InputStatus::NotFound, None),
        ReplayOutcome::NotVisible => (InputStatus::NotVisible, None),
        ReplayOutcome::Timeout => (InputStatus::Timeout, None),
        ReplayOutcome::Unsupported => (InputStatus::Unsupported, None),
        ReplayOutcome::Failed(message) => (InputStatus::Failed, Some(message)),
    };
    ViewInput {
        view,
        status,
        message,
    }
}

/// Applies `data` in every view of `views`, side by side, each within `timeout`.
pub async fn scroll_views(
    port: &dyn ReplayPort,
    views: &[ViewId],
    data: &ScrollData,
    require: Option<&ElementTarget>,
    timeout: Duration,
) -> InputAnswer {
    let runs = views.iter().map(|view| async move {
        let outcome = match tokio::time::timeout(timeout, apply(port, *view, data, require)).await {
            Ok(Ok(())) => ReplayOutcome::Done,
            Ok(Err(outcome)) => outcome,
            Err(_) => ReplayOutcome::Timeout,
        };
        view_input(*view, outcome)
    });
    InputAnswer {
        views: join_all(runs).await,
    }
}

/// Runs `scroll` with `args` (its JSON).
pub async fn scroll(host: &Arc<Host>, args: &Value) -> Result<InputAnswer, CommandError> {
    let args: ScrollArgs = serde_json::from_value(args.clone()).map_err(|error| {
        let message = error.to_string();
        if message.starts_with("unknown field") || message.starts_with("missing field") {
            invalid(message)
        } else {
            invalid("an argument has the wrong type or value")
        }
    })?;
    let (data, require) = scroll_request(&args)?;
    let views = host
        .target_views(args.target)
        .map_err(|error| invalid(error.to_string()))?;
    let timeout = Duration::from_millis(u64::from(host.config().sync.desync_timeout_ms));
    let port = HostPort::new(host);
    Ok(scroll_views(&port, &views, &data, require.as_ref(), timeout).await)
}

#[cfg(test)]
#[path = "mcp_tests.rs"]
mod tests;
