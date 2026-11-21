// RB-094-03: the page menu's Screenshot submenu compares the clicked view's page in the
// engines, once the command is registered.

use std::collections::BTreeSet;

use serde_json::json;

use super::*;
use crate::engine_compare::defs::ENGINES_COMMAND;

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

/// The entries of the Screenshot submenu, wherever it is nested (Capture > Screenshot).
fn screenshot(items: &[Item]) -> Vec<Entry> {
    items
        .iter()
        .find_map(|item| match item {
            Item::Menu {
                label: Label::Key(KEY_SCREENSHOT),
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
            Item::Menu { items, .. } => Some(screenshot(items)).filter(|found| !found.is_empty()),
            _ => None,
        })
        .unwrap_or_default()
}

#[test]
fn the_screenshot_submenu_compares_the_clicked_views_page_in_the_engines() {
    let shown = screenshot(&build(&state(&[ENGINES_COMMAND])));
    let compare = shown
        .iter()
        .find(|entry| entry.label == Label::Key("contextmenu.screenshot.compare_engines"))
        .expect("offered");
    assert_eq!(
        compare.action,
        Action::Run {
            command: ENGINES_COMMAND.to_owned(),
            args: json!({ "view": 2 })
        }
    );
    assert!(compare.enabled);
}

#[test]
fn without_its_command_the_comparison_is_not_offered() {
    let shown = screenshot(&build(&state(&[])));
    assert!(!shown.is_empty(), "the submenu is there");
    assert!(
        shown
            .iter()
            .all(|entry| entry.label != Label::Key("contextmenu.screenshot.compare_engines"))
    );
}
