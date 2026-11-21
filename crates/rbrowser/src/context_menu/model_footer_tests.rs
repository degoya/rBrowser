// RB-080-60: the footer under every view is switched from the page's context menu too - a check
// entry of rBrowser's block that runs `ui.view_footer.toggle`.

use std::collections::BTreeSet;

use serde_json::Value;

use super::*;
use crate::view_footer::defs::{TOGGLE, VITALS_TOGGLE};

fn state(footer_visible: bool) -> MenuState {
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
        known: [TOGGLE, VITALS_TOGGLE]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        footer_visible,
        vitals_bar_visible: footer_visible,
        lighthouse_bar_visible: footer_visible,
        forced: crate::force_state::ForcedMarks::default(),
        recorder: crate::context_menu::recorder::RecorderMenu::default(),
        view_entries: crate::context_menu::view_entries::ViewEntries::default(),
    }
}

/// The top level of the block: an entry's key with its mark, a submenu's key with `>`.
fn top(items: &[Item]) -> Vec<String> {
    items
        .iter()
        .map(|item| match item {
            Item::Entry(Entry {
                label: Label::Key(key),
                mark: Mark::Check(checked),
                ..
            }) => format!("{key}={checked}"),
            Item::Entry(Entry {
                label: Label::Key(key),
                ..
            }) => (*key).to_owned(),
            Item::Menu {
                label: Label::Key(key),
                ..
            } => format!("{key}>"),
            Item::Separator => "-".to_owned(),
            other => format!("{other:?}"),
        })
        .collect()
}

/// The View submenu's items, where the bars' switches hang (RB-090-92); none without it.
fn view_menu(items: &[Item]) -> &[Item] {
    items
        .iter()
        .find_map(|item| match item {
            Item::Menu {
                label: Label::Key(KEY_VIEW),
                items,
                ..
            } => Some(items.as_slice()),
            _ => None,
        })
        .unwrap_or_default()
}

fn footer(items: &[Item]) -> Option<&Entry> {
    view_menu(items).iter().find_map(|item| match item {
        Item::Entry(entry) if entry.label == Label::Key(KEY_FOOTER) => Some(entry),
        _ => None,
    })
}

fn vitals_bar(items: &[Item]) -> Option<&Entry> {
    view_menu(items).iter().find_map(|item| match item {
        Item::Entry(entry) if entry.label == Label::Key(KEY_VITALS_BAR) => Some(entry),
        _ => None,
    })
}

/// The bars hang under View as in the main menu (RB-090-92).
#[test]
fn the_footer_is_a_check_entry_of_the_view_submenu_that_runs_the_toggle() {
    let items = build(&state(true));
    assert_eq!(
        top(&items),
        [
            "contextmenu.inspect",
            "contextmenu.devtools=false",
            "-",
            "shell.menu.view>",
            "shell.menu.capture>",
            "shell.menu.checks>",
            "shell.menu.panels>",
            "-",
            "sync.menu.skipNextClick",
            "popout.menu.out",
        ]
    );
    assert_eq!(
        top(view_menu(&items)),
        [
            "contextmenu.view_footer=true",
            "contextmenu.vitals_bar=true"
        ]
    );
    let entry = footer(&items).expect("the footer entry");
    assert!(entry.enabled);
    assert_eq!(
        entry.action,
        Action::Run {
            command: TOGGLE.to_owned(),
            args: Value::Null,
        }
    );
    // Hidden, the entry is there unchecked: a click shows the footer again.
    let hidden = build(&state(false));
    assert_eq!(
        footer(&hidden).map(|entry| entry.mark),
        Some(Mark::Check(false))
    );
}

#[test]
fn the_footer_entry_waits_for_its_command_and_the_block() {
    let mut unknown = state(true);
    unknown.known = BTreeSet::new();
    assert!(footer(&build(&unknown)).is_none());
    assert!(vitals_bar(&build(&unknown)).is_none());
    // `ui.pageContextMenu` off leaves the pre-0.8 entries only.
    let mut off = state(true);
    off.menu_enabled = false;
    assert!(footer(&build(&off)).is_none());
}

// RB-090-71: the vitals bar is switched beside the breakpoint bar, a check entry of its own.
#[test]
fn the_vitals_bar_is_a_check_entry_beside_the_breakpoint_bar() {
    let mut on = state(true);
    on.vitals_bar_visible = true;
    on.footer_visible = false;
    let items = build(&on);
    let entry = vitals_bar(&items).expect("the vitals bar entry");
    assert_eq!(entry.mark, Mark::Check(true));
    assert!(entry.enabled);
    assert_eq!(
        entry.action,
        Action::Run {
            command: VITALS_TOGGLE.to_owned(),
            args: Value::Null,
        }
    );
    assert_eq!(
        footer(&items).map(|entry| entry.mark),
        Some(Mark::Check(false)),
        "each entry checks its own bar"
    );
    let mut off = state(true);
    off.vitals_bar_visible = false;
    assert_eq!(
        vitals_bar(&build(&off)).map(|entry| entry.mark),
        Some(Mark::Check(false))
    );
    // Without its command only the breakpoint bar's entry is there.
    let mut unknown = state(true);
    unknown.known = [TOGGLE].into_iter().map(str::to_owned).collect();
    assert!(vitals_bar(&build(&unknown)).is_none());
    assert!(footer(&build(&unknown)).is_some());
}

/// DevTools is a check entry like the bars: checked and closing while open, unchecked and
/// opening the clicked view's DevTools while closed (owner, 2026-11-17).
#[test]
fn devtools_is_a_check_entry_of_its_open_state() {
    let devtools = |items: &[Item]| {
        items.iter().find_map(|item| match item {
            Item::Entry(entry) if entry.label == Label::Key("contextmenu.devtools") => {
                Some(entry.clone())
            }
            _ => None,
        })
    };
    let mut menu = state(true);
    let closed = devtools(&build(&menu)).expect("the DevTools entry");
    assert_eq!(closed.mark, Mark::Check(false));
    assert_eq!(
        closed.action,
        Action::Run {
            command: "devtools.open".to_owned(),
            args: serde_json::json!({ "target": 2 }),
        }
    );
    menu.devtools_open = true;
    let open = devtools(&build(&menu)).expect("the DevTools entry");
    assert_eq!(open.mark, Mark::Check(true));
    assert!(open.enabled);
    assert_eq!(
        open.action,
        Action::Run {
            command: "devtools.toggle".to_owned(),
            args: serde_json::json!({}),
        }
    );
}

/// Inspect element and DevTools; the submenus as the main menu has them (RB-090-92); the
/// view's own entries - each set apart by one separator, no empty group.
#[test]
fn the_page_menu_is_grouped_with_one_separator_between_groups() {
    let mut full = state(true);
    full.known.insert(crate::force_state::SET.to_owned());
    assert_eq!(
        top(&build(&full)),
        [
            "contextmenu.inspect",
            "contextmenu.devtools=false",
            "-",
            "shell.menu.view>",
            "shell.menu.capture>",
            "shell.menu.checks>",
            "shell.menu.tools>",
            "shell.menu.panels>",
            "-",
            "sync.menu.skipNextClick",
            "popout.menu.out",
        ]
    );
}

// RB-090-73: the Lighthouse bar is switched after the vitals bar, a check entry of its own once
// its command is registered.
#[test]
fn the_lighthouse_bar_is_a_check_entry_after_the_vitals_bar() {
    let mut on = state(true);
    on.known
        .insert(crate::view_footer::defs::LIGHTHOUSE_TOGGLE.to_owned());
    on.lighthouse_bar_visible = false;
    let items = build(&on);
    let keys = top(view_menu(&items));
    let at = |key: &str| keys.iter().position(|shown| shown.starts_with(key));
    assert_eq!(
        at("contextmenu.lighthouse_bar"),
        at("contextmenu.vitals_bar").map(|vitals| vitals + 1),
        "{keys:?}"
    );
    let entry = view_menu(&items)
        .iter()
        .find_map(|item| match item {
            Item::Entry(entry) if entry.label == Label::Key(KEY_LIGHTHOUSE_BAR) => Some(entry),
            _ => None,
        })
        .expect("the Lighthouse bar entry");
    assert_eq!(entry.mark, Mark::Check(false), "its own switch");
    assert_eq!(
        entry.action,
        Action::Run {
            command: crate::view_footer::defs::LIGHTHOUSE_TOGGLE.to_owned(),
            args: Value::Null,
        }
    );
}
