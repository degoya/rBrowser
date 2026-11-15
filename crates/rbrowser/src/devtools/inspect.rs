//! "Inspect element" (RB-080-43): `devtools.inspect_at { view, x, y, dip? }` opens DevTools
//! for the view and selects the node under the point in the Elements panel; without a point it
//! waits for a click in the view (the inspect overlay, as `capture.element`'s pick). `x` and
//! `y` are CSS px of the page (MCP); with `dip: true` they are DIP of the view's widget (the
//! context menu's click), divided by the cell's scale first. The node is resolved on the page's
//! session with `DOM.getNodeForLocation` before DevTools open, so a failed resolution opens
//! nothing; the docked frontend then gets `Overlay.inspectNodeRequested` through its relay
//! ([`InspectQueue`]), once it has enabled the overlay domain - what Chromium's own overlay
//! agent does for its "Inspect" entry. DevTools in a window (CEF's own frontend) take no
//! selection: the window opens, nothing is resolved.
//!
//! `devtools.openOnInspect` (RB-080-45) is the person's preference for the menu and the
//! shortcut: off, inspect neither opens DevTools nor moves them from another view - it selects
//! only where they are open for the view, and says in the status bar (linked to the setting)
//! when they are not. `open: true|false` in the arguments overrides the setting; MCP's
//! `devtools_inspect_at` sends `true`, an agent's call always opens.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use rb_commands::CommandError;
use rb_engine::{EngineError, ViewHost, ViewId};
use serde_json::{Value, json};
use tokio::sync::Notify;

use crate::capture::pick::{PICK_TIMEOUT, pick};
use crate::cdp::{CdpRouter, lock};
use crate::control::Core;

/// The status bar's text when inspect may not open DevTools and they are not open for the view.
pub const NOT_OPEN_KEY: &str = "devtools.notice.inspectNotOpen";
/// The setting the notice links.
pub const OPEN_SETTING: &str = "devtools.openOnInspect";

/// The command.
pub const COMMAND: &str = "devtools.inspect_at";

/// The nodes waiting for a docked frontend, by view: the newest request per view. A frontend
/// connection takes its view's once the frontend enabled the overlay.
#[derive(Default)]
pub struct InspectQueue {
    pending: Mutex<HashMap<ViewId, i64>>,
    changed: Notify,
}

impl InspectQueue {
    /// Asks the frontend of `view` to reveal `backend_node_id`; a request still waiting for
    /// that view is replaced.
    pub fn request(&self, view: ViewId, backend_node_id: i64) {
        lock(&self.pending).insert(view, backend_node_id);
        self.changed.notify_one();
    }

    /// Takes the request waiting for `view`.
    pub fn take(&self, view: ViewId) -> Option<i64> {
        lock(&self.pending).remove(&view)
    }

    /// The request waiting for `view`, left in place: a connection peeks, delivers, then
    /// [`Self::take_if`]s it.
    pub fn pending(&self, view: ViewId) -> Option<i64> {
        lock(&self.pending).get(&view).copied()
    }

    /// Takes `view`'s request when it is still `backend_node_id` (delivered); `false` when it
    /// was replaced or taken meanwhile - the newer request stays for the next delivery.
    pub fn take_if(&self, view: ViewId, backend_node_id: i64) -> bool {
        let mut pending = lock(&self.pending);
        if pending.get(&view) == Some(&backend_node_id) {
            pending.remove(&view);
            true
        } else {
            false
        }
    }

    /// Forgets `view`'s request (its DevTools closed).
    pub fn clear(&self, view: ViewId) {
        lock(&self.pending).remove(&view);
    }

    /// Resolves after the next [`Self::request`] (or at once, when one came since the last
    /// wait).
    pub async fn changed(&self) {
        self.changed.notified().await;
    }
}

/// A point of a view: CSS px of the page, or DIP of the view's widget, which shows the page at
/// `scale` (the cell's fit) - then the page's CSS px are the DIP divided by it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    /// The point in the page's CSS px for a view shown at `scale` (1 when the scale is not
    /// usable).
    #[must_use]
    pub fn in_page(self, scale: f64) -> (i64, i64) {
        let scale = if scale.is_finite() && scale > 0.0 {
            scale
        } else {
            1.0
        };
        (
            (self.x / scale).round() as i64,
            (self.y / scale).round() as i64,
        )
    }
}

/// The backend node id of the element at `point` of the page on `session`.
pub async fn node_at(
    cdp: &Arc<CdpRouter>,
    session: &str,
    point: Point,
    scale: f64,
) -> Result<i64, EngineError> {
    let (x, y) = point.in_page(scale);
    let answer = cdp
        .call(
            "DOM.getNodeForLocation",
            json!({ "x": x, "y": y, "includeUserAgentShadowDOM": false }),
            Some(session),
        )
        .await?;
    answer
        .get("backendNodeId")
        .and_then(Value::as_i64)
        .ok_or_else(|| EngineError::Invalid(format!("no element at {x},{y}")))
}

/// The arguments of the command: the view (`view` or `target`, a view id; the active view
/// without either), the point - both coordinates or none - and whether it is in DIP of the
/// widget (`dip`, default: CSS px of the page), and `open` when the caller decides instead of
/// `devtools.openOnInspect`.
fn parse(
    core: &Core,
    args: &Value,
) -> Result<(ViewId, Option<Point>, bool, Option<bool>), CommandError> {
    let view = match args.get("view").or_else(|| args.get("target")) {
        None | Some(Value::Null) => {
            let host = core.host();
            host.active_view()
                .or_else(|| host.views().first().map(|info| info.id))
                .ok_or_else(|| CommandError::Failed("there is no view".to_owned()))?
        }
        Some(value) => value
            .as_u64()
            .and_then(|id| u32::try_from(id).ok())
            .map(ViewId)
            .ok_or_else(|| {
                CommandError::InvalidArgs("`view` is one view id (inspect one view)".to_owned())
            })?,
    };
    let coordinate = |name: &str| -> Result<Option<f64>, CommandError> {
        match args.get(name) {
            None | Some(Value::Null) => Ok(None),
            Some(value) => value
                .as_f64()
                .filter(|value| value.is_finite() && *value >= 0.0)
                .map(Some)
                .ok_or_else(|| {
                    CommandError::InvalidArgs(format!("`{name}` is a non-negative number"))
                }),
        }
    };
    let point = match (coordinate("x")?, coordinate("y")?) {
        (Some(x), Some(y)) => Some(Point { x, y }),
        (None, None) => None,
        _ => {
            return Err(CommandError::InvalidArgs(
                "`x` and `y` come together, or neither (then the view waits for a click)"
                    .to_owned(),
            ));
        }
    };
    let flag = |name: &str| -> Result<Option<bool>, CommandError> {
        match args.get(name) {
            None | Some(Value::Null) => Ok(None),
            Some(Value::Bool(flag)) => Ok(Some(*flag)),
            Some(_) => Err(CommandError::InvalidArgs(format!("`{name}` is a boolean"))),
        }
    };
    let dip = flag("dip")?.unwrap_or(false);
    Ok((view, point, dip, flag("open")?))
}

/// Inspect may not open DevTools and they are not open for `view`: nothing is inspected, and
/// the status bar says so with the setting to change. Not a failure of the command.
fn not_open(core: &Core, view: ViewId) -> Value {
    tracing::info!(%view, "inspect: DevTools are not open for the view and may not be opened");
    core.notify(crate::control::warning_notification(
        NOT_OPEN_KEY,
        Some(OPEN_SETTING),
    ));
    json!({
        "open": false,
        "view": view,
        "backendNodeId": Value::Null,
        "selected": false,
        "reason": OPEN_SETTING,
    })
}

/// `devtools.inspect_at`: resolves the node under the point - or the one the user clicks,
/// without a point - then opens DevTools for the view (where they are open already, or in the
/// settings' mode) and hands the node to the docked frontend; in a window nothing is resolved.
/// Without leave to open (`open`, else `devtools.openOnInspect`) DevTools are neither opened
/// nor moved: the node goes to DevTools open for this view, or [`not_open`] answers.
/// Runs on a command thread of the core's runtime, never on the UI thread: the resolution
/// blocks on that runtime.
pub fn run(core: &Core, args: &Value) -> Result<Value, CommandError> {
    let (view, point, dip, open) = parse(core, args)?;
    let devtools = super::of(core).map_err(super::failed)?;
    let host = core.host();
    let target = host
        .capture_target(view)
        .ok_or_else(|| CommandError::Failed(format!("view {view} has no CDP target yet")))?;
    let may_open = open.unwrap_or_else(|| core.user_settings().devtools.open_on_inspect);
    // The DevTools open for this view, and the mode inspect shows them in: theirs, or the
    // settings' when they are closed or with another view.
    let shown = || {
        let here = devtools.current().filter(|open| open.view == view);
        let mode = here.map_or_else(|| devtools.configured_mode(), |open| open.mode);
        (here.is_some(), mode)
    };
    let (here, mode) = shown();
    if !here && !may_open {
        return Ok(not_open(core, view));
    }
    let node = if super::layout::side_of(mode).is_some() {
        let scale = if dip {
            host.view(view).map_or(1.0, |info| info.scale)
        } else {
            1.0
        };
        let cdp = host.cdp().clone();
        let resolved = crate::analyzer::plugins::block_on(async move {
            match point {
                Some(point) => node_at(&cdp, &target.session, point, scale).await,
                None => pick(&cdp, &target.session, PICK_TIMEOUT).await,
            }
        })
        .map_err(|error| CommandError::Failed(error.to_string()))?;
        Some(resolved.map_err(super::failed)?)
    } else {
        None
    };
    // The resolution took its time (a pick waits for a click): DevTools may have closed,
    // moved to another view or changed their mode meanwhile. State and mode are read again,
    // together, for both paths.
    let (here, mode) = shown();
    if may_open {
        devtools.open(view, mode).map_err(super::failed)?;
    } else if !here {
        return Ok(not_open(core, view));
    }
    let selected = node.is_some_and(|node| devtools.request_inspect(view, node));
    Ok(json!({
        "open": true,
        "view": view,
        "mode": mode,
        "backendNodeId": node,
        "selected": selected,
    }))
}

#[cfg(test)]
#[path = "inspect_tests.rs"]
mod tests;
