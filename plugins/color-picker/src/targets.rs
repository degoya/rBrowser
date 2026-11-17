//! The views a call covers: `target` (`"all"` or a view id, as the host resolves MCP's
//! `target`), else the default the call names - the active view or every view.

use rb_plugin_api::{Failure, FailureKind, PluginHost, ViewId, ViewInfo};
use serde_json::{Value, json};

use crate::{failure, invalid};

/// The most views one call covers.
pub const MAX_VIEWS: usize = 16;

/// What a call without `target` covers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Default {
    Active,
    All,
}

/// The views `args["target"]` names, at most [`MAX_VIEWS`], in grid order.
///
/// # Errors
///
/// `invalid` for a `target` of another shape; `unknown_view` for an id the grid does not have
/// or no active view.
pub fn resolve(
    host: &mut dyn PluginHost,
    args: &Value,
    default: Default,
) -> Result<Vec<ViewInfo>, Failure> {
    let views = host.views()?;
    let one = |id: ViewId, views: Vec<ViewInfo>| {
        views
            .into_iter()
            .find(|view| view.id == id)
            .map(|view| vec![view])
            .ok_or_else(|| {
                failure(FailureKind::NotFound, "unknown_view").with_param("view", id.to_string())
            })
    };
    match &args["target"] {
        Value::String(all) if all == "all" => Ok(views.into_iter().take(MAX_VIEWS).collect()),
        Value::Number(number) => {
            let id = number
                .as_u64()
                .and_then(|id| ViewId::try_from(id).ok())
                .ok_or_else(|| invalid("target"))?;
            one(id, views)
        }
        Value::Null => match default {
            Default::All => Ok(views.into_iter().take(MAX_VIEWS).collect()),
            Default::Active => {
                let active = host.active_view()?.ok_or_else(|| {
                    failure(FailureKind::NotFound, "unknown_view").with_param("view", "active")
                })?;
                one(active, views)
            }
        },
        _ => Err(invalid("target")),
    }
}

/// Whether the plugin reads `view` (evaluate and CDP: Chromium only for now).
#[must_use]
pub fn supported(view: &ViewInfo) -> bool {
    view.engine == "chromium"
}

/// The answer for a view the plugin does not read.
#[must_use]
pub fn unsupported(view: &ViewInfo) -> Value {
    json!({
        "view": view.id,
        "label": view.label,
        "engine": view.engine,
        "error": crate::key("errors.unsupported_engine"),
    })
}

/// The answer for a view whose call failed: its code only (never a page's text).
#[must_use]
pub fn failed(view: &ViewInfo, failure: &Failure) -> Value {
    json!({
        "view": view.id,
        "label": view.label,
        "error": failure.code.clone().unwrap_or_else(|| "plugins.error.host_failed".to_owned()),
    })
}

#[cfg(test)]
#[path = "targets_tests.rs"]
mod tests;
