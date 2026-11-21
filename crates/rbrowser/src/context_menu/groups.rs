//! The rBrowser block's submenus, grouped as the main menu groups the same functions (RB-090-92,
//! owner, 2026-11-20): each submenu carries the key of the main menu's submenu that holds those
//! functions, so its text is the same, and they stand in the main menu's order. Inside, the
//! main menu's grouping holds (Capture > Screenshot, Capture > Video, Flows > Recorder,
//! Environments > Sessions); a level that would hold one entry only is left out (Layout >
//! Analyze breakpoints, not Layout > Breakpoints > Analyze). An empty submenu is left out.
//! What hangs on the point under the click and on the view itself stays flat ([`super::build`]).

use super::{ANALYZE, Builder, Item, Label};

/// The submenus of the main menu in its order: a mirror of `MAIN_MENU` in
/// `ui/src/menu/mainMenu.ts` (a test holds the two together). The block's submenus are named by
/// these keys and stand in this order; not every one has a counterpart in the page's menu.
pub const MAIN_MENU: [&str; 14] = [
    KEY_VIEW,
    "shell.menu.devices",
    KEY_LAYOUT,
    KEY_SYNC,
    KEY_EMULATION,
    KEY_ENVIRONMENTS,
    KEY_CAPTURE,
    KEY_CHECKS,
    "shell.menu.overlays",
    KEY_TOOLS,
    KEY_FLOWS,
    KEY_PANELS,
    "shell.menu.plugins",
    "shell.menu.app",
];

/// View: the bars under every view.
pub const KEY_VIEW: &str = "shell.menu.view";
/// Layout: the breakpoint analyzer.
pub const KEY_LAYOUT: &str = "shell.menu.layout";
/// Sync: this view and its navigation.
pub const KEY_SYNC: &str = "shell.menu.sync";
/// Emulation: the toolbar's switches for the clicked view.
pub const KEY_EMULATION: &str = "shell.menu.emulation";
/// Environments: the view's session.
pub const KEY_ENVIRONMENTS: &str = "shell.menu.environments";
/// Environments > Sessions.
pub const KEY_SESSIONS: &str = "shell.menu.sessions";
/// Capture: Screenshot and Video.
pub const KEY_CAPTURE: &str = "shell.menu.capture";
/// Capture > Screenshot.
pub const KEY_SCREENSHOT: &str = "shell.menu.screenshot";
/// Capture > Video.
pub const KEY_VIDEO: &str = "shell.menu.video";
/// Checks.
pub const KEY_CHECKS: &str = "shell.menu.checks";
/// Tools: Force state and the plugins' tools.
pub const KEY_TOOLS: &str = "shell.menu.tools";
/// Flows: Run flow and the recorder.
pub const KEY_FLOWS: &str = "shell.menu.flows";
/// Flows > Recorder.
pub const KEY_RECORDER: &str = "shell.menu.recorder";
/// Panels.
pub const KEY_PANELS: &str = "shell.menu.panels";

impl Builder<'_> {
    /// A submenu labelled `key` with `items`, or nothing when there are none.
    fn group(&mut self, key: &'static str, items: Vec<Item>) -> Option<Item> {
        (!items.is_empty()).then(|| self.submenu(Label::Key(key), items))
    }

    /// The block's submenus in the main menu's order, built in menu order so the entries' ids
    /// follow it.
    pub(super) fn groups(&mut self) -> Vec<Item> {
        let state = self.state;
        let mut menus = Vec::new();
        let view = self.bars();
        menus.extend(self.group(KEY_VIEW, view));
        let layout = if state.knows(ANALYZE) {
            vec![self.analyze()]
        } else {
            Vec::new()
        };
        menus.extend(self.group(KEY_LAYOUT, layout));
        menus.extend(self.sync_menu());
        if let Some(emulation) = &state.emulation {
            menus.push(self.emulation(emulation));
        }
        let environments = self.session_menu().into_iter().collect();
        menus.extend(self.group(KEY_ENVIRONMENTS, environments));
        let mut capture = vec![self.screenshot(), self.video()];
        // The bug report of every view (RB-093-04).
        if state.knows(crate::bug_report::CREATE) {
            capture.push(self.run(
                "contextmenu.bug_report",
                true,
                crate::bug_report::CREATE,
                serde_json::json!({}),
            ));
        }
        menus.extend(self.group(KEY_CAPTURE, capture));
        menus.push(self.checks());
        let mut tools = Vec::new();
        if state.knows(crate::force_state::SET) {
            tools.push(self.force_state(&state.forced));
        }
        tools.extend(self.tools_entries());
        menus.extend(self.group(KEY_TOOLS, tools));
        let mut flows = Vec::new();
        if state.knows(crate::flows::RUN) {
            flows.push(self.flows());
        }
        if state.knows(crate::recorder::CHECK) {
            flows.push(self.recorder());
        }
        menus.extend(self.group(KEY_FLOWS, flows));
        menus.push(self.panels());
        menus
    }
}

#[cfg(test)]
#[path = "groups_tests.rs"]
mod tests;
