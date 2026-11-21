// RB-090-63: the page menu's Panels submenu opens the Videos panel too, once its command is
// registered.

use std::collections::BTreeSet;

use serde_json::json;

use super::*;
use crate::video::defs::PANEL;

fn state(known: &[&str]) -> MenuState {
    MenuState {
        view: ViewId(2),
        point: Some((30, 40)),
        menu_enabled: true,
        attached: true,
        detached: false,
        devtools_open: false,
        open_on_inspect: true,
        emulation: None,
        recording: None,
        checks_busy: false,
        checks: Vec::new(),
        panels: Vec::new(),
        known: known
            .iter()
            .map(|id| (*id).to_owned())
            .collect::<BTreeSet<_>>(),
        footer_visible: true,
        vitals_bar_visible: true,
        lighthouse_bar_visible: true,
        forced: crate::force_state::ForcedMarks::default(),
        recorder: crate::context_menu::recorder::RecorderMenu::default(),
        view_entries: crate::context_menu::view_entries::ViewEntries::default(),
    }
}

/// The entries of the Panels submenu.
fn panels(items: &[Item]) -> Vec<Entry> {
    items
        .iter()
        .find_map(|item| match item {
            Item::Menu {
                label: Label::Key("shell.menu.panels"),
                items,
                ..
            } => Some(
                items
                    .iter()
                    .filter_map(|item| match item {
                        Item::Entry(entry) => Some(entry.clone()),
                        _ => None,
                    })
                    .collect(),
            ),
            _ => None,
        })
        .unwrap_or_default()
}

#[test]
fn the_panels_submenu_opens_the_videos_panel() {
    let shown = panels(&build(&state(&[PANEL])));
    let videos = shown
        .iter()
        .find(|entry| entry.label == Label::Key("contextmenu.panels.videos"))
        .expect("the Videos panel is in the Panels submenu");
    assert_eq!(
        videos.action,
        Action::Run {
            command: PANEL.to_owned(),
            args: json!({})
        }
    );
    assert!(videos.enabled);
}

#[test]
fn without_its_command_the_videos_panel_is_not_offered() {
    let shown = panels(&build(&state(&[])));
    assert!(
        shown
            .iter()
            .all(|entry| entry.label != Label::Key("contextmenu.panels.videos"))
    );
}
