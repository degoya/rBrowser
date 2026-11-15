//! The commands of popping a view out (RB-080-14) for the one registry: `view.pop_out`
//! (`Leader V X`, "extract") and `view.pop_in` (`Leader V V`, "back to the view grid") - the job
//! file's `V O` / `V I` are the zoom steps' - each with its MCP tool. Both take `{ view? }`, the
//! active view when absent (the first without one), and answer `{ view, detached, changed }`.
//! The placeholder's click and the cell's context menu run them too.

use std::path::Path;
use std::sync::{Arc, Weak};

use rb_commands::{
    Binding, CommandDef, CommandError, CommandId, CommandRegistry, Group, Handler, McpCoverage,
};
use rb_engine::{ViewHost, ViewId};
use serde_json::{Value, json};

use super::bounds::ScreenRect;
use super::store;
use crate::control::Core;
use crate::host::PopOutOptions;

/// Pop the view out into a window of its own; a view out already comes to the front.
pub const POP_OUT: &str = "view.pop_out";
/// Take the view back into its cell and close its window.
pub const POP_IN: &str = "view.pop_in";

/// Id, default binding, MCP tool.
const ROWS: [(&str, &str, &str); 2] = [
    (POP_OUT, "Ctrl+Shift+Space V X", "view_pop_out"),
    (POP_IN, "Ctrl+Shift+Space V V", "view_pop_in"),
];

/// The definitions; title key `commands.<id>`.
pub fn definitions() -> Result<Vec<CommandDef>, CommandError> {
    ROWS.iter()
        .map(|(id, binding, tool)| {
            Ok(CommandDef {
                id: CommandId::new(id)?,
                title_key: format!("commands.{id}"),
                group: Group::View,
                default_bindings: vec![binding.parse::<Binding>()?],
                mcp: McpCoverage::Covered((*tool).to_owned()),
            })
        })
        .collect()
}

/// Registers the commands with `registry`; returns what the registry refused.
pub fn register(registry: &mut CommandRegistry, core: &Arc<Core>) -> Vec<CommandError> {
    let defs = match definitions() {
        Ok(defs) => defs,
        Err(error) => return vec![error],
    };
    // The remembered bounds are read here, off the UI thread, not in the first command.
    if let Some(path) = store::default_path() {
        core.runtime().spawn_blocking(move || store::warm(&path));
    }
    defs.into_iter()
        .filter_map(|def| {
            let handler = handler(def.id.as_str(), Arc::downgrade(core));
            registry.register(def, handler).err()
        })
        .collect()
}

fn handler(id: &str, core: Weak<Core>) -> Handler {
    let id = id.to_owned();
    Arc::new(move |args: Value| {
        let core = core
            .upgrade()
            .ok_or_else(|| CommandError::Failed("the window is closed".to_owned()))?;
        run_with(&core, &id, &args, store::default_path().as_deref())
    })
}

/// Runs `id` with `args`; the remembered bounds are read from `state` (`pop-out.json`).
pub fn run_with(
    core: &Core,
    id: &str,
    args: &Value,
    state: Option<&Path>,
) -> Result<Value, CommandError> {
    let host = core.host();
    let view = view_arg(core, args)?;
    // A Firefox or WebKit view is a window of its own already: popping it out shows it as one
    // ("released" from its cell), popping it in puts it back into its cell (RB-080-39).
    if let Some(engines) =
        crate::engines::for_host(host).filter(|engines| engines.is_external(view))
    {
        let changed = match id {
            POP_OUT => engines.release(view),
            POP_IN => engines.embed_again(view),
            other => return Err(CommandError::UnknownCommand(other.to_owned())),
        };
        // The grid never pops the stand-in out: `detached` is the host's word, as in views_list.
        let detached = host.is_detached(view);
        return Ok(json!({ "view": view, "detached": detached, "changed": changed }));
    }
    let failed = |error: rb_engine::EngineError| CommandError::Failed(error.to_string());
    let (changed, detached) = match id {
        POP_OUT => {
            let settings = core.user_settings().pop_out;
            let remembered = if settings.remember_bounds {
                let key = bounds_key(core, view);
                state
                    .zip(key)
                    .and_then(|(path, key)| store::remembered(path, &key))
            } else {
                None
            };
            let options = PopOutOptions {
                keep_placeholder: settings.keep_placeholder,
                fit_to_screen: settings.fit_to_screen,
                remembered,
            };
            (host.pop_out(view, options).map_err(failed)?, true)
        }
        POP_IN => (host.pop_in(view).map_err(failed)?, false),
        other => return Err(CommandError::UnknownCommand(other.to_owned())),
    };
    Ok(json!({ "view": view, "detached": detached, "changed": changed }))
}

/// The window of `view` was closed by hand at `client` (its client area, screen DIP): the bounds
/// are kept for its breakpoint (`popOut.rememberBounds`, in `state`) and the view goes back into
/// its cell. Runs off the UI thread (it writes a file).
pub fn window_closed(core: &Core, view: ViewId, client: Option<ScreenRect>, state: Option<&Path>) {
    let remember = core.user_settings().pop_out.remember_bounds;
    if let (true, Some(client), Some(path), Some(key)) =
        (remember, client, state, bounds_key(core, view))
        && let Err(error) = store::remember_and_save(path, &key, client)
    {
        tracing::warn!(%error, "the pop-out window's bounds were not saved");
    }
    if let Err(error) = core.host().pop_in(view) {
        tracing::warn!(%view, %error, "the view did not go back into the grid");
    }
}

/// The view of `args.view`, else the active view, else the first.
fn view_arg(core: &Core, args: &Value) -> Result<ViewId, CommandError> {
    match args.get("view").filter(|value| !value.is_null()) {
        Some(value) => serde_json::from_value(value.clone())
            .map_err(|_| CommandError::InvalidArgs("view is a view id".to_owned())),
        None => core
            .host()
            .target_views(None)
            .map_err(|error| CommandError::Failed(error.to_string()))?
            .first()
            .copied()
            .ok_or_else(|| CommandError::Failed("there is no view".to_owned())),
    }
}

/// The breakpoint whose window bounds `view` uses: the one it shows, else its cell's.
fn bounds_key(core: &Core, view: ViewId) -> Option<String> {
    let info = core.host().view(view)?;
    info.breakpoint.or(info.cell_breakpoint)
}

#[cfg(test)]
#[path = "commands_tests.rs"]
mod tests;
