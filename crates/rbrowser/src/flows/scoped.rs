//! The views a flow run takes from a command's or a tool's arguments (RB-099-10): `views` (view
//! ids, breakpoint ids or labels, [`super::plan::resolve_views`]), else `scope` - `"visible"` or
//! breakpoint ids, the views showing them ([`crate::run_scope`]), resolved once when the run
//! starts -, else none and the flow's own `// @views`. `flows.run_all_visible` is
//! `flows.run_all` over the views the grid shows. A chosen breakpoint no view shows is named in a
//! notice, never dropped silently.

use rb_config::{ProjectConfig, ViewScope};
use rb_engine::{ViewHost as _, ViewId, ViewInfo};
use serde_json::Value;

use super::FlowError;
use super::plan::resolve_views;
use crate::control::Core;
use crate::run_scope;

/// The views of `args`' `views` or `scope` among `views` in `config`, and the chosen
/// breakpoints no view shows; `None` for neither (or the scope `"all"`): the flow's own.
///
/// # Errors
///
/// [`FlowError::NoViews`]: a name no view has, a scope that does not read or resolve, or one no
/// view shows.
pub fn views_of(
    config: &ProjectConfig,
    views: &[ViewInfo],
    args: &Value,
) -> Result<(Option<Vec<ViewId>>, Vec<String>), FlowError> {
    if let Some(asked) = resolve_views(views, &args["views"])? {
        return Ok((Some(asked), Vec::new()));
    }
    let (_, scope) = run_scope::take_scope(args).map_err(no_views)?;
    let Some(scope) = scope else {
        return Ok((None, Vec::new()));
    };
    match run_scope::scoped(config, views, &scope).map_err(no_views)? {
        Some(scoped) => Ok((Some(scoped.views), scoped.not_shown)),
        None => Ok((None, Vec::new())),
    }
}

/// [`views_of`] against `core`'s views now, the notice of the breakpoints no view shows sent.
///
/// # Errors
///
/// As [`views_of`].
pub fn views(core: &Core, args: &Value) -> Result<Option<Vec<ViewId>>, FlowError> {
    let host = core.host();
    let (views, not_shown) = views_of(&host.config(), &host.views(), args)?;
    if let Some(notice) = run_scope::not_shown_notice(&not_shown) {
        core.notify(notice);
    }
    Ok(views)
}

/// [`views`] for `scope` alone (MCP `flow_run`'s `target`).
///
/// # Errors
///
/// As [`views_of`].
pub fn scope_views(core: &Core, scope: &ViewScope) -> Result<Option<Vec<ViewId>>, FlowError> {
    let scope =
        serde_json::to_value(scope).map_err(|error| FlowError::NoViews(error.to_string()))?;
    let args = serde_json::Map::from_iter([(run_scope::SCOPE_ARG.to_owned(), scope)]);
    views(core, &Value::Object(args))
}

/// The views the grid shows (`flows.run_all_visible`).
///
/// # Errors
///
/// As [`views_of`].
pub fn visible(core: &Core) -> Result<Option<Vec<ViewId>>, FlowError> {
    scope_views(core, &ViewScope::Visible)
}

fn no_views(error: run_scope::RunScopeError) -> FlowError {
    FlowError::NoViews(error.to_string())
}

#[cfg(test)]
#[path = "scoped_tests.rs"]
mod tests;
