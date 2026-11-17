//! The commands (RB-090-53): `view.force_state.set` forces states on an element - at a point of
//! a view (`view`, `x`, `y`, `dip`: the page menu's click, "Inspect element"'s resolver), the
//! one a click picks (neither), or a CSS selector's first match in each view (`selector`) - in
//! the views of `target` (the active view without one), the same element in each found by the
//! sync agent's selectors; `view.force_state.reset` takes every state of `target` (all views
//! without one) back. The status bar says while states are forced, and names the views without
//! the element.

use std::collections::BTreeSet;
use std::sync::Arc;

use rb_commands::CommandError;
use rb_engine::{EngineError, Target, ViewHost as _, ViewId};
use rb_ipc::{CoreToShell, Notification, NotificationLevel};
use serde::Deserialize;
use serde_json::{Value, json};

use super::apply::{self, Element, Outcome, Request, Source, ViewPage};
use super::model::PseudoClass;
use crate::capture::pick::{PICK_TIMEOUT, pick};
use crate::cdp::{CdpRouter, lock};
use crate::control::Core;
use crate::devtools::inspect::{Point, node_at};

/// The longest selector taken, in characters.
pub const MAX_SELECTOR_CHARS: usize = 512;

/// The status bar while states are forced; `{views}` names them.
pub const FORCED_KEY: &str = "devtools.force_state.notice.forced";
/// The status bar after the last forced state went.
pub const CLEARED_KEY: &str = "devtools.force_state.notice.cleared";
/// Views without the element; `{views}`.
pub const MISSING_KEY: &str = "devtools.force_state.notice.missing";
/// Views where forcing failed; `{views}`, `{error}`.
pub const FAILED_KEY: &str = "devtools.force_state.notice.failed";

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    target: Option<Target>,
    view: Option<u32>,
    x: Option<f64>,
    y: Option<f64>,
    dip: Option<bool>,
    selector: Option<String>,
    states: Option<Vec<String>>,
    toggle: Option<String>,
    clear: Option<bool>,
}

/// Which element.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Pointed {
    /// At a point of the source view: CSS px of its page, or DIP of its widget with `dip`.
    At { point: Point, dip: bool },
    /// The one the user clicks in the source view.
    Pick,
    /// The first match in each view.
    Selector(String),
}

/// `view.force_state.set`'s arguments, read.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Args {
    pub target: Option<Target>,
    pub view: Option<ViewId>,
    /// `None`: `clear` without an element - every state of the target's views goes.
    pub element: Option<Pointed>,
    pub request: Request,
}

fn invalid(text: impl Into<String>) -> CommandError {
    CommandError::InvalidArgs(text.into())
}

fn class_of(text: &str) -> Result<PseudoClass, CommandError> {
    PseudoClass::parse(text).ok_or_else(|| {
        let known: Vec<&str> = PseudoClass::ALL.iter().map(|class| class.name()).collect();
        invalid(format!(
            "unknown state `{text}`; one of {}",
            known.join(", ")
        ))
    })
}

/// Reads `view.force_state.set`'s arguments: one of `states` (exactly these; `[]` takes the
/// element's back), `toggle` (one, by what the element has; needs a point or a pick) or
/// `clear: true`, `:hover` without any; a point (`x` and `y`) or a `selector`, a pick without.
pub(super) fn parse(args: &Value) -> Result<Args, CommandError> {
    let raw: Raw = if args.is_null() {
        Raw::default()
    } else {
        serde_json::from_value(args.clone()).map_err(|error| invalid(error.to_string()))?
    };
    let clear = raw.clear == Some(true);
    let given =
        usize::from(raw.states.is_some()) + usize::from(raw.toggle.is_some()) + usize::from(clear);
    if given > 1 {
        return Err(invalid("give one of `states`, `toggle` or `clear`"));
    }
    let request = match (&raw.states, &raw.toggle) {
        (Some(states), _) => Request::Set(
            states
                .iter()
                .map(String::as_str)
                .map(class_of)
                .collect::<Result<BTreeSet<_>, _>>()?,
        ),
        (_, Some(toggle)) => Request::Toggle(class_of(toggle)?),
        _ if clear => Request::Set(BTreeSet::new()),
        _ => Request::Set([PseudoClass::Hover].into_iter().collect()),
    };
    let coordinate = |value: Option<f64>, name: &str| match value {
        Some(value) if value.is_finite() && value >= 0.0 => Ok(Some(value)),
        Some(_) => Err(invalid(format!("`{name}` is a non-negative number"))),
        None => Ok(None),
    };
    let point = match (coordinate(raw.x, "x")?, coordinate(raw.y, "y")?) {
        (Some(x), Some(y)) => Some(Point { x, y }),
        (None, None) => None,
        _ => return Err(invalid("`x` and `y` come together, or neither")),
    };
    let element = match (point, raw.selector) {
        (Some(_), Some(_)) => return Err(invalid("a point or a `selector`, not both")),
        (Some(point), None) => Some(Pointed::At {
            point,
            dip: raw.dip.unwrap_or(false),
        }),
        (None, Some(selector)) => {
            let selector = selector.trim();
            if selector.is_empty()
                || selector.chars().count() > MAX_SELECTOR_CHARS
                || selector.chars().any(char::is_control)
            {
                return Err(invalid(format!(
                    "the selector is empty, longer than {MAX_SELECTOR_CHARS} characters or holds control characters"
                )));
            }
            if matches!(request, Request::Toggle(_)) {
                return Err(invalid(
                    "`toggle` decides by the element at a point; with a selector give `states`",
                ));
            }
            Some(Pointed::Selector(selector.to_owned()))
        }
        (None, None) if clear => None,
        (None, None) => Some(Pointed::Pick),
    };
    Ok(Args {
        target: raw.target,
        view: raw.view.map(ViewId),
        element,
        request,
    })
}

/// `view.force_state.reset`'s target: every view without one.
pub(super) fn parse_reset(args: &Value) -> Result<Target, CommandError> {
    #[derive(Debug, Default, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct ResetArgs {
        target: Option<Target>,
    }
    let raw: ResetArgs = if args.is_null() {
        ResetArgs::default()
    } else {
        serde_json::from_value(args.clone()).map_err(|error| invalid(error.to_string()))?
    };
    Ok(raw.target.unwrap_or(Target::All))
}

/// `view.force_state.set`. Runs on a command thread of the core's runtime, never on the UI
/// thread: the resolution blocks on that runtime (a pick waits for a click).
pub fn run(core: &Core, args: &Value) -> Result<Value, CommandError> {
    let args = parse(args)?;
    let host = core.host();
    let targets = host.target_views(args.target).map_err(failed)?;
    let Some(pointed) = args.element else {
        return reset_views(core, &targets);
    };
    super::watch::ensure(core);
    let source = args
        .view
        .or(match targets.as_slice() {
            [only] => Some(*only),
            _ => None,
        })
        .or_else(|| host.active_view())
        .or_else(|| targets.first().copied())
        .ok_or_else(|| CommandError::Failed("there is no view".to_owned()))?;
    let pages: Vec<ViewPage> = targets.iter().map(|view| page_of(core, *view)).collect();
    let source_page = page_of(core, source);
    let scale = host.view(source).map_or(1.0, |info| info.scale);
    let cdp = host.cdp().clone();
    let request = args.request;
    let outcomes = crate::analyzer::plugins::block_on(async move {
        let _one_at_a_time = super::OPERATION.lock().await;
        let element = element_of(&cdp, &source_page, pointed, scale).await?;
        Ok::<_, EngineError>(
            apply::force(&cdp, super::registry(), &pages, &element, &request).await,
        )
    })
    .map_err(|error| CommandError::Failed(error.to_string()))?
    .map_err(failed)?;
    report(core, &outcomes);
    Ok(answer(&outcomes))
}

/// The element `pointed` names: the source view's node at the point or picked, else the
/// selector.
async fn element_of(
    cdp: &Arc<CdpRouter>,
    source: &ViewPage,
    pointed: Pointed,
    scale: f64,
) -> Result<Element, EngineError> {
    let at = match pointed {
        Pointed::Selector(selector) => return Ok(Element::Selector(selector)),
        other => other,
    };
    let page = source.page.as_deref().ok_or_else(|| {
        EngineError::Invalid(format!(
            "view {} has no CDP target yet: only Chromium forces states",
            source.view
        ))
    })?;
    let backend = match at {
        Pointed::At { point, dip } => {
            node_at(cdp, page, point, if dip { scale } else { 1.0 }).await?
        }
        Pointed::Pick | Pointed::Selector(_) => pick(cdp, page, PICK_TIMEOUT).await?,
    };
    Ok(Element::From(Source {
        view: source.view,
        backend,
        agent: source.agent.clone(),
    }))
}

/// `view.force_state.reset`.
pub fn reset(core: &Core, args: &Value) -> Result<Value, CommandError> {
    let target = parse_reset(args)?;
    let views = core.host().target_views(Some(target)).map_err(failed)?;
    reset_views(core, &views)
}

fn reset_views(core: &Core, views: &[ViewId]) -> Result<Value, CommandError> {
    let cdp = core.host().cdp().clone();
    let views = views.to_vec();
    let reset = crate::analyzer::plugins::block_on(async move {
        let _one_at_a_time = super::OPERATION.lock().await;
        apply::reset(&cdp, super::registry(), &views).await
    })
    .map_err(|error| CommandError::Failed(error.to_string()))?;
    let forced = lock(super::registry()).forced_views();
    if !reset.is_empty() {
        core.notify(status(core, &forced));
    }
    Ok(json!({ "reset": reset, "forced": forced }))
}

fn page_of(core: &Core, view: ViewId) -> ViewPage {
    let host = core.host();
    ViewPage {
        view,
        page: host.capture_target(view).map(|target| target.session),
        agent: host.sync_agents().top_context(view).ok(),
    }
}

/// `{ views: [{ view, states } | { view, missing: true } | { view, error }], forced }`.
pub(super) fn answer(outcomes: &[(ViewId, Outcome)]) -> Value {
    let views: Vec<Value> = outcomes
        .iter()
        .map(|(view, outcome)| match outcome {
            Outcome::Forced(classes) => json!({
                "view": view,
                "states": classes.iter().map(|class| class.name()).collect::<Vec<_>>(),
            }),
            Outcome::Missing => json!({ "view": view, "missing": true }),
            Outcome::Failed(error) => json!({ "view": view, "error": error }),
        })
        .collect();
    json!({ "views": views, "forced": lock(super::registry()).forced_views() })
}

/// The status bar: the views without the element, the failures, and what stays forced.
fn report(core: &Core, outcomes: &[(ViewId, Outcome)]) {
    let missing: Vec<ViewId> = outcomes
        .iter()
        .filter(|(_, outcome)| *outcome == Outcome::Missing)
        .map(|(view, _)| *view)
        .collect();
    if !missing.is_empty() {
        let names = names_of(core, &missing);
        core.notify(notice(
            NotificationLevel::Warning,
            MISSING_KEY,
            &[("views", names)],
        ));
    }
    let failed: Vec<(ViewId, &String)> = outcomes
        .iter()
        .filter_map(|(view, outcome)| match outcome {
            Outcome::Failed(error) => Some((*view, error)),
            _ => None,
        })
        .collect();
    if let Some((_, error)) = failed.first() {
        let views: Vec<ViewId> = failed.iter().map(|(view, _)| *view).collect();
        tracing::warn!(?views, %error, "force state failed");
        core.notify(notice(
            NotificationLevel::Warning,
            FAILED_KEY,
            &[
                ("views", names_of(core, &views)),
                ("error", (*error).clone()),
            ],
        ));
    }
    let forced = lock(super::registry()).forced_views();
    core.notify(status(core, &forced));
}

/// What the status bar says about `forced`, the views with forced states.
pub(super) fn status(core: &Core, forced: &[ViewId]) -> CoreToShell {
    if forced.is_empty() {
        notice(NotificationLevel::Info, CLEARED_KEY, &[])
    } else {
        notice(
            NotificationLevel::Warning,
            FORCED_KEY,
            &[("views", names_of(core, forced))],
        )
    }
}

/// The views' labels, comma-separated (the id for a view that is gone).
pub(super) fn names_of(core: &Core, views: &[ViewId]) -> String {
    let host = core.host();
    views
        .iter()
        .map(|view| {
            host.view(*view)
                .map_or_else(|| view.to_string(), |info| info.label)
        })
        .collect::<Vec<_>>()
        .join(", ")
}

pub(super) fn notice(level: NotificationLevel, key: &str, args: &[(&str, String)]) -> CoreToShell {
    CoreToShell::Notification(Notification {
        level,
        message_key: key.to_owned(),
        args: args
            .iter()
            .map(|(name, value)| ((*name).to_owned(), value.clone()))
            .collect(),
        settings_key: None,
    })
}

fn failed(error: EngineError) -> CommandError {
    match error {
        EngineError::UnknownView(_) | EngineError::Invalid(_) => {
            CommandError::InvalidArgs(error.to_string())
        }
        other => CommandError::Failed(other.to_string()),
    }
}

#[cfg(test)]
#[path = "command_tests.rs"]
mod tests;
