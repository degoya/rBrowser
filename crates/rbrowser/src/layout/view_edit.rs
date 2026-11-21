//! Views added and removed in every layout mode (RB-080-41): which breakpoints are open is a
//! selection, `layout.hidden` in `config.local.json`, that the grid, focus, horizontal, split and
//! canvas all follow (`host::rebuild`, `layout::spec`, `presets::layouts`).
//!
//! | Command | Default | |
//! | --- | --- | --- |
//! | `view.add { breakpoint \| "all", x?, y? }` | `Leader V +` | opens the view again; on the canvas at `x`/`y` (canvas DIP), else after the others; without a breakpoint the shell's "+" menu opens |
//! | `view.remove { view? \| breakpoint? }` | `Leader V -` | hides the breakpoint and closes its view (a window of its own included); the last view of the grid in effect stays; an ad-hoc view (the quick pick's free size) just closes |
//! | `view.add_size { width, height, name?, dpr?, x?, y? }` or `{ device, name?, x?, y? }` | `Leader V *` | a breakpoint of that size or device preset in `config.local.json`, opened; without either the picker opens (`view_add_size`) |
//!
//! Every write goes to `config.local.json` (the selection is personal), the config is read again
//! and the preset or mode in effect comes back with it, as after the watcher's reload. A preset
//! saved with `hidden` restores the selection when chosen ([`take_preset_selection`]); one
//! without leaves it alone. The grid in effect ([`base_grid`]: the preset's own grid, else the
//! config's) decides which hidden breakpoints have a cell to open ([`addable`]) and what the
//! last view is. MCP: `view_add`, `view_remove`, `view_add_size`.

use std::sync::{Arc, Weak};

use rb_commands::{
    Binding, CommandDef, CommandError, CommandId, CommandRegistry, Group, Handler, McpCoverage,
};
use rb_config::{ConfigFile, LayoutMode, ProjectConfig};
use rb_engine::{ViewHost as _, ViewId};
use rb_ipc::CoreToShell;
use serde::Deserialize;
use serde_json::{Map, Value, json};

use super::store;
pub use super::view_add_size::{
    ADD_SIZE_CUSTOM, MAX_NAME_CHARS, add_breakpoint, add_engine_view, breakpoint_id,
};
pub use super::view_selection::{addable, addable_in, base_grid, selection, take_preset_selection};
use crate::control::Core;

/// Open a hidden breakpoint's view again.
pub const ADD: &str = "view.add";
/// Hide a breakpoint and close its view.
pub const REMOVE: &str = "view.remove";
/// A breakpoint of a new size, opened.
pub const ADD_SIZE: &str = "view.add_size";
/// `view.add`'s `breakpoint` for every hidden one.
pub const ALL: &str = "all";

/// Id, default binding, MCP tool.
const ROWS: [(&str, &str, &str); 3] = [
    (ADD, "Ctrl+Shift+Space V Plus", "view_add"),
    (REMOVE, "Ctrl+Shift+Space V -", "view_remove"),
    (ADD_SIZE, "Ctrl+Shift+Space V *", "view_add_size"),
];

/// `view.add`'s arguments.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AddArgs {
    /// A hidden breakpoint id, or `"all"`; absent opens the shell's menu.
    breakpoint: Option<String>,
    /// Canvas: where the view goes, in canvas DIP (both or neither).
    x: Option<f64>,
    y: Option<f64>,
}

/// `view.remove`'s arguments.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RemoveArgs {
    /// The view; the active view when neither it nor `breakpoint` is given.
    #[serde(default, alias = "viewId")]
    view: Option<ViewId>,
    /// The breakpoint, instead of a view.
    breakpoint: Option<String>,
}

/// Whether `id` is a command of this module.
pub fn is_own(id: &str) -> bool {
    ROWS.iter().any(|(own, _, _)| *own == id)
}

/// The definitions: View group, one leader chord each, title key `commands.<id>`.
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
    defs.into_iter()
        .filter_map(|def| {
            let id = def.id.as_str().to_owned();
            let weak: Weak<Core> = Arc::downgrade(core);
            let handler: Handler = Arc::new(move |args: Value| {
                let core = weak
                    .upgrade()
                    .ok_or_else(|| CommandError::Failed("the window is closed".to_owned()))?;
                run(&core, &id, &args)
            });
            registry.register(def, handler).err()
        })
        .collect()
}

/// Runs the command `id` of this module.
pub fn run(core: &Core, id: &str, args: &Value) -> Result<Value, CommandError> {
    match id {
        ADD => add(core, parse(args)?),
        REMOVE => remove(core, parse(args)?),
        ADD_SIZE => super::view_add_size::add_size(core, parse(args)?),
        other => Err(CommandError::UnknownCommand(other.to_owned())),
    }
}

fn parse<T: for<'de> Deserialize<'de> + Default>(args: &Value) -> Result<T, CommandError> {
    if args.is_null() {
        return Ok(T::default());
    }
    serde_json::from_value(args.clone())
        .map_err(|error| CommandError::InvalidArgs(error.to_string()))
}

/// `view.add`: answers `{ added: [...], hidden: [...], changed }`, or `null` when the shell's
/// menu was opened instead.
fn add(core: &Core, args: AddArgs) -> Result<Value, CommandError> {
    let Some(wanted) = args.breakpoint else {
        core.send(&CoreToShell::UiCommand {
            id: ADD.to_owned(),
            args: None,
        });
        return Ok(Value::Null);
    };
    let at = point(args.x, args.y)?;
    let host = core.host();
    let config = host.config();
    let hidden = selection(host);
    let addable = addable(host);
    let added: Vec<String> = if wanted == ALL {
        addable
    } else {
        known(&config, &wanted)?;
        if hidden.contains(&wanted) && !addable.contains(&wanted) {
            return Err(CommandError::InvalidArgs(format!(
                "breakpoint `{wanted}` has no cell in the grid in effect"
            )));
        }
        addable.into_iter().filter(|id| *id == wanted).collect()
    };
    if added.is_empty() {
        return Ok(json!({ "added": [], "hidden": hidden, "changed": false }));
    }
    let next: Vec<String> = hidden
        .iter()
        .filter(|id| !added.contains(id))
        .cloned()
        .collect();
    write_hidden(core, &next)?;
    crate::presets::on_config_changed(core);
    if let (Some((x, y)), [one]) = (at, added.as_slice()) {
        place_on_canvas(core, one, x, y);
    }
    Ok(json!({ "added": added, "hidden": next, "changed": true }))
}

/// `view.remove`: answers `{ removed, view, hidden, changed }`. An ad-hoc view (a breakpoint
/// not in the config, opened by the quick pick's free size) is closed instead of hidden.
fn remove(core: &Core, args: RemoveArgs) -> Result<Value, CommandError> {
    let host = core.host();
    let config = host.config();
    let (view, breakpoint) = match args.breakpoint {
        Some(breakpoint) => {
            known(&config, &breakpoint)?;
            let view = host
                .views()
                .into_iter()
                .find(|info| info.cell_breakpoint.as_deref() == Some(breakpoint.as_str()))
                .map(|info| info.id);
            (view, breakpoint)
        }
        None => {
            let view = match args.view {
                Some(view) => view,
                None => host
                    .target_views(None)
                    .map_err(|error| CommandError::Failed(error.to_string()))?
                    .first()
                    .copied()
                    .ok_or_else(|| CommandError::Failed("there is no view".to_owned()))?,
            };
            let info = host
                .view(view)
                .ok_or_else(|| CommandError::InvalidArgs(format!("unknown view {view}")))?;
            let breakpoint = info.cell_breakpoint.or(info.breakpoint).ok_or_else(|| {
                CommandError::InvalidArgs(format!("view {view} shows no breakpoint"))
            })?;
            if known(&config, &breakpoint).is_err() {
                host.close_extra_view(view)
                    .map_err(|error| CommandError::Failed(error.to_string()))?;
                let hidden = selection(host);
                return Ok(json!({
                    "removed": breakpoint, "view": view, "hidden": hidden, "changed": true
                }));
            }
            (Some(view), breakpoint)
        }
    };
    let hidden = selection(host);
    if hidden.contains(&breakpoint) {
        return Ok(
            json!({ "removed": breakpoint, "view": view, "hidden": hidden, "changed": false }),
        );
    }
    let mut next = hidden;
    next.push(breakpoint.clone());
    if !rb_config::keeps_a_chromium_view_in(&config, &base_grid(host), &next) {
        return Err(CommandError::Failed(
            "the last view cannot be removed".to_owned(),
        ));
    }
    write_hidden(core, &next)?;
    crate::presets::on_config_changed(core);
    Ok(json!({ "removed": breakpoint, "view": view, "hidden": next, "changed": true }))
}

/// `breakpoint` is one of the config's.
fn known(config: &ProjectConfig, breakpoint: &str) -> Result<(), CommandError> {
    if config
        .breakpoints
        .iter()
        .any(|candidate| candidate.id == breakpoint)
    {
        Ok(())
    } else {
        Err(CommandError::InvalidArgs(format!(
            "no breakpoint `{breakpoint}` in the config"
        )))
    }
}

/// `x` and `y` together, as a canvas point; neither is none.
pub(super) fn point(x: Option<f64>, y: Option<f64>) -> Result<Option<(f64, f64)>, CommandError> {
    match (x, y) {
        (Some(x), Some(y)) if x.is_finite() && y.is_finite() => Ok(Some((x, y))),
        (None, None) => Ok(None),
        _ => Err(CommandError::InvalidArgs(
            "pass x and y, or neither".to_owned(),
        )),
    }
}

/// Writes `hidden` as `layout.hidden` of `config.local.json` (the key goes when empty), reads
/// the config again and brings the preset or mode in effect back.
pub(super) fn write_hidden(core: &Core, hidden: &[String]) -> Result<(), CommandError> {
    let path = ["layout".to_owned()];
    let mut fields = Map::new();
    fields.insert("hidden".to_owned(), hidden_value(core, hidden));
    store::write_into(core, Some(ConfigFile::Local), &path, |document| {
        store::patch(document, &path, None, &fields);
    })?;
    reload(core)
}

/// The config read again after a write. The caller brings the preset or mode in effect back
/// (`presets::on_config_changed`), or applies another.
pub(super) fn reload(core: &Core) -> Result<(), CommandError> {
    crate::commands::reload_config(core.host()).map_err(CommandError::Failed)
}

/// `hidden` as the local file's `layout.hidden`: the key goes when empty (`null`), unless
/// `config.json` hides breakpoints itself - then an explicit `[]` overrides it.
pub(super) fn hidden_value(core: &Core, hidden: &[String]) -> Value {
    if !hidden.is_empty() {
        return json!(hidden);
    }
    let project_hides = core
        .host()
        .project_root()
        .and_then(|root| rb_config::read_json(&ConfigFile::Project.path(&root)).ok())
        .and_then(|document| document.pointer("/layout/hidden").cloned())
        .is_some_and(|value| value.as_array().is_some_and(|list| !list.is_empty()));
    if project_hides {
        json!([])
    } else {
        Value::Null
    }
}

/// On the canvas the just opened view of `breakpoint` goes to `x`/`y`; elsewhere nothing.
pub(super) fn place_on_canvas(core: &Core, breakpoint: &str, x: f64, y: f64) {
    if core.host().shell_state().mode != LayoutMode::Canvas {
        return;
    }
    if let Err(error) = super::canvas_ops::place_breakpoint(core, breakpoint, x, y) {
        tracing::warn!(%breakpoint, %error, "the new view was not placed at the click");
    }
}

#[cfg(test)]
#[path = "view_edit_tests.rs"]
mod tests;
