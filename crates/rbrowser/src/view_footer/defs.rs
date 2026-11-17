//! The switches of the bars under every view, in the Overlays group: `ui.view_footer.toggle`
//! (`Leader O B`, RB-080-44) shows or hides the breakpoint bar by switching
//! `ui.viewFooter.visible`, `ui.vitals_bar.toggle` (`Leader O W`, RB-090-71) the vitals bar by
//! switching `ui.viewFooter.vitals`, `ui.lighthouse_bar.toggle` (`Leader O Q`, RB-090-73) the
//! Lighthouse bar by switching `ui.viewFooter.lighthouse`. None has a tool: the switch is a
//! setting (`settings_set`), `views_list` names each view's range, `view_metrics` its vitals and
//! `lighthouse_reports` the scores of every run.

use std::sync::{Arc, Weak};

use rb_commands::{
    Binding, Chord, CommandDef, CommandError, CommandId, CommandRegistry, Group, Handler, Key,
    McpCoverage,
};
use rb_ipc::CoreToShell;
use serde_json::{Value, json};

use crate::control::Core;
use crate::settings_window::Caller;

/// Show or hide the breakpoint bar under every view.
pub const TOGGLE: &str = "ui.view_footer.toggle";
/// The setting the command switches.
pub const VISIBLE_KEY: &str = "ui.viewFooter.visible";
/// Why the command has no tool.
pub const TOGGLE_OMITTED: &str = "display switch; settings_set writes ui.viewFooter.visible";
/// Show or hide the vitals bar under every view (RB-090-71).
pub const VITALS_TOGGLE: &str = "ui.vitals_bar.toggle";
/// The setting the vitals bar's command switches.
pub const VITALS_KEY: &str = "ui.viewFooter.vitals";
/// Why the vitals bar's command has no tool.
pub const VITALS_OMITTED: &str =
    "display switch; settings_set writes ui.viewFooter.vitals, view_metrics reads the vitals";

/// Show or hide the Lighthouse bar under every view (RB-090-73).
pub const LIGHTHOUSE_TOGGLE: &str = "ui.lighthouse_bar.toggle";
/// The setting the Lighthouse bar's command switches.
pub const LIGHTHOUSE_KEY: &str = "ui.viewFooter.lighthouse";
/// Why the Lighthouse bar's command has no tool.
pub const LIGHTHOUSE_OMITTED: &str = "display switch; settings_set writes ui.viewFooter.lighthouse, lighthouse_reports reads the scores";

/// Id, the key after `O`, the setting it switches, why it has no tool: `B` the breakpoint bar,
/// `W` the (Web) vitals bar, `Q` the Lighthouse bar's quality scores.
const ROWS: [(&str, char, &str, &str); 3] = [
    (TOGGLE, 'B', VISIBLE_KEY, TOGGLE_OMITTED),
    (VITALS_TOGGLE, 'W', VITALS_KEY, VITALS_OMITTED),
    (LIGHTHOUSE_TOGGLE, 'Q', LIGHTHOUSE_KEY, LIGHTHOUSE_OMITTED),
];

/// The definitions; title keys `commands.<id>`.
///
/// # Errors
///
/// An id the registry would refuse (a test holds them valid).
pub fn definitions() -> Result<Vec<CommandDef>, CommandError> {
    ROWS.iter()
        .map(|(id, key, _, omitted)| {
            Ok(CommandDef {
                id: CommandId::new(id)?,
                title_key: format!("commands.{id}"),
                group: Group::Overlays,
                default_bindings: vec![Binding::Leader(Chord {
                    keys: vec![Key::Char(Group::Overlays.leader_key()), Key::Char(*key)],
                })],
                mcp: McpCoverage::Omitted((*omitted).to_owned()),
            })
        })
        .collect()
}

/// The setting the command `id` switches.
#[must_use]
pub fn setting_of(id: &str) -> Option<&'static str> {
    ROWS.iter()
        .find(|(own, _, _, _)| *own == id)
        .map(|(_, _, key, _)| *key)
}

/// Registers the commands with `registry`; returns what the registry refused.
pub fn register(registry: &mut CommandRegistry, core: &Arc<Core>) -> Vec<CommandError> {
    let defs = match definitions() {
        Ok(defs) => defs,
        Err(error) => return vec![error],
    };
    defs.into_iter()
        .filter_map(|def| {
            let handler = handler(def.id.as_str().to_owned(), Arc::downgrade(core));
            registry.register(def, handler).err()
        })
        .collect()
}

fn handler(id: String, core: Weak<Core>) -> Handler {
    Arc::new(move |_args: Value| {
        let core = core
            .upgrade()
            .ok_or_else(|| CommandError::Failed("the window is closed".to_owned()))?;
        match id.as_str() {
            VITALS_TOGGLE => toggle_vitals(&core),
            LIGHTHOUSE_TOGGLE => toggle_lighthouse(&core),
            _ => toggle(&core),
        }
    })
}

/// Writes the switched `ui.viewFooter.visible`; the views follow the setting.
///
/// # Errors
///
/// The setting could not be written.
pub fn toggle(core: &Core) -> Result<Value, CommandError> {
    let visible = !core.user_settings().ui.view_footer.visible;
    write(core, VISIBLE_KEY, visible)
}

/// Writes the switched `ui.viewFooter.vitals` (RB-090-71); the views follow the setting. On
/// switches measuring on too (RB-090-72, [`crate::vitals::defs::measure_for_bar`]).
///
/// # Errors
///
/// A setting could not be written.
pub fn toggle_vitals(core: &Core) -> Result<Value, CommandError> {
    let visible = !core.user_settings().ui.view_footer.vitals;
    let answer = write(core, VITALS_KEY, visible)?;
    if visible {
        crate::vitals::defs::measure_for_bar(core)?;
    }
    Ok(answer)
}

/// Writes the switched `ui.viewFooter.lighthouse` (RB-090-73); the views follow the setting.
///
/// # Errors
///
/// The setting could not be written.
pub fn toggle_lighthouse(core: &Core) -> Result<Value, CommandError> {
    let visible = !core.user_settings().ui.view_footer.lighthouse;
    write(core, LIGHTHOUSE_KEY, visible)
}

/// Writes `key` as `visible` in the user settings.
fn write(core: &Core, key: &str, visible: bool) -> Result<Value, CommandError> {
    let answer = crate::settings_window::set_setting(
        core,
        key,
        &Value::Bool(visible),
        rb_settings::Scope::User,
        Caller::Shell,
    );
    if let CoreToShell::SettingRejected { .. } = answer {
        return Err(CommandError::Failed(format!("{key} could not be written")));
    }
    Ok(json!({ "visible": visible }))
}

#[cfg(test)]
#[path = "defs_tests.rs"]
mod tests;
