//! The commands `view.external.arrange`, `view.external.raise` and `engine.restart`; WebKit's
//! `engine.webkit.toggle` is [`super::webkit::run_command`].

use rb_commands::CommandError;
use rb_config::EngineKind;
use rb_engine::ViewId;
use rb_engine::external::RestartError;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{ARRANGE, HostEngines, RAISE, RESTART, for_host};
use crate::control::Core;

/// The arguments of `engine.restart`.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RestartArgs {
    /// The engine; absent: the view's, or every engine with an external view.
    #[serde(default)]
    pub engine: Option<EngineKind>,
    /// One view; absent: every view of the engine.
    #[serde(default)]
    pub view: Option<ViewId>,
}

/// Runs the command `id` when it is one of the engines'; `None` otherwise.
pub fn run_command(core: &Core, id: &str, args: &Value) -> Option<Result<Value, CommandError>> {
    if id == super::webkit::TOGGLE {
        return super::webkit::run_command(core, id, args);
    }
    if ![ARRANGE, RAISE, RESTART].contains(&id) {
        return None;
    }
    let Some(engines) = for_host(core.host()) else {
        return Some(Err(CommandError::Failed(
            "the external windows are not started".to_owned(),
        )));
    };
    engines.apply_settings(&core.user_settings().engines.external);
    Some(match id {
        ARRANGE => {
            // "Show as own windows": the embedded views leave their cells first (RB-080-39).
            let released = engines.release_all();
            Ok(json!({ "moved": engines.windows().arrange(), "released": released }))
        }
        RAISE => Ok(json!({ "raised": engines.windows().raise_all() })),
        _ => parse(args).and_then(|args| {
            restart(&engines, args.engine, args.view)
                .map(|views| json!({ "restarted": views }))
                .map_err(|error| CommandError::Failed(error.to_string()))
        }),
    })
}

/// Restarts the external views of `engine` (or `view` alone). Without an engine: the view's,
/// or every engine that has an external view.
pub fn restart(
    engines: &HostEngines,
    engine: Option<EngineKind>,
    view: Option<ViewId>,
) -> Result<Vec<ViewId>, RestartError> {
    if engine == Some(EngineKind::Chromium) {
        return Err(RestartError::Unavailable {
            engine: EngineKind::Chromium,
            reason: "Chromium runs the grid and restarts with rBrowser only".to_owned(),
        });
    }
    let windows = engines.windows();
    let kinds: Vec<EngineKind> = match (engine, view) {
        (Some(kind), _) => vec![kind],
        (None, Some(view)) => match windows.view(view) {
            Some(known) => vec![known.engine],
            None => {
                return Err(RestartError::UnknownView {
                    engine: EngineKind::Firefox,
                    view,
                });
            }
        },
        (None, None) => {
            let mut kinds: Vec<EngineKind> = Vec::new();
            for known in windows.list() {
                if !kinds.contains(&known.engine) {
                    kinds.push(known.engine);
                }
            }
            if kinds.is_empty() {
                return Err(RestartError::NotRunning(EngineKind::Firefox));
            }
            kinds
        }
    };
    let mut restarted = Vec::new();
    let mut first_error = None;
    for kind in kinds {
        match windows.restart(kind, view) {
            Ok(views) => restarted.extend(views),
            Err(error) => {
                first_error.get_or_insert(error);
            }
        }
    }
    match first_error {
        Some(error) if restarted.is_empty() => Err(error),
        _ => Ok(restarted),
    }
}

fn parse(args: &Value) -> Result<RestartArgs, CommandError> {
    if args.is_null() {
        return Ok(RestartArgs::default());
    }
    serde_json::from_value(args.clone())
        .map_err(|error| CommandError::InvalidArgs(error.to_string()))
}

#[cfg(test)]
#[path = "commands_tests.rs"]
mod tests;
