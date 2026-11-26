//! The views a test run takes (RB-099-10): checks (`checks.run_all`, `runChecks`), visual
//! regression (`visual.compare`, `visual.baseline_update`), flows (`flows.run`,
//! `flows.run_all`) and plugin calls (Lighthouse) take a [`ViewScope`] - every view, the ones
//! the grid shows, or chosen breakpoints. The breakpoints come from the one resolution,
//! [`rb_config::resolve_scope`]; here they become the views showing them, in grid order, once
//! when the run starts.
//!
//! [`ViewScope::All`] narrows nothing: the run takes every view as it always did. A chosen
//! breakpoint no view shows (hidden in the layout, or not in a cell) is named in
//! [`Scoped::not_shown`], never dropped silently; a choice no view shows at all is refused
//! ([`RunScopeError::NoView`]). Which engines a function can run stays the function's: it names
//! the views it skips with their reason.
//!
//! A plugin gets no scope of its own: the host turns a call's `$scope` into the `breakpoints`
//! the plugin takes ([`plugin_args`]).

use std::collections::BTreeMap;

use rb_config::{ProjectConfig, ScopeError, ViewScope, resolve_scope};
use rb_engine::{ViewId, ViewInfo};
use rb_ipc::{CoreToShell, Notification, NotificationLevel};
use serde_json::{Map, Value};

/// The argument of a command that carries the scope.
pub const SCOPE_ARG: &str = "scope";
/// The argument of a plugin call that carries the scope; the host's, never the plugin's.
pub const PLUGIN_SCOPE_ARG: &str = "$scope";
/// The argument a plugin takes the chosen breakpoints in.
pub const BREAKPOINTS_ARG: &str = "breakpoints";
/// The i18n key of the notice naming the chosen breakpoints no view shows; `{breakpoints}`.
pub const NOT_SHOWN_KEY: &str = "scope.notice.not_shown";
/// The i18n key of a choice no view shows.
pub const NO_VIEW_KEY: &str = "scope.error.no_view";
/// The i18n key of a scope that does not read.
pub const INVALID_KEY: &str = "scope.error.invalid";
/// The i18n key of a plugin call with both a scope and its own breakpoints.
pub const BOTH_KEY: &str = "scope.error.both";

/// The views a scope takes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Scoped {
    /// The views that run, in grid order.
    pub views: Vec<ViewId>,
    /// The breakpoints of the scope, in config order.
    pub breakpoints: Vec<String>,
    /// The breakpoints of the scope no view shows.
    pub not_shown: Vec<String>,
}

/// Why a scope gives no views.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum RunScopeError {
    /// An unknown breakpoint, or none left.
    #[error(transparent)]
    Scope(#[from] ScopeError),
    /// No view shows any of these breakpoints.
    #[error("{key}: no view shows the breakpoints {list}", key = NO_VIEW_KEY, list = .0.join(", "))]
    NoView(Vec<String>),
    /// The scope argument does not read.
    #[error(
        "{key}: the scope is \"all\", \"visible\" or a list of breakpoint ids",
        key = INVALID_KEY
    )]
    Invalid,
    /// A plugin call names both.
    #[error("{key}: give either the scope or the breakpoints, not both", key = BOTH_KEY)]
    Both,
}

impl RunScopeError {
    /// The stable code (an i18n key).
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Scope(error) => error.code(),
            Self::NoView(_) => NO_VIEW_KEY,
            Self::Invalid => INVALID_KEY,
            Self::Both => BOTH_KEY,
        }
    }
}

/// The views of `views` (grid order) that `scope` takes in `config`; `None` for
/// [`ViewScope::All`], which narrows nothing.
///
/// # Errors
///
/// The scope's own ([`ScopeError`]), or no view shows any breakpoint of it.
pub fn scoped(
    config: &ProjectConfig,
    views: &[ViewInfo],
    scope: &ViewScope,
) -> Result<Option<Scoped>, RunScopeError> {
    if *scope == ViewScope::All {
        return Ok(None);
    }
    let breakpoints = resolve_scope(config, scope)?;
    let shows = |view: &ViewInfo, id: &str| {
        view.breakpoint.as_deref() == Some(id) || view.cell_breakpoint.as_deref() == Some(id)
    };
    let chosen: Vec<ViewId> = views
        .iter()
        .filter(|view| breakpoints.iter().any(|id| shows(view, id)))
        .map(|view| view.id)
        .collect();
    let not_shown: Vec<String> = breakpoints
        .iter()
        .filter(|id| !views.iter().any(|view| shows(view, id)))
        .cloned()
        .collect();
    if chosen.is_empty() {
        return Err(RunScopeError::NoView(not_shown));
    }
    Ok(Some(Scoped {
        views: chosen,
        breakpoints,
        not_shown,
    }))
}

/// The notice naming `not_shown`, the chosen breakpoints no view shows - so a run never reads as
/// covering them; `None` when every one has a view.
#[must_use]
pub fn not_shown_notice(not_shown: &[String]) -> Option<CoreToShell> {
    (!not_shown.is_empty()).then(|| {
        CoreToShell::Notification(Notification {
            level: NotificationLevel::Warning,
            message_key: NOT_SHOWN_KEY.to_owned(),
            args: BTreeMap::from([("breakpoints".to_owned(), not_shown.join(", "))]),
            settings_key: None,
        })
    })
}

/// `args` without its [`SCOPE_ARG`], and the scope it named (`null` or absent: none).
///
/// # Errors
///
/// [`RunScopeError::Invalid`] for a scope that is no word or list.
pub fn take_scope(args: &Value) -> Result<(Value, Option<ViewScope>), RunScopeError> {
    let Value::Object(fields) = args else {
        return Ok((args.clone(), None));
    };
    let mut rest: Map<String, Value> = fields.clone();
    let scope = match rest.remove(SCOPE_ARG) {
        None | Some(Value::Null) => None,
        Some(value) => Some(serde_json::from_value(value).map_err(|_| RunScopeError::Invalid)?),
    };
    Ok((Value::Object(rest), scope))
}

/// A plugin call's `args` with its [`PLUGIN_SCOPE_ARG`] resolved in `config` into
/// [`BREAKPOINTS_ARG`]; `"all"` leaves the plugin its own default. The scope stays, so the plugin
/// can say what its run covered. Without one, `args` as they are.
///
/// # Errors
///
/// The scope does not read or resolve, or the call names breakpoints too.
pub fn plugin_args(config: &ProjectConfig, args: &Value) -> Result<Value, RunScopeError> {
    let Value::Object(fields) = args else {
        return Ok(args.clone());
    };
    let Some(raw) = fields.get(PLUGIN_SCOPE_ARG) else {
        return Ok(args.clone());
    };
    if fields.contains_key(BREAKPOINTS_ARG) {
        return Err(RunScopeError::Both);
    }
    let scope: ViewScope =
        serde_json::from_value(raw.clone()).map_err(|_| RunScopeError::Invalid)?;
    let mut fed = fields.clone();
    if scope != ViewScope::All {
        let breakpoints = resolve_scope(config, &scope)?;
        fed.insert(
            BREAKPOINTS_ARG.to_owned(),
            Value::Array(breakpoints.into_iter().map(Value::String).collect()),
        );
    }
    Ok(Value::Object(fed))
}

/// `args` with `scope` as its [`PLUGIN_SCOPE_ARG`] when it has none (a plugin command that runs
/// over the views the grid shows, `scope = "visible"` in its manifest).
#[must_use]
pub fn with_default_scope(args: &Value, scope: Option<&ViewScope>) -> Value {
    let (Some(scope), Value::Object(fields)) = (scope, args) else {
        return args.clone();
    };
    if fields.contains_key(PLUGIN_SCOPE_ARG) || fields.contains_key(BREAKPOINTS_ARG) {
        return args.clone();
    }
    let mut fed = fields.clone();
    fed.insert(
        PLUGIN_SCOPE_ARG.to_owned(),
        serde_json::to_value(scope).unwrap_or(Value::Null),
    );
    Value::Object(fed)
}

#[cfg(test)]
#[path = "run_scope_tests.rs"]
mod tests;
