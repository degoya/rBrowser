//! The page menu's entries for what hangs on the clicked view (RB-090-63, the menu audit):
//! Sync › this view and this view's navigation (check items), Session › the sessions the view
//! can be in (radio) and "Clear session data...", and Tools › Peek, the raw view of a JSON or
//! XML document and Fill forms › (with "Take form as preset", RB-090-70) of the first-party
//! plugins, each once its command is
//! registered. No logic of its own: every entry runs the registry's command, as its shortcut
//! does. CPU throttling is in the Emulation submenu ([`super::emulation`]), SEO, Fonts and
//! Outline under Checks through their manifests' `context_menu`.

use rb_config::DEFAULT_SESSION;
use serde_json::{Value, json};

use super::model::{Action, Builder, Item, Label, Mark};

/// The Sync submenu's label.
pub const SYNC_MENU: &str = "contextmenu.sync.menu";
/// The Session submenu's label.
pub const SESSION_MENU: &str = "contextmenu.session.menu";
/// The Tools submenu's label.
pub const TOOLS_MENU: &str = "contextmenu.tools.menu";
/// The Fill forms submenu's label, inside Tools.
pub const FILL_MENU: &str = "contextmenu.tools.fill_menu";

/// Peek's hover inspector on or off (every view).
pub const PEEK: &str = "plugin.peek.toggle";
/// The JSON/XML viewer between its tree and the raw text.
pub const RAW_VIEW: &str = "plugin.json_xml_viewer.toggle_raw";
/// The forms filled with valid values.
pub const FILL_VALID: &str = "plugin.form_autofill.fill_valid";
/// The forms filled with invalid values.
pub const FILL_INVALID: &str = "plugin.form_autofill.fill_invalid";
/// The clicked view's form taken as an autofill preset (RB-090-70): a right-click on a field
/// focuses it, and the plugin reads the focused field's form.
pub const FILL_CAPTURE: &str = "plugin.form_autofill.capture";
/// The plugins' commands the Tools submenu waits for; a plugin id's `-` is `_` in its command
/// ids (`rb_plugin_host` `PluginManifest::command_id`).
pub const PLUGIN_COMMANDS: [&str; 5] = [PEEK, RAW_VIEW, FILL_VALID, FILL_INVALID, FILL_CAPTURE];

/// What the entries show of the clicked view.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ViewEntries {
    /// The view is in sync (`sync.toggle_view`); `None` before the SyncHub of the window runs.
    pub synced: Option<bool>,
    /// The view's navigation leads and follows (`sync.toggle_view_navigation`); `None` before
    /// navigation sync runs.
    pub navigation_coupled: Option<bool>,
    /// The sessions a view can be in, `default` first; with one only there is no submenu.
    pub sessions: Vec<String>,
    /// The session the view lives in.
    pub session: Option<String>,
}

fn run(command: &str, args: Value) -> Action {
    Action::Run {
        command: command.to_owned(),
        args,
    }
}

impl Builder<'_> {
    /// Sync ›, Session › and Tools ›, each when it has an entry.
    pub(super) fn view_entries(&mut self) -> Vec<Item> {
        [self.sync_menu(), self.session_menu(), self.tools_menu()]
            .into_iter()
            .flatten()
            .collect()
    }

    /// A check item that runs `command` for the clicked view with `enabled` turned around.
    fn view_switch(&mut self, key: &'static str, on: bool, command: &str) -> Item {
        let args = json!({ "view": self.state.view, "enabled": !on });
        self.labelled(Label::Key(key), true, Mark::Check(on), run(command, args))
    }

    fn sync_menu(&mut self) -> Option<Item> {
        let state = self.state;
        let entries = &state.view_entries;
        let mut items = Vec::new();
        if let Some(synced) = entries.synced {
            items.push(self.view_switch(
                "contextmenu.sync.this_view",
                synced,
                crate::sync_hub::TOGGLE_VIEW,
            ));
        }
        if let Some(coupled) = entries.navigation_coupled {
            items.push(self.view_switch(
                "contextmenu.sync.this_view_navigation",
                coupled,
                crate::nav_sync::TOGGLE_VIEW_NAVIGATION,
            ));
        }
        (!items.is_empty()).then(|| self.submenu(Label::Key(SYNC_MENU), items))
    }

    fn session_menu(&mut self) -> Option<Item> {
        let state = self.state;
        let entries = &state.view_entries;
        if entries.sessions.len() < 2 {
            return None;
        }
        let mut items = Vec::new();
        for session in &entries.sessions {
            let label = if session == DEFAULT_SESSION {
                Label::Key("contextmenu.session.default")
            } else {
                Label::Text(session.clone())
            };
            let chosen = entries.session.as_deref() == Some(session.as_str());
            let args = json!({ "target": state.view, "session": session });
            items.push(self.labelled(
                label,
                true,
                Mark::Radio(chosen),
                run(crate::sessions::ASSIGN, args),
            ));
        }
        items.push(Item::Separator);
        // Without `session` the shell asks to confirm, starting at the clicked view's session.
        items.push(self.run(
            "contextmenu.session.clear",
            true,
            crate::sessions::CLEAR,
            json!({ "view": state.view }),
        ));
        Some(self.submenu(Label::Key(SESSION_MENU), items))
    }

    fn tools_menu(&mut self) -> Option<Item> {
        let state = self.state;
        let this = json!({ "target": state.view });
        let mut items = Vec::new();
        if state.knows(PEEK) {
            items.push(self.run("contextmenu.tools.peek", true, PEEK, json!({})));
        }
        if state.knows(RAW_VIEW) {
            let args = this.clone();
            items.push(self.run("contextmenu.tools.raw_view", state.attached, RAW_VIEW, args));
        }
        let mut fill = Vec::new();
        let rows = [
            ("contextmenu.tools.fill_valid", FILL_VALID, false),
            ("contextmenu.tools.fill_invalid", FILL_INVALID, false),
            ("contextmenu.tools.fill_valid_all", FILL_VALID, true),
            ("contextmenu.tools.fill_invalid_all", FILL_INVALID, true),
        ];
        for (key, command, all) in rows {
            if !state.knows(command) {
                continue;
            }
            let (enabled, args) = if all {
                (true, json!({ "target": "all" }))
            } else {
                (state.attached, this.clone())
            };
            fill.push(self.run(key, enabled, command, args));
        }
        if state.knows(FILL_CAPTURE) {
            if !fill.is_empty() {
                fill.push(Item::Separator);
            }
            fill.push(self.run(
                "contextmenu.tools.fill_capture",
                state.attached,
                FILL_CAPTURE,
                this,
            ));
        }
        if !fill.is_empty() {
            items.push(self.submenu(Label::Key(FILL_MENU), fill));
        }
        (!items.is_empty()).then(|| self.submenu(Label::Key(TOOLS_MENU), items))
    }
}

#[cfg(test)]
#[path = "view_entries_tests.rs"]
mod tests;
