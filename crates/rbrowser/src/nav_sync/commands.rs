//! The commands of navigation sync: `sync.toggle_view_navigation` (`Leader S U`), and the
//! explicit navigation of the core commands `view.back`, `view.forward`, `view.navigate` and
//! `reload.all`, which reach every view whose navigation is coupled (`reload.all` every view) -
//! or, with `view`, that one view (the address field of a decoupled view's label strip). They
//! answer what they reached ([`super::NavOutcome`]).

use rb_commands::CommandError;
use rb_engine::{EngineError, Target, ViewHost, ViewId};
use serde_json::{Value, json};

use super::{TOGGLE_VIEW_NAVIGATION, back, couple, for_host, forward, navigate, reload};
use crate::control::Core;

/// Runs the command `id` when navigation sync owns it; `None` otherwise (and for
/// `view.navigate` without a URL, which focuses the address bar).
pub fn run_command(core: &Core, id: &str, args: &Value) -> Option<Result<Value, CommandError>> {
    let host = core.host();
    let result = match id {
        TOGGLE_VIEW_NAVIGATION => return Some(toggle(core, args)),
        "view.back" => target_of(args).and_then(|target| back(host, target).map_err(failed)),
        "view.forward" => target_of(args).and_then(|target| forward(host, target).map_err(failed)),
        "view.navigate" => {
            let url = args.get("url").and_then(Value::as_str)?;
            target_of(args).and_then(|target| navigate(host, target, url).map_err(failed))
        }
        "reload.all" => {
            let ignore_cache = args
                .get("ignoreCache")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            target_of(args).and_then(|target| reload(host, target, ignore_cache).map_err(failed))
        }
        _ => return None,
    };
    Some(result.and_then(|outcome| {
        serde_json::to_value(outcome).map_err(|error| CommandError::Failed(error.to_string()))
    }))
}

/// `view` in the arguments (the address field): that view; else `target` (MCP: `"all"` or a
/// view id); absent: `all`.
fn target_of(args: &Value) -> Result<Target, CommandError> {
    match (args.get("view"), args.get("target")) {
        (Some(view), _) if !view.is_null() => serde_json::from_value::<ViewId>(view.clone())
            .map(Target::View)
            .map_err(|error| CommandError::InvalidArgs(format!("view: {error}"))),
        (_, Some(target)) if !target.is_null() => serde_json::from_value::<Target>(target.clone())
            .map_err(|error| CommandError::InvalidArgs(format!("target: {error}"))),
        _ => Ok(Target::All),
    }
}

/// `sync.toggle_view_navigation { view?, enabled? }`: answers the view and whether its
/// navigation is coupled now.
fn toggle(core: &Core, args: &Value) -> Result<Value, CommandError> {
    let host = core.host();
    let nav = for_host(host)
        .ok_or_else(|| CommandError::Failed("navigation sync is not started".to_owned()))?;
    let view = match args.get("view") {
        None | Some(Value::Null) => host
            .active_view()
            .ok_or_else(|| CommandError::Failed("there is no active view".to_owned()))?,
        Some(view) => serde_json::from_value::<ViewId>(view.clone())
            .map_err(|error| CommandError::InvalidArgs(format!("view: {error}")))?,
    };
    let coupled = match args.get("enabled") {
        None | Some(Value::Null) => !nav.is_coupled(view),
        Some(Value::Bool(enabled)) => *enabled,
        Some(_) => {
            return Err(CommandError::InvalidArgs("enabled: a boolean".to_owned()));
        }
    };
    couple(host, view, coupled).map_err(failed)?;
    Ok(json!({ "view": view, "syncNavigation": coupled }))
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
#[path = "commands_tests.rs"]
mod tests;
