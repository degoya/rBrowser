//! `ui.view_footer.toggle` (`Leader O B`, RB-080-44) in the Overlays group: shows or hides the
//! footer under every view by switching `ui.viewFooter.visible`. It has no tool: the switch is
//! a setting (`settings_set`), and `views_list` names each view's range.

use std::sync::{Arc, Weak};

use rb_commands::{
    Binding, Chord, CommandDef, CommandError, CommandId, CommandRegistry, Group, Handler, Key,
    McpCoverage,
};
use rb_ipc::CoreToShell;
use serde_json::{Value, json};

use crate::control::Core;
use crate::settings_window::Caller;

/// Show or hide the footer under every view.
pub const TOGGLE: &str = "ui.view_footer.toggle";
/// The setting the command switches.
pub const VISIBLE_KEY: &str = "ui.viewFooter.visible";
/// Why the command has no tool.
pub const TOGGLE_OMITTED: &str = "display switch; settings_set writes ui.viewFooter.visible";
/// The key after `O`: the breakpoint bar.
const KEY: char = 'B';

/// The definition; title key `commands.ui.view_footer.toggle`.
///
/// # Errors
///
/// An id the registry would refuse (a test holds it valid).
pub fn definitions() -> Result<Vec<CommandDef>, CommandError> {
    Ok(vec![CommandDef {
        id: CommandId::new(TOGGLE)?,
        title_key: format!("commands.{TOGGLE}"),
        group: Group::Overlays,
        default_bindings: vec![Binding::Leader(Chord {
            keys: vec![Key::Char(Group::Overlays.leader_key()), Key::Char(KEY)],
        })],
        mcp: McpCoverage::Omitted(TOGGLE_OMITTED.to_owned()),
    }])
}

/// Registers the command with `registry`; returns what the registry refused.
pub fn register(registry: &mut CommandRegistry, core: &Arc<Core>) -> Vec<CommandError> {
    let defs = match definitions() {
        Ok(defs) => defs,
        Err(error) => return vec![error],
    };
    defs.into_iter()
        .filter_map(|def| {
            let handler = handler(Arc::downgrade(core));
            registry.register(def, handler).err()
        })
        .collect()
}

fn handler(core: Weak<Core>) -> Handler {
    Arc::new(move |_args: Value| {
        let core = core
            .upgrade()
            .ok_or_else(|| CommandError::Failed("the window is closed".to_owned()))?;
        toggle(&core)
    })
}

/// Writes the switched `ui.viewFooter.visible`; the views follow the setting.
///
/// # Errors
///
/// The setting could not be written.
pub fn toggle(core: &Core) -> Result<Value, CommandError> {
    let visible = !core.user_settings().ui.view_footer.visible;
    let answer = crate::settings_window::set_setting(
        core,
        VISIBLE_KEY,
        &Value::Bool(visible),
        rb_settings::Scope::User,
        Caller::Shell,
    );
    if let CoreToShell::SettingRejected { .. } = answer {
        return Err(CommandError::Failed(format!(
            "{VISIBLE_KEY} could not be written"
        )));
    }
    Ok(json!({ "visible": visible }))
}

#[cfg(test)]
#[path = "defs_tests.rs"]
mod tests;
