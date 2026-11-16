// RB-080-60: the footer under every view is switched from the page's context menu too - a check
// entry of rBrowser's block that runs `ui.view_footer.toggle`.

use std::collections::BTreeSet;

use serde_json::Value;

use super::*;
use crate::view_footer::defs::TOGGLE;

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
        known: [TOGGLE].into_iter().map(str::to_owned).collect(),
        footer_visible,
        forced: crate::force_state::ForcedMarks::default(),
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

fn footer(items: &[Item]) -> Option<&Entry> {
    items.iter().find_map(|item| match item {
        Item::Entry(entry) if entry.label == Label::Key(KEY_FOOTER) => Some(entry),
        _ => None,
    })
}

#[test]
fn the_footer_is_a_check_entry_after_the_panels_that_runs_the_toggle() {
    let items = build(&state(true));
    assert_eq!(
        top(&items),
        [
            "contextmenu.inspect",
            "contextmenu.devtools_open",
            "contextmenu.screenshot.menu>",
            "contextmenu.video.menu>",
            "contextmenu.checks.menu>",
            "contextmenu.panels.menu>",
            "contextmenu.view_footer=true",
            "-",
            "sync.menu.skipNextClick",
            "popout.menu.out",
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
    // `ui.pageContextMenu` off leaves the pre-0.8 entries only.
    let mut off = state(true);
    off.menu_enabled = false;
    assert!(footer(&build(&off)).is_none());
}
