//! Which views a run takes: the ones asked for (`--views`, the MCP `target`), else the flow's
//! `// @views` (breakpoint ids or labels), else all. Chromium views with a page run; Firefox views
//! are skipped (Playwright does not drive the installed Firefox reliably over BiDi), and so are
//! WebKit views (their pages live in the WebKit module, which this runner does not reach yet) and
//! Chromium views without a page. A view the layout hides - placed off screen by a layout mode -
//! is skipped too ([`without_hidden`], RB-090-84): the browser throttles its page, so its actions
//! wait in vain; the breakpoints a flow names that no view shows (focus mode) are named in the
//! result ([`missing_breakpoints`]).

use std::collections::BTreeSet;

use rb_engine::{EngineKind, ViewId, ViewInfo};
use serde_json::Value;

use super::FlowError;
use super::result::{ViewResult, ViewStatus};

/// Why a Firefox view is skipped.
pub const FIREFOX_REASON: &str = "Firefox views are not driven by flows: Playwright does not control the installed Firefox reliably over BiDi";
/// Why a WebKit view is skipped.
pub const WEBKIT_REASON: &str = "flows do not reach WebKit views in this build";
/// Why a Chromium view without a page is skipped.
pub const NO_PAGE_REASON: &str = "the view has no page yet";
/// Why a view the layout hides is skipped (`automation.flows.hiddenViews`, RB-090-84).
pub const HIDDEN_REASON: &str = "the view is hidden in the layout (automation.flows.hiddenViews)";

/// A view that runs the flow and its CDP target.
#[derive(Clone, Debug, PartialEq)]
pub struct Runnable {
    /// The view.
    pub view: ViewInfo,
    /// Its page's target id.
    pub target: String,
}

/// The views of a run.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Planned {
    /// The views that run, in grid order.
    pub run: Vec<Runnable>,
    /// The views skipped, with their reason.
    pub skipped: Vec<ViewResult>,
}

/// The engine as results name it.
pub fn engine_name(engine: EngineKind) -> &'static str {
    match engine {
        EngineKind::Chromium => "chromium",
        EngineKind::Firefox => "firefox",
        EngineKind::Webkit => "webkit",
    }
}

/// Plans a run over `views` (grid order); `target` gives a view's page target.
///
/// # Errors
///
/// [`FlowError::NoViews`] when an asked view is unknown or nothing is left.
pub fn plan(
    views: &[ViewInfo],
    asked: Option<&[ViewId]>,
    head_views: &[String],
    target: impl Fn(ViewId) -> Option<String>,
) -> Result<Planned, FlowError> {
    let chosen: Vec<&ViewInfo> = match asked {
        Some(ids) => {
            if let Some(unknown) = ids.iter().find(|id| !views.iter().any(|v| v.id == **id)) {
                return Err(FlowError::NoViews(format!("there is no view {unknown}")));
            }
            views.iter().filter(|view| ids.contains(&view.id)).collect()
        }
        None if !head_views.is_empty() => {
            let matching: Vec<&ViewInfo> = views
                .iter()
                .filter(|view| head_views.iter().any(|name| names(view, name)))
                .collect();
            if matching.is_empty() {
                return Err(FlowError::NoViews(format!(
                    "// @views {} matches no view",
                    head_views.join(",")
                )));
            }
            matching
        }
        None => views.iter().collect(),
    };
    if chosen.is_empty() {
        return Err(FlowError::NoViews("the grid has no view".to_owned()));
    }
    let mut planned = Planned::default();
    for view in chosen {
        let reason = match (view.engine, target(view.id)) {
            (EngineKind::Chromium, Some(target)) => {
                planned.run.push(Runnable {
                    view: view.clone(),
                    target,
                });
                continue;
            }
            (EngineKind::Chromium, None) => NO_PAGE_REASON,
            (EngineKind::Firefox, _) => FIREFOX_REASON,
            (EngineKind::Webkit, _) => WEBKIT_REASON,
        };
        planned.skipped.push(skipped(view, reason));
    }
    Ok(planned)
}

/// `planned` with the views of `hidden` skipped ([`HIDDEN_REASON`]); the run sorts its results
/// into grid order.
#[must_use]
pub fn without_hidden(mut planned: Planned, hidden: &BTreeSet<ViewId>) -> Planned {
    let (gone, kept): (Vec<Runnable>, Vec<Runnable>) = planned
        .run
        .into_iter()
        .partition(|runnable| hidden.contains(&runnable.view.id));
    planned.run = kept;
    planned.skipped.extend(
        gone.iter()
            .map(|runnable| skipped(&runnable.view, HIDDEN_REASON)),
    );
    planned
}

/// Why a run whose views were all `skipped` has nothing to play: every view hidden in the
/// layout, else the reasons the views give.
#[must_use]
pub fn nothing_to_play(skipped: &[ViewResult]) -> String {
    let mut reasons: Vec<&str> = Vec::new();
    for reason in skipped.iter().filter_map(|view| view.reason.as_deref()) {
        if !reasons.contains(&reason) {
            reasons.push(reason);
        }
    }
    if !reasons.is_empty() && reasons.iter().all(|reason| *reason == HIDDEN_REASON) {
        return "every view of the run is hidden in the layout (automation.flows.hiddenViews)"
            .to_owned();
    }
    format!("no view of the run can play: {}", reasons.join("; "))
}

/// The names of `names` (breakpoint ids or labels, as `// @views` or a recording gives them)
/// that no view of `views` shows.
#[must_use]
pub fn missing_breakpoints(views: &[ViewInfo], names: &[String]) -> Vec<String> {
    names
        .iter()
        .filter(|name| !views.iter().any(|view| self::names(view, name)))
        .cloned()
        .collect()
}

/// The views of a command's `views` argument: ids, or breakpoint ids and labels (every view
/// showing one); `None` when absent.
///
/// # Errors
///
/// [`FlowError::NoViews`] for a name no view has.
pub fn resolve_views(views: &[ViewInfo], value: &Value) -> Result<Option<Vec<ViewId>>, FlowError> {
    let Some(items) = value.as_array() else {
        return Ok(None);
    };
    let mut ids = Vec::new();
    for item in items {
        let text = item.as_str().map(str::trim);
        let id = item.as_u64().or_else(|| text.and_then(|t| t.parse().ok()));
        let found: Vec<ViewId> = views
            .iter()
            .filter(|view| match (id, text) {
                (Some(id), _) => u64::from(view.id.0) == id,
                (None, Some(name)) => [view.breakpoint.as_deref(), view.cell_breakpoint.as_deref()]
                    .into_iter()
                    .flatten()
                    .chain([view.label.as_str()])
                    .any(|candidate| candidate.eq_ignore_ascii_case(name)),
                (None, None) => false,
            })
            .map(|view| view.id)
            .collect();
        if found.is_empty() {
            return Err(FlowError::NoViews(format!("there is no view {item}")));
        }
        for id in found {
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
    }
    Ok(Some(ids))
}

/// Whether `name` (from `// @views`) names `view`: its breakpoint, its cell's or its label.
fn names(view: &ViewInfo, name: &str) -> bool {
    let same = |text: &str| text.eq_ignore_ascii_case(name);
    view.breakpoint.as_deref().is_some_and(same)
        || view.cell_breakpoint.as_deref().is_some_and(same)
        || same(&view.label)
}

/// A skipped view's result.
pub fn skipped(view: &ViewInfo, reason: &str) -> ViewResult {
    ViewResult {
        view_id: view.id.0,
        label: view.label.clone(),
        breakpoint: view.breakpoint.clone(),
        engine: engine_name(view.engine).to_owned(),
        status: ViewStatus::Skipped,
        duration_ms: 0,
        steps: Vec::new(),
        error: None,
        screenshot: None,
        reason: Some(reason.to_owned()),
    }
}

#[cfg(test)]
#[path = "plan_tests.rs"]
mod tests;
