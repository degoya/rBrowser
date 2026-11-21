//! `view.edit_address` (RB-092-03, `Leader S L`): the address field of a view whose navigation
//! is decoupled. Without `url` it opens the field in the view's label strip - the grid's native
//! strip (`UiBackend::edit_address`, `cef_app/address_field.rs`), or on the canvas the board's
//! page (`uiCommand`, `ui/src/canvas/BoardAddressField.vue`) - with the view's address; with
//! `url`, the field's Enter, it loads that address in that view only, checked as
//! [`super::address::resolve`] says, and the view stays decoupled. A refusal of a typed address
//! goes to the status bar with its reason. MCP loads a view's page with `navigate { target:
//! viewId }`, the command's coverage.

use std::collections::BTreeMap;
use std::sync::{Arc, Weak};

use rb_commands::{
    Binding, Chord, CommandDef, CommandError, CommandId, CommandRegistry, Group, Handler, Key,
    McpCoverage,
};
use rb_config::LayoutMode;
use rb_engine::{EngineError, Target, ViewHost as _, ViewId, ViewInfo};
use rb_i18n::t_with;
use rb_ipc::{CoreToShell, Notification, NotificationLevel};
use serde_json::{Value, json};

use super::address::{EDIT_ADDRESS, Refusal, field_text, may_edit, resolve};
use crate::control::Core;

/// The MCP tool that loads a view's page.
const TOOL: &str = "navigate";

/// The definition; title key `commands.view.edit_address`.
pub fn definitions() -> Result<Vec<CommandDef>, CommandError> {
    Ok(vec![CommandDef {
        id: CommandId::new(EDIT_ADDRESS)?,
        title_key: format!("commands.{EDIT_ADDRESS}"),
        group: Group::Sync,
        default_bindings: vec![Binding::Leader(Chord {
            keys: vec![Key::Char('S'), Key::Char('L')],
        })],
        mcp: McpCoverage::Covered(TOOL.to_owned()),
    }])
}

/// Registers the command with `registry`, running [`run`]; returns what the registry refused
/// (a test holds it empty).
pub fn register(registry: &mut CommandRegistry, core: &Arc<Core>) -> Vec<CommandError> {
    let defs = match definitions() {
        Ok(defs) => defs,
        Err(error) => return vec![error],
    };
    defs.into_iter()
        .filter_map(|def| registry.register(def, handler(Arc::downgrade(core))).err())
        .collect()
}

fn handler(core: Weak<Core>) -> Handler {
    Arc::new(move |args: Value| {
        let core = core
            .upgrade()
            .ok_or_else(|| CommandError::Failed("the window is closed".to_owned()))?;
        run(&core, &args)
    })
}

/// `view.edit_address { view?, url? }`: opens the field of `view` (the active view when absent)
/// and answers `{ view, open: true }`, or loads `url` in it and answers `{ view, url }`.
///
/// # Errors
///
/// [`CommandError::InvalidArgs`] for a view that does not exist, malformed arguments and a
/// typed address the field refuses (its reason also in the status bar);
/// [`CommandError::Failed`] when the field cannot open - the setting is off, the view follows
/// the active view, there is no active view.
pub fn run(core: &Core, args: &Value) -> Result<Value, CommandError> {
    let host = core.host();
    let view = match args.get("view") {
        None | Some(Value::Null) => host
            .active_view()
            .ok_or_else(|| CommandError::Failed("there is no active view".to_owned()))?,
        Some(view) => serde_json::from_value::<ViewId>(view.clone())
            .map_err(|error| CommandError::InvalidArgs(format!("view: {error}")))?,
    };
    let info = host
        .view(view)
        .ok_or_else(|| CommandError::InvalidArgs(format!("no view {view}")))?;
    let decoupled = crate::nav_sync::for_host(host).is_some_and(|nav| !nav.is_coupled(view));
    let allowed = may_edit(&core.user_settings().ui.cell_label, !decoupled);
    match args.get("url") {
        None | Some(Value::Null) => {
            allowed.map_err(|refusal| CommandError::Failed(refusal.message(&info.label)))?;
            open(core, &info);
            Ok(json!({ "view": view, "open": true }))
        }
        Some(Value::String(typed)) => {
            let config = host.config();
            let environment = host.shell_state().environment;
            let base = rb_config::bookmarks::base_url(&config, environment.as_deref());
            let url = allowed
                .and_then(|()| resolve(typed, base.as_deref()))
                .map_err(|refusal| refused(core, &refusal, &info.label))?;
            crate::nav_sync::navigate(host, Target::View(view), &url).map_err(failed)?;
            Ok(json!({ "view": view, "url": url }))
        }
        Some(_) => Err(CommandError::InvalidArgs("url: a string".to_owned())),
    }
}

/// Opens the field of the view `info` names with its address: on the canvas in the board's page,
/// which draws the labels there; else in the grid's native strip.
fn open(core: &Core, info: &ViewInfo) {
    let url = field_text(&info.url).to_owned();
    let canvas = core
        .host()
        .mode_spec()
        .is_some_and(|spec| spec.mode() == LayoutMode::Canvas);
    if canvas {
        core.send(&CoreToShell::UiCommand {
            id: EDIT_ADDRESS.to_owned(),
            args: Some(json!({ "view": info.id, "url": url })),
        });
    } else {
        let name = t_with("sync.address.label", &[("label", &info.label)]);
        core.ui().edit_address(info.id, url, name);
    }
}

/// A typed address refused: its reason in the status bar, and as the command's error.
fn refused(core: &Core, refusal: &Refusal, label: &str) -> CommandError {
    core.notify(CoreToShell::Notification(Notification {
        level: NotificationLevel::Warning,
        message_key: refusal.key().to_owned(),
        args: refusal
            .args(label)
            .into_iter()
            .map(|(name, value)| (name.to_owned(), value))
            .collect::<BTreeMap<_, _>>(),
        settings_key: refusal.settings_key().map(str::to_owned),
    }));
    CommandError::InvalidArgs(refusal.message(label))
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
#[path = "edit_tests.rs"]
mod tests;
