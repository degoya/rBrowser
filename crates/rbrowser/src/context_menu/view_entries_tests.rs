// RB-090-63: the page menu reaches what hangs on the clicked view - Sync › this view and its
// navigation, Session › the sessions and clearing one, Tools › Peek, the raw view and Fill
// forms › - each the registry's own command, shown only when it can run.

use std::collections::BTreeSet;

use rb_engine::ViewId;
use serde_json::{Value, json};

use super::*;
use crate::context_menu::model::{Action, Entry, Item, Label, Mark, MenuState, build};

fn state(known: &[&str], view_entries: ViewEntries) -> MenuState {
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
        view_entries,
    }
}

/// The items of the submenu labelled `key` at any depth, if there is one: Sessions hangs under
/// Environments (RB-090-92).
fn submenu<'a>(items: &'a [Item], key: &str) -> Option<&'a [Item]> {
    items.iter().find_map(|item| match item {
        Item::Menu {
            label: Label::Key(label),
            items,
            ..
        } if *label == key => Some(items.as_slice()),
        Item::Menu { items, .. } => submenu(items, key),
        _ => None,
    })
}

fn entries(items: &[Item]) -> Vec<Entry> {
    items
        .iter()
        .filter_map(|item| match item {
            Item::Entry(entry) => Some(entry.clone()),
            _ => None,
        })
        .collect()
}

fn entry(items: &[Item], label: &Label) -> Entry {
    entries(items)
        .into_iter()
        .find(|entry| &entry.label == label)
        .unwrap_or_else(|| panic!("no entry {label:?}"))
}

fn run(command: &str, args: Value) -> Action {
    Action::Run {
        command: command.to_owned(),
        args,
    }
}

#[test]
fn nothing_new_shows_without_the_state_or_the_commands() {
    let items = build(&state(&[], ViewEntries::default()));
    for key in [SYNC_MENU, SESSION_MENU, TOOLS_MENU] {
        assert!(submenu(&items, key).is_none(), "{key}");
    }
}

#[test]
fn sync_checks_this_view_and_its_navigation_and_turns_them_the_other_way() {
    let view_entries = ViewEntries {
        synced: Some(true),
        navigation_coupled: Some(false),
        ..ViewEntries::default()
    };
    let items = build(&state(&[], view_entries));
    let sync = submenu(&items, SYNC_MENU).expect("the Sync submenu");
    let this = entry(sync, &Label::Key("contextmenu.sync.this_view"));
    assert_eq!(this.mark, Mark::Check(true));
    assert!(this.enabled);
    assert_eq!(
        this.action,
        run(
            crate::sync_hub::TOGGLE_VIEW,
            json!({ "view": 2, "enabled": false })
        )
    );
    let navigation = entry(sync, &Label::Key("contextmenu.sync.this_view_navigation"));
    assert_eq!(navigation.mark, Mark::Check(false));
    assert_eq!(
        navigation.action,
        run(
            crate::nav_sync::TOGGLE_VIEW_NAVIGATION,
            json!({ "view": 2, "enabled": true })
        )
    );
}

// RB-092-03: Sync › opens the address field of a view with its own navigation; for a view that
// follows the active view the entry is disabled and its label says why; off, it is not there.
#[test]
fn sync_opens_the_address_field_of_a_decoupled_view_and_says_why_not_otherwise() {
    let with = |coupled: bool, editable: bool| {
        build(&state(
            &[],
            ViewEntries {
                navigation_coupled: Some(coupled),
                address_editable: editable,
                ..ViewEntries::default()
            },
        ))
    };
    let items = with(false, true);
    let sync = submenu(&items, SYNC_MENU).expect("the Sync submenu");
    let edit = entry(sync, &Label::Key(EDIT_ADDRESS));
    assert!(edit.enabled);
    assert_eq!(edit.mark, Mark::None);
    assert_eq!(
        edit.action,
        run(
            crate::cell_info::address::EDIT_ADDRESS,
            json!({ "view": 2 })
        )
    );
    let items = with(true, true);
    let sync = submenu(&items, SYNC_MENU).expect("the Sync submenu");
    let edit = entry(sync, &Label::Key(EDIT_ADDRESS_COUPLED));
    assert!(!edit.enabled, "the view follows the active view");
    for coupled in [false, true] {
        let items = with(coupled, false);
        let sync = submenu(&items, SYNC_MENU).expect("the Sync submenu");
        assert!(
            entries(sync).iter().all(|entry| ![
                Label::Key(EDIT_ADDRESS),
                Label::Key(EDIT_ADDRESS_COUPLED)
            ]
            .contains(&entry.label)),
            "off: no entry"
        );
    }
}

#[test]
fn sync_shows_only_the_switch_whose_state_is_known() {
    let view_entries = ViewEntries {
        synced: Some(false),
        ..ViewEntries::default()
    };
    let items = build(&state(&[], view_entries));
    let sync = submenu(&items, SYNC_MENU).expect("the Sync submenu");
    assert_eq!(entries(sync).len(), 1);
    assert_eq!(
        entry(sync, &Label::Key("contextmenu.sync.this_view")).mark,
        Mark::Check(false)
    );
}

#[test]
fn session_lists_the_sessions_marks_the_views_and_moves_it() {
    let view_entries = ViewEntries {
        sessions: vec!["default".to_owned(), "admin".to_owned()],
        session: Some("admin".to_owned()),
        ..ViewEntries::default()
    };
    let items = build(&state(&[], view_entries));
    let environments =
        submenu(&items, crate::context_menu::model::KEY_ENVIRONMENTS).expect("Environments");
    assert!(
        matches!(
            environments,
            [Item::Menu {
                label: Label::Key(SESSION_MENU),
                ..
            }]
        ),
        "Sessions is Environments' only entry"
    );
    let session = submenu(&items, SESSION_MENU).expect("the Session submenu");
    let default = entry(session, &Label::Key("contextmenu.session.default"));
    assert_eq!(default.mark, Mark::Radio(false));
    assert_eq!(
        default.action,
        run(
            crate::sessions::ASSIGN,
            json!({ "target": 2, "session": "default" })
        )
    );
    let admin = entry(session, &Label::Text("admin".to_owned()));
    assert_eq!(admin.mark, Mark::Radio(true));
    assert_eq!(
        admin.action,
        run(
            crate::sessions::ASSIGN,
            json!({ "target": 2, "session": "admin" })
        )
    );
    // Clearing asks first: without `session` the shell confirms, at the clicked view's session.
    let clear = entry(session, &Label::Key("contextmenu.session.clear"));
    assert_eq!(
        clear.action,
        run(crate::sessions::CLEAR, json!({ "view": 2 }))
    );
}

#[test]
fn only_the_default_session_offers_no_session_submenu() {
    let view_entries = ViewEntries {
        sessions: vec!["default".to_owned()],
        session: Some("default".to_owned()),
        ..ViewEntries::default()
    };
    let items = build(&state(&[], view_entries));
    assert!(submenu(&items, SESSION_MENU).is_none());
    assert!(
        submenu(&items, crate::context_menu::model::KEY_ENVIRONMENTS).is_none(),
        "an empty Environments submenu is left out"
    );
}

#[test]
fn tools_run_peek_the_raw_view_and_fill_forms() {
    let items = build(&state(
        &[PEEK, RAW_VIEW, FILL_VALID, FILL_INVALID],
        ViewEntries::default(),
    ));
    let tools = submenu(&items, TOOLS_MENU).expect("the Tools submenu");
    assert_eq!(
        entry(tools, &Label::Key("contextmenu.tools.peek")).action,
        run(PEEK, json!({}))
    );
    assert_eq!(
        entry(tools, &Label::Key("contextmenu.tools.raw_view")).action,
        run(RAW_VIEW, json!({ "target": 2 }))
    );
    let fill = submenu(tools, FILL_MENU).expect("the Fill forms submenu");
    let shown: Vec<(Label, Action)> = entries(fill)
        .into_iter()
        .map(|entry| (entry.label, entry.action))
        .collect();
    assert_eq!(
        shown,
        [
            (
                Label::Key("contextmenu.tools.fill_valid"),
                run(FILL_VALID, json!({ "target": 2 }))
            ),
            (
                Label::Key("contextmenu.tools.fill_invalid"),
                run(FILL_INVALID, json!({ "target": 2 }))
            ),
            (
                Label::Key("contextmenu.tools.fill_valid_all"),
                run(FILL_VALID, json!({ "target": "all" }))
            ),
            (
                Label::Key("contextmenu.tools.fill_invalid_all"),
                run(FILL_INVALID, json!({ "target": "all" }))
            ),
        ]
    );
}

#[test]
fn fill_forms_takes_the_clicked_views_form_as_a_preset() {
    // RB-090-70: below the fills, apart; this view only, so it needs the view's session.
    let items = build(&state(&[FILL_VALID, FILL_CAPTURE], ViewEntries::default()));
    let tools = submenu(&items, TOOLS_MENU).expect("the Tools submenu");
    let fill = submenu(tools, FILL_MENU).expect("the Fill forms submenu");
    assert!(matches!(fill.last(), Some(Item::Entry(_))));
    assert!(fill.iter().any(|item| matches!(item, Item::Separator)));
    let take = entry(fill, &Label::Key("contextmenu.tools.fill_capture"));
    assert_eq!(take.action, run(FILL_CAPTURE, json!({ "target": 2 })));
    assert!(take.enabled);

    let only = build(&state(&[FILL_CAPTURE], ViewEntries::default()));
    let tools = submenu(&only, TOOLS_MENU).expect("the Tools submenu");
    let fill = submenu(tools, FILL_MENU).expect("the Fill forms submenu");
    assert_eq!(fill.len(), 1, "no separator before the only entry");

    let mut detached = state(&[FILL_CAPTURE], ViewEntries::default());
    detached.attached = false;
    let items = build(&detached);
    let tools = submenu(&items, TOOLS_MENU).expect("the Tools submenu");
    let fill = submenu(tools, FILL_MENU).expect("the Fill forms submenu");
    assert!(!entry(fill, &Label::Key("contextmenu.tools.fill_capture")).enabled);
}

#[test]
fn tools_offer_only_the_registered_plugins_commands() {
    let items = build(&state(&[PEEK], ViewEntries::default()));
    let tools = submenu(&items, TOOLS_MENU).expect("the Tools submenu");
    assert_eq!(entries(tools).len(), 1);
    assert!(submenu(tools, FILL_MENU).is_none());
}

#[test]
fn a_view_without_its_session_cannot_run_the_this_view_tools() {
    let mut detached = state(&[RAW_VIEW, FILL_VALID], ViewEntries::default());
    detached.attached = false;
    let items = build(&detached);
    let tools = submenu(&items, TOOLS_MENU).expect("the Tools submenu");
    assert!(!entry(tools, &Label::Key("contextmenu.tools.raw_view")).enabled);
    let fill = submenu(tools, FILL_MENU).expect("the Fill forms submenu");
    assert!(!entry(fill, &Label::Key("contextmenu.tools.fill_valid")).enabled);
    assert!(
        entry(fill, &Label::Key("contextmenu.tools.fill_valid_all")).enabled,
        "the other views can still be filled"
    );
}

#[test]
fn the_plugin_commands_are_known_to_the_state() {
    for command in PLUGIN_COMMANDS {
        assert!(
            crate::context_menu::state::OPTIONAL_COMMANDS.contains(&command),
            "{command}"
        );
    }
}

/// Each id is a command the bundled plugin declares, named as the plugin host names it.
#[test]
fn the_plugin_commands_are_the_bundled_plugins() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins");
    for (plugin, command) in [
        ("peek", PEEK),
        ("json-xml-viewer", RAW_VIEW),
        ("form-autofill", FILL_VALID),
        ("form-autofill", FILL_INVALID),
        ("form-autofill", FILL_CAPTURE),
    ] {
        let manifest =
            std::fs::read_to_string(root.join(plugin).join("manifest.toml")).expect("manifest");
        let own = command
            .rsplit('.')
            .next()
            .expect("a command id has a last part");
        assert!(
            manifest.contains(&format!("id = \"{own}\"")),
            "{plugin} declares no {own}"
        );
        assert_eq!(
            command,
            format!("plugin.{}.{own}", plugin.replace('-', "_")),
            "as rb-plugin-host names it"
        );
    }
}
