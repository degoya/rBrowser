//! The model of the page's context menu (RB-080-43): the rBrowser block below Chromium's
//! entries, built from the core's state at the moment the menu opens - entries, submenus, state
//! texts (Open/Close DevTools, Record/Stop) and what is enabled. Pure: no CEF, no core; the
//! CEF side (`cef_app/context_menu.rs`) renders the items and runs the chosen entry's action
//! through [`super::run`].
//!
//! Entry ids are CEF menu command ids in the user range: `FIRST_ID` and up, numbered in menu
//! order at build time (`MENU_ID_USER_FIRST` 26500 to `MENU_ID_USER_LAST` 28500; the size quick
//! pick's rows live below 28 000, the pre-0.8 entries at 28 000 and 28 001). Submenus take ids
//! of their own at build time too, from `FIRST_MENU_ID` to `LAST_MENU_ID` below the entries.

use std::collections::BTreeSet;

use rb_engine::ViewId;
use serde_json::{Value, json};

/// The first command id of the block.
pub const FIRST_ID: i32 = 28_010;
/// The first id of the block's submenus. A submenu is never chosen, but its id is its own - a
/// menu looks an item up by its command id, and an id shared with an entry would name two
/// items.
pub const FIRST_MENU_ID: i32 = 27_900;
/// The last id of the block's submenus: a hundred of them, below the pre-0.8 entries.
pub const LAST_MENU_ID: i32 = 27_999;
/// The last command id the block may take (CEF's `MENU_ID_USER_LAST`).
pub const LAST_ID: i32 = 28_500;

/// The analyzer's command.
const ANALYZE: &str = "breakpoints.analyze";
/// The footer's check entry (RB-080-60).
pub const KEY_FOOTER: &str = "contextmenu.view_footer";

/// A label: an i18n key of the catalogue, a text from outside it (a plugin's title, a user's
/// preset name, a time zone - rendered through [`plain_label`]), or a key built at run time
/// with the text to show when the catalogue has none.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Label {
    Key(&'static str),
    Text(String),
    KeyOr { key: String, text: String },
}

/// `text` as a menu label: `&` doubled (a single one marks the mnemonic, the letter after it
/// underlined and the `&` gone) and control characters dropped. For every label that does not
/// come from the catalogue.
#[must_use]
pub fn plain_label(text: &str) -> String {
    text.chars()
        .filter(|character| !character.is_control())
        .flat_map(|character| std::iter::repeat_n(character, if character == '&' { 2 } else { 1 }))
        .collect()
}

/// How an entry is marked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    /// A plain entry.
    None,
    /// A switch: a check item, checked or not.
    Check(bool),
    /// One of a submenu's choices: a radio item, chosen or not.
    Radio(bool),
}

/// What an entry does when chosen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    /// Runs the registry's command with these arguments on the core's runtime.
    Run { command: String, args: Value },
    /// Makes the view active and runs `sync.skip_next` (RB-020-05).
    SkipNextClick,
}

/// One entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub id: i32,
    pub label: Label,
    pub enabled: bool,
    pub mark: Mark,
    pub action: Action,
}

/// One item of a menu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Item {
    Entry(Entry),
    Separator,
    Menu {
        id: i32,
        label: Label,
        items: Vec<Item>,
    },
}

/// A check command a plugin hangs into the menu (`context_menu` of its manifest).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginCommand {
    /// The command id (`plugin.<id>.<command>`).
    pub command: String,
    /// Its title in the UI language.
    pub title: String,
}

/// A plugin panel the panels window offers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginPanel {
    pub plugin: String,
    pub panel: String,
    pub title: String,
}

/// What the menu is built from: the clicked view and the core's state when it opened.
#[derive(Clone, Debug, PartialEq)]
pub struct MenuState {
    /// The clicked view.
    pub view: ViewId,
    /// The click, in DIP of the view's widget; `None` when CEF gave no parameters - the
    /// point-based entries are disabled then.
    pub point: Option<(i32, i32)>,
    /// `ui.pageContextMenu`: off leaves Chromium's menu and "Do not sync the next click".
    pub menu_enabled: bool,
    /// The view has its CDP session: DevTools, inspect and captures of this view can run.
    pub attached: bool,
    /// The view is in a window of its own (RB-080-14).
    pub detached: bool,
    /// DevTools are open for this view.
    pub devtools_open: bool,
    /// `devtools.openOnInspect`: off, "Inspect element" needs DevTools open for this view.
    pub open_on_inspect: bool,
    /// The view's emulation for the Emulation submenu and the top-level "Analyze breakpoints"
    /// (RB-080-49); `None` with `ui.pageContextMenuEmulation` off.
    pub emulation: Option<super::emulation::EmulationState>,
    /// The views of the running recording, if one runs (one recording at a time).
    pub recording: Option<Vec<ViewId>>,
    /// A check run is in progress.
    pub checks_busy: bool,
    /// The check plugins' entries, in load order.
    pub checks: Vec<PluginCommand>,
    /// The plugins' panels, as the panels window offers them.
    pub panels: Vec<PluginPanel>,
    /// The commands the registry knows: optional entries appear only when their command does.
    pub known: BTreeSet<String>,
    /// `ui.viewFooter.visible`: the footer entry's check mark (RB-080-60).
    pub footer_visible: bool,
}

impl MenuState {
    fn knows(&self, command: &str) -> bool {
        self.known.contains(command)
    }
}

/// Builds the block for `state`; the ids are numbered from [`FIRST_ID`] in menu order.
#[must_use]
pub fn build(state: &MenuState) -> Vec<Item> {
    let mut builder = Builder {
        state,
        next_id: FIRST_ID,
        next_menu_id: FIRST_MENU_ID,
    };
    builder.menu()
}

/// The entry with `id` among `items`, submenus included.
#[must_use]
pub fn find(items: &[Item], id: i32) -> Option<&Entry> {
    items.iter().find_map(|item| match item {
        Item::Entry(entry) if entry.id == id => Some(entry),
        Item::Entry(_) | Item::Separator => None,
        Item::Menu { items, .. } => find(items, id),
    })
}

pub(super) struct Builder<'a> {
    pub(super) state: &'a MenuState,
    next_id: i32,
    next_menu_id: i32,
}

impl Builder<'_> {
    pub(super) fn entry(&mut self, key: &'static str, enabled: bool, action: Action) -> Item {
        self.labelled(Label::Key(key), enabled, Mark::None, action)
    }

    /// A submenu with the next id of the submenus' range. Past the range the last id repeats;
    /// a test holds the fullest block inside it.
    pub(super) fn submenu(&mut self, label: Label, items: Vec<Item>) -> Item {
        let id = self.next_menu_id.min(LAST_MENU_ID);
        self.next_menu_id = self.next_menu_id.saturating_add(1);
        Item::Menu { id, label, items }
    }

    pub(super) fn labelled(
        &mut self,
        label: Label,
        enabled: bool,
        mark: Mark,
        action: Action,
    ) -> Item {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        Item::Entry(Entry {
            id,
            label,
            // Past CEF's user range nothing can be chosen: the entry is shown, disabled.
            enabled: enabled && id <= LAST_ID,
            mark,
            action,
        })
    }

    pub(super) fn run(
        &mut self,
        key: &'static str,
        enabled: bool,
        command: &str,
        args: Value,
    ) -> Item {
        self.entry(
            key,
            enabled,
            Action::Run {
                command: command.to_owned(),
                args,
            },
        )
    }

    fn this_view(&self) -> Value {
        json!({ "target": self.state.view })
    }

    fn menu(&mut self) -> Vec<Item> {
        let state = self.state;
        let mut items = Vec::new();
        if state.menu_enabled {
            items.push(self.inspect());
            items.push(self.devtools());
            items.push(self.screenshot());
            items.push(self.video());
            items.push(self.checks());
            items.push(self.panels());
            if let Some(emulation) = &state.emulation {
                items.push(self.emulation(emulation));
                if state.knows(ANALYZE) {
                    items.push(self.analyze());
                }
            }
            if state.knows(crate::view_footer::defs::TOGGLE) {
                items.push(self.footer());
            }
            items.push(Item::Separator);
        }
        items.push(self.entry("sync.menu.skipNextClick", true, Action::SkipNextClick));
        // RB-080-14's and RB-080-41's entries stay whatever the setting says.
        let (key, command) = if state.detached {
            ("popout.menu.in", crate::pop_out::POP_IN)
        } else {
            ("popout.menu.out", crate::pop_out::POP_OUT)
        };
        items.push(self.run(key, true, command, json!({ "view": state.view })));
        // RB-080-41 defines `view.remove`; the entry waits for it.
        if state.knows("view.remove") {
            items.push(self.run(
                "contextmenu.remove_view",
                true,
                "view.remove",
                json!({ "view": state.view }),
            ));
        }
        items
    }

    fn inspect(&mut self) -> Item {
        let state = self.state;
        // Off, inspect neither opens nor moves DevTools: it runs only where they are open.
        let may_show = state.open_on_inspect || state.devtools_open;
        let (enabled, args) = match state.point {
            Some((x, y)) => (
                state.attached && may_show,
                json!({ "view": self.state.view, "x": x, "y": y, "dip": true }),
            ),
            None => (false, json!({ "view": self.state.view })),
        };
        self.run("contextmenu.inspect", enabled, "devtools.inspect_at", args)
    }

    fn devtools(&mut self) -> Item {
        if self.state.devtools_open {
            self.run(
                "contextmenu.devtools_close",
                true,
                "devtools.toggle",
                json!({}),
            )
        } else {
            let args = self.this_view();
            self.run(
                "contextmenu.devtools_open",
                self.state.attached,
                "devtools.open",
                args,
            )
        }
    }

    fn screenshot(&mut self) -> Item {
        let attached = self.state.attached;
        let this = self.this_view();
        let (element_enabled, element) = match self.state.point {
            Some((x, y)) => (
                attached,
                json!({ "target": self.state.view, "at": { "x": x, "y": y, "dip": true } }),
            ),
            None => (false, this.clone()),
        };
        let items = vec![
            self.run(
                "contextmenu.screenshot.this_viewport",
                attached,
                "capture.views",
                this.clone(),
            ),
            self.run(
                "contextmenu.screenshot.this_full_page",
                attached,
                "capture.full_page",
                this,
            ),
            self.run(
                "contextmenu.screenshot.element",
                element_enabled,
                "capture.element",
                element,
            ),
            self.run(
                "contextmenu.screenshot.all_viewport",
                true,
                "capture.all_viewport",
                json!({}),
            ),
            self.run(
                "contextmenu.screenshot.all_full_page",
                true,
                "capture.full_page",
                json!({}),
            ),
            self.run(
                "contextmenu.screenshot.overview",
                true,
                "capture.overview",
                json!({}),
            ),
        ];
        self.submenu(Label::Key("contextmenu.screenshot.menu"), items)
    }

    fn video(&mut self) -> Item {
        let view = self.state.view;
        let this = self.this_view();
        let (this_key, this_enabled) = match &self.state.recording {
            Some(views) if views.contains(&view) => ("contextmenu.video.stop", true),
            // One recording at a time: another view's blocks this one.
            Some(_) => ("contextmenu.video.record", false),
            None => ("contextmenu.video.record", self.state.attached),
        };
        let all_key = if self.state.recording.is_some() {
            "contextmenu.video.stop_all"
        } else {
            "contextmenu.video.record_all"
        };
        let items = vec![
            self.run(this_key, this_enabled, crate::video::defs::TOGGLE, this),
            self.run(all_key, true, crate::video::defs::TOGGLE_ALL, json!({})),
        ];
        self.submenu(Label::Key("contextmenu.video.menu"), items)
    }

    fn checks(&mut self) -> Item {
        let state = self.state;
        let busy = state.checks_busy;
        let mut items = vec![self.run(
            "contextmenu.checks.run_all",
            !busy,
            crate::checks::RUN_ALL_COMMAND,
            json!({}),
        )];
        for check in &state.checks {
            let args = self.this_view();
            items.push(self.labelled(
                Label::Text(check.title.clone()),
                state.attached,
                Mark::None,
                Action::Run {
                    command: check.command.clone(),
                    args,
                },
            ));
        }
        if state.knows(crate::vitals::defs::VITALS) {
            items.push(self.run(
                "contextmenu.checks.vitals",
                true,
                crate::vitals::defs::VITALS,
                json!({}),
            ));
        }
        // With the Emulation block (RB-080-49) the analyzer is an entry of the menu itself.
        if state.emulation.is_none() && state.knows(ANALYZE) {
            items.push(self.analyze());
        }
        self.submenu(Label::Key("contextmenu.checks.menu"), items)
    }

    /// The analyzer's window, as the chord opens it: only `Null` does (a value is a detection
    /// request).
    fn analyze(&mut self) -> Item {
        self.run("contextmenu.checks.analyze", true, ANALYZE, Value::Null)
    }

    /// "Footer under every view", checked while it shows: the same switch as `Leader O B`.
    fn footer(&mut self) -> Item {
        let action = Action::Run {
            command: crate::view_footer::defs::TOGGLE.to_owned(),
            args: Value::Null,
        };
        let mark = Mark::Check(self.state.footer_visible);
        self.labelled(Label::Key(KEY_FOOTER), true, mark, action)
    }

    fn panels(&mut self) -> Item {
        let state = self.state;
        let mut items = vec![self.run(
            "contextmenu.panels.findings",
            true,
            "panels.findings",
            json!({}),
        )];
        if state.knows(crate::sitemap::defs::PANEL) {
            items.push(self.run(
                "contextmenu.panels.sitemap",
                true,
                crate::sitemap::defs::PANEL,
                json!({}),
            ));
        }
        for panel in &state.panels {
            let args = json!({ "plugin": panel.plugin, "panel": panel.panel, "view": state.view });
            items.push(self.labelled(
                Label::Text(panel.title.clone()),
                true,
                Mark::None,
                Action::Run {
                    command: rb_plugin_host::PANELS_COMMAND.to_owned(),
                    args,
                },
            ));
        }
        self.submenu(Label::Key("contextmenu.panels.menu"), items)
    }
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "model_footer_tests.rs"]
mod footer_tests;
