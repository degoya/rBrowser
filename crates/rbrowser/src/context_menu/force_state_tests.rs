use std::collections::BTreeSet;

use serde_json::json;

use rb_engine::ViewId;

use super::*;
use crate::context_menu::model::{Entry, MenuState, build};
use crate::force_state::PseudoClass;

fn with_force_state(forced: ForcedMarks) -> MenuState {
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
        known: [SET.to_owned()].into_iter().collect::<BTreeSet<String>>(),
        footer_visible: false,
        vitals_bar_visible: false,
        lighthouse_bar_visible: false,
        forced,
        recorder: crate::context_menu::recorder::RecorderMenu::default(),
        view_entries: crate::context_menu::view_entries::ViewEntries::default(),
    }
}

fn text(label: &Label) -> String {
    match label {
        Label::Key(key) => (*key).to_owned(),
        Label::Text(text) => format!("'{text}'"),
        Label::KeyOr { key, .. } => key.clone(),
    }
}

/// The submenu labelled `key` at any depth: Force state hangs under Tools (RB-090-92).
fn find_menu<'a>(items: &'a [Item], key: &str) -> Option<&'a [Item]> {
    items.iter().find_map(|item| match item {
        Item::Menu { label, items, .. } if text(label) == key => Some(items.as_slice()),
        Item::Menu { items, .. } => find_menu(items, key),
        _ => None,
    })
}

fn submenu<'a>(items: &'a [Item], key: &str) -> &'a [Item] {
    find_menu(items, key).unwrap_or_else(|| panic!("no submenu {key}"))
}

fn entries(items: &[Item]) -> Vec<&Entry> {
    items
        .iter()
        .filter_map(|item| match item {
            Item::Entry(entry) => Some(entry),
            _ => None,
        })
        .collect()
}

/// The menu as lines of `[x]`/`[ ]` and labels; a submenu as `label>`.
fn lines(items: &[Item]) -> Vec<String> {
    items
        .iter()
        .map(|item| match item {
            Item::Entry(entry) => match entry.mark {
                Mark::Check(true) => format!("[x] {}", text(&entry.label)),
                Mark::Check(false) => format!("[ ] {}", text(&entry.label)),
                _ => text(&entry.label),
            },
            Item::Separator => "-".to_owned(),
            Item::Menu { label, .. } => format!("{}>", text(label)),
        })
        .collect()
}

#[test]
fn the_submenu_offers_every_state_a_reset_and_all_views() {
    let forced = ForcedMarks {
        this_view: [PseudoClass::Hover].into_iter().collect(),
        all_views: BTreeSet::new(),
        any_view: true,
    };
    let items = build(&with_force_state(forced));
    let menu = submenu(&items, KEY_MENU);
    let shown = lines(menu);
    assert_eq!(
        shown[..9],
        [
            "[x] ':hover'",
            "[ ] ':active'",
            "[ ] ':focus'",
            "[ ] ':focus-visible'",
            "[ ] ':focus-within'",
            "[ ] ':visited'",
            "[ ] ':target'",
            "-",
            KEY_RESET,
        ]
    );
    assert_eq!(shown[9..], [format!("{KEY_ALL}>")]);
    let all = submenu(menu, KEY_ALL);
    assert_eq!(lines(all)[0], "[ ] ':hover'", "not every view hovers");
    let reset_all = entries(all).last().copied().expect("the reset");
    assert!(reset_all.enabled, "some view has a forced state");
}

/// Each switch toggles its state on the element under the click, in DIP of the clicked view;
/// "All views" with `target: "all"`.
#[test]
fn a_switch_toggles_its_state_at_the_click_for_this_view_or_all() {
    let items = build(&with_force_state(ForcedMarks::default()));
    let menu = submenu(&items, KEY_MENU);
    let hover = entries(menu)[0];
    assert!(hover.enabled);
    assert_eq!(
        hover.action,
        Action::Run {
            command: SET.to_owned(),
            args: json!({
                "target": 2, "view": 2, "x": 30, "y": 40, "dip": true, "toggle": "hover",
            }),
        }
    );
    let reset = entries(menu)[7];
    assert_eq!(
        reset.action,
        Action::Run {
            command: RESET.to_owned(),
            args: json!({ "target": 2 }),
        }
    );
    assert!(!reset.enabled, "nothing forced here");
    let all = submenu(menu, KEY_ALL);
    let visited = entries(all)[5];
    assert_eq!(
        visited.action,
        Action::Run {
            command: SET.to_owned(),
            args: json!({
                "target": "all", "view": 2, "x": 30, "y": 40, "dip": true, "toggle": "visited",
            }),
        }
    );
    assert!(!entries(all)[7].enabled, "no view has a forced state");
}

#[test]
fn without_a_point_or_a_session_the_switches_are_disabled_the_reset_is_not() {
    let forced = ForcedMarks {
        this_view: [PseudoClass::Focus].into_iter().collect(),
        all_views: [PseudoClass::Focus].into_iter().collect(),
        any_view: true,
    };
    let mut state = with_force_state(forced.clone());
    state.point = None;
    let items = build(&state);
    let menu = submenu(&items, KEY_MENU);
    assert!(entries(menu)[..7].iter().all(|entry| !entry.enabled));
    assert!(entries(menu)[7].enabled, "a reset needs no element");
    let mut state = with_force_state(forced);
    state.attached = false;
    let items = build(&state);
    let menu = submenu(&items, KEY_MENU);
    assert!(entries(menu)[..7].iter().all(|entry| !entry.enabled));
    assert_eq!(lines(submenu(menu, KEY_ALL))[2], "[x] ':focus'");
}

#[test]
fn the_submenu_waits_for_its_command() {
    let mut state = with_force_state(ForcedMarks::default());
    state.known.remove(SET);
    let items = build(&state);
    assert!(find_menu(&items, KEY_MENU).is_none());
}

/// Force state is the first entry of Tools, as the main menu's Tools hold the tools (RB-090-92).
#[test]
fn force_state_hangs_under_tools() {
    let items = build(&with_force_state(ForcedMarks::default()));
    let tools = submenu(&items, crate::context_menu::model::KEY_TOOLS);
    assert_eq!(lines(tools), [format!("{KEY_MENU}>")]);
    assert!(
        items
            .iter()
            .all(|item| !matches!(item, Item::Menu { label, .. } if text(label) == KEY_MENU)),
        "not on the top level"
    );
}
