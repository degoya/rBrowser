use std::collections::BTreeSet;

use rb_engine::ViewId;
use serde_json::{Value, json};

use super::*;
use crate::context_menu::model::{Entry, MenuState, build};

fn with_recorder(recorder: RecorderMenu, point: Option<(i32, i32)>) -> MenuState {
    MenuState {
        view: ViewId(2),
        point,
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
        known: [crate::recorder::CHECK, crate::flows::RUN]
            .into_iter()
            .map(str::to_owned)
            .collect::<BTreeSet<String>>(),
        footer_visible: false,
        vitals_bar_visible: false,
        lighthouse_bar_visible: false,
        forced: crate::force_state::ForcedMarks::default(),
        recorder,
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

/// The submenu labelled `key` at any depth: Run flow and Recorder hang under Flows
/// (RB-090-92).
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

/// The entries as `(label, enabled, command, args)`; a separator as `-`.
fn rows(items: &[Item]) -> Vec<(String, bool, String, Value)> {
    items
        .iter()
        .map(|item| match item {
            Item::Entry(Entry {
                label,
                enabled,
                action: Action::Run { command, args },
                ..
            }) => (text(label), *enabled, command.clone(), args.clone()),
            Item::Entry(entry) => (
                text(&entry.label),
                entry.enabled,
                String::new(),
                Value::Null,
            ),
            Item::Separator => ("-".to_owned(), true, String::new(), Value::Null),
            Item::Menu { label, .. } => (
                format!("{}>", text(label)),
                true,
                String::new(),
                Value::Null,
            ),
        })
        .collect()
}

#[test]
fn without_a_recording_the_menu_starts_one_in_this_view_and_offers_no_check() {
    let items = build(&with_recorder(RecorderMenu::default(), Some((30, 40))));
    let rows = rows(submenu(&items, KEY_MENU));
    assert_eq!(
        rows[0],
        (
            "contextmenu.recorder.start".to_owned(),
            true,
            crate::recorder::START.to_owned(),
            json!({ "target": 2 })
        )
    );
    let checks: Vec<&(String, bool, String, Value)> = rows
        .iter()
        .filter(|row| row.2 == crate::recorder::CHECK)
        .collect();
    assert_eq!(checks.len(), 3);
    assert!(
        checks.iter().all(|row| !row.1),
        "disabled without a recording"
    );
    assert_eq!(
        rows.last().map(|row| row.2.as_str()),
        Some(crate::recorder::PANEL)
    );
}

#[test]
fn while_recording_it_pauses_stops_and_checks_the_element_under_the_pointer() {
    let recorder = RecorderMenu {
        recording: Some(false),
        flows: None,
    };
    let items = build(&with_recorder(recorder, Some((30, 40))));
    let rows = rows(submenu(&items, KEY_MENU));
    let labels: Vec<&str> = rows.iter().map(|row| row.0.as_str()).collect();
    assert_eq!(
        labels,
        [
            "contextmenu.recorder.pause",
            "contextmenu.recorder.stop",
            "-",
            "contextmenu.recorder.check_visible",
            "contextmenu.recorder.check_text",
            "contextmenu.recorder.wait_for",
            "-",
            "contextmenu.recorder.play",
            "contextmenu.recorder.panel",
        ]
    );
    assert_eq!(rows[0].2, crate::recorder::PAUSE);
    assert_eq!(rows[1].2, crate::recorder::STOP);
    for (row, kind) in rows[3..6].iter().zip(["visible", "text", "waitFor"]) {
        assert!(row.1, "{} enabled", row.0);
        assert_eq!(
            row.3,
            json!({ "kind": kind, "view": 2, "x": 30, "y": 40, "dip": true })
        );
    }
    assert_eq!(rows[7].2, crate::recorder::PLAY);
    assert!(!rows[7].1, "no playing while a recording runs");
}

#[test]
fn a_paused_recording_resumes_and_a_menu_without_a_point_checks_nothing() {
    let recorder = RecorderMenu {
        recording: Some(true),
        flows: None,
    };
    let items = build(&with_recorder(recorder, None));
    let rows = rows(submenu(&items, KEY_MENU));
    assert_eq!(rows[0].0, "contextmenu.recorder.resume");
    assert!(
        rows.iter()
            .filter(|row| row.2 == crate::recorder::CHECK)
            .all(|row| !row.1)
    );
}

#[test]
fn run_flow_lists_the_projects_flows_then_all_and_the_panel() {
    let recorder = RecorderMenu {
        recording: None,
        flows: Some(vec!["login".to_owned(), "cart & checkout".to_owned()]),
    };
    let items = build(&with_recorder(recorder, Some((1, 1))));
    let rows = rows(submenu(&items, KEY_FLOWS));
    let labels: Vec<&str> = rows.iter().map(|row| row.0.as_str()).collect();
    assert_eq!(
        labels,
        [
            "'login'",
            "'cart & checkout'",
            "-",
            "contextmenu.flows.run_all",
            "contextmenu.flows.panel",
        ]
    );
    assert_eq!(
        (rows[0].2.as_str(), &rows[0].3),
        (crate::flows::RUN, &json!({ "name": "login" }))
    );
    assert_eq!(rows[3].2, crate::flows::RUN_ALL);
    assert_eq!(rows[4].2, crate::flows::PANEL);
}

#[test]
fn run_flow_before_the_first_listing_and_without_flows() {
    let unknown = build(&with_recorder(RecorderMenu::default(), None));
    let rows_unknown = rows(submenu(&unknown, KEY_FLOWS));
    assert_eq!(
        rows_unknown[0],
        (
            "contextmenu.flows.run".to_owned(),
            true,
            crate::flows::RUN.to_owned(),
            json!({})
        )
    );
    let none = RecorderMenu {
        recording: None,
        flows: Some(Vec::new()),
    };
    let empty = build(&with_recorder(none, None));
    let rows_empty = rows(submenu(&empty, KEY_FLOWS));
    assert_eq!(rows_empty[0].0, "contextmenu.flows.none");
    assert!(!rows_empty[0].1);
}

#[test]
fn both_submenus_wait_for_their_commands() {
    let mut state = with_recorder(RecorderMenu::default(), None);
    state.known = BTreeSet::new();
    let items = build(&state);
    assert!(find_menu(&items, KEY_MENU).is_none());
    assert!(find_menu(&items, KEY_FLOWS).is_none());
    assert!(
        find_menu(&items, crate::context_menu::model::KEY_FLOWS).is_none(),
        "an empty Flows submenu is left out"
    );
}

/// Flows holds Run flow, then the Recorder, as the main menu's Flows do (RB-090-92).
#[test]
fn run_flow_and_the_recorder_hang_under_flows() {
    let items = build(&with_recorder(RecorderMenu::default(), None));
    let flows = submenu(&items, crate::context_menu::model::KEY_FLOWS);
    let titles: Vec<String> = flows
        .iter()
        .filter_map(|item| match item {
            Item::Menu { label, .. } => Some(text(label)),
            _ => None,
        })
        .collect();
    assert_eq!(titles, [KEY_FLOWS, KEY_MENU]);
}
