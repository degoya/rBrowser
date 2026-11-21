// RB-090-92: the block's submenus are named and ordered as the main menu's, and inside them
// the main menu's grouping holds (Capture > Screenshot, Flows > Recorder, Environments >
// Sessions); a level that would hold one entry only is left out (Layout > Analyze breakpoints).

use std::collections::BTreeSet;

use rb_config::EmulationUserSettings;
use rb_engine::ViewId;
use serde_json::Value;

use super::*;
use crate::context_menu::emulation::EmulationState;
use crate::context_menu::model::{
    FIRST_ID, FIRST_MENU_ID, Item, LAST_MENU_ID, Label, MenuState, PluginCommand, PluginPanel,
    build,
};
use crate::context_menu::recorder::RecorderMenu;
use crate::context_menu::view_entries::{PLUGIN_COMMANDS, ViewEntries};

/// Every part of the block shown: every optional command known, emulation on, two sessions,
/// a check and a panel of a plugin, the project's flows listed.
fn fullest() -> MenuState {
    let mut known: BTreeSet<String> = [
        "breakpoints.analyze",
        "view.remove",
        crate::view_footer::defs::TOGGLE,
        crate::view_footer::defs::VITALS_TOGGLE,
        crate::view_footer::defs::LIGHTHOUSE_TOGGLE,
        crate::force_state::SET,
        crate::recorder::CHECK,
        crate::flows::RUN,
        crate::sitemap::defs::PANEL,
        crate::video::defs::PANEL,
        crate::bug_report::CREATE,
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    known.extend(PLUGIN_COMMANDS.iter().map(|command| (*command).to_owned()));
    MenuState {
        view: ViewId(2),
        point: Some((30, 40)),
        menu_enabled: true,
        attached: true,
        detached: false,
        devtools_open: false,
        open_on_inspect: true,
        emulation: Some(EmulationState::of(
            None,
            &[],
            &EmulationUserSettings::default(),
        )),
        recording: None,
        checks_busy: false,
        checks: vec![PluginCommand {
            command: "plugin.seo.check".to_owned(),
            title: "SEO check".to_owned(),
        }],
        panels: vec![PluginPanel {
            plugin: "network-panel".to_owned(),
            panel: "network".to_owned(),
            title: "Network".to_owned(),
        }],
        known,
        footer_visible: true,
        vitals_bar_visible: true,
        lighthouse_bar_visible: true,
        forced: crate::force_state::ForcedMarks::default(),
        recorder: RecorderMenu {
            recording: None,
            flows: Some(vec!["login".to_owned()]),
        },
        view_entries: ViewEntries {
            synced: Some(true),
            navigation_coupled: Some(true),
            address_editable: true,
            sessions: vec!["default".to_owned(), "staging".to_owned()],
            session: Some("default".to_owned()),
        },
    }
}

fn text(label: &Label) -> String {
    match label {
        Label::Key(key) => (*key).to_owned(),
        Label::Text(text) => format!("'{text}'"),
        Label::KeyOr { key, .. } => key.clone(),
    }
}

/// One level of a menu: an entry's label, a submenu's label with `>`, `-` for a separator.
fn level(items: &[Item]) -> Vec<String> {
    items
        .iter()
        .map(|item| match item {
            Item::Entry(entry) => text(&entry.label),
            Item::Separator => "-".to_owned(),
            Item::Menu { label, .. } => format!("{}>", text(label)),
        })
        .collect()
}

/// The items of the top-level submenu labelled `key`.
fn submenu<'a>(items: &'a [Item], key: &str) -> &'a [Item] {
    items
        .iter()
        .find_map(|item| match item {
            Item::Menu { label, items, .. } if text(label) == key => Some(items.as_slice()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no submenu {key} in {:?}", level(items)))
}

/// The labels of the top level's submenus.
fn top_menus(items: &[Item]) -> Vec<String> {
    items
        .iter()
        .filter_map(|item| match item {
            Item::Menu { label, .. } => Some(text(label)),
            _ => None,
        })
        .collect()
}

/// The keys of the submenus of `MAIN_MENU` in `ui/src/menu/mainMenu.ts`, in its order.
fn main_menu_ts() -> Vec<String> {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/src/menu/mainMenu.ts");
    let source = std::fs::read_to_string(&path).expect("ui/src/menu/mainMenu.ts");
    let start = source
        .find("export const MAIN_MENU")
        .expect("MAIN_MENU in mainMenu.ts");
    let body = &source[start..];
    let body = &body[..body.find("];").expect("the end of MAIN_MENU")];
    body.split("sub('")
        .skip(1)
        .filter_map(|rest| rest.split('\'').next())
        .map(str::to_owned)
        .collect()
}

#[test]
fn the_main_menu_mirror_is_the_main_menus_order() {
    assert_eq!(main_menu_ts(), MAIN_MENU, "MAIN_MENU mirrors mainMenu.ts");
}

#[test]
fn the_top_level_submenus_are_main_menu_keys_in_its_order() {
    let items = build(&fullest());
    let menus = top_menus(&items);
    assert_eq!(
        menus,
        [
            KEY_VIEW,
            KEY_LAYOUT,
            KEY_SYNC,
            KEY_EMULATION,
            KEY_ENVIRONMENTS,
            KEY_CAPTURE,
            KEY_CHECKS,
            KEY_TOOLS,
            KEY_FLOWS,
            KEY_PANELS,
        ]
    );
    let at: Vec<usize> = menus
        .iter()
        .map(|key| {
            MAIN_MENU
                .iter()
                .position(|main| main == key)
                .unwrap_or_else(|| panic!("{key} is no submenu of the main menu"))
        })
        .collect();
    assert!(at.windows(2).all(|pair| pair[0] < pair[1]), "{menus:?}");
    // The flat entries stay where they were: inspect and DevTools on top, the view's own below.
    assert_eq!(
        level(&items),
        [
            "contextmenu.inspect",
            "contextmenu.devtools",
            "-",
            "shell.menu.view>",
            "shell.menu.layout>",
            "shell.menu.sync>",
            "shell.menu.emulation>",
            "shell.menu.environments>",
            "shell.menu.capture>",
            "shell.menu.checks>",
            "shell.menu.tools>",
            "shell.menu.flows>",
            "shell.menu.panels>",
            "-",
            "sync.menu.skipNextClick",
            "popout.menu.out",
            "contextmenu.remove_view",
        ]
    );
}

#[test]
fn inside_the_submenus_the_main_menus_grouping_holds() {
    let items = build(&fullest());
    assert_eq!(
        level(submenu(&items, KEY_VIEW)),
        [
            "contextmenu.view_footer",
            "contextmenu.vitals_bar",
            "contextmenu.lighthouse_bar",
        ]
    );
    assert_eq!(
        level(submenu(&items, KEY_LAYOUT)),
        ["contextmenu.checks.analyze"],
        "no Breakpoints level for its one entry"
    );
    assert_eq!(
        level(submenu(&items, KEY_ENVIRONMENTS)),
        ["shell.menu.sessions>"]
    );
    assert_eq!(
        level(submenu(&items, KEY_CAPTURE)),
        [
            "shell.menu.screenshot>",
            "shell.menu.video>",
            "contextmenu.bug_report"
        ]
    );
    assert_eq!(
        level(submenu(&items, KEY_TOOLS))[0],
        "contextmenu.force_state.menu>"
    );
    assert_eq!(
        level(submenu(&items, KEY_FLOWS)),
        ["contextmenu.flows.menu>", "shell.menu.recorder>"]
    );
    assert!(
        !level(submenu(&items, KEY_CHECKS)).contains(&"contextmenu.checks.analyze".to_owned()),
        "the analyzer is under Layout"
    );
}

/// `ui.pageContextMenuEmulation` off takes the Emulation submenu only: the analyzer stays under
/// Layout.
#[test]
fn the_analyzer_is_under_layout_with_or_without_emulation() {
    let mut state = fullest();
    state.emulation = None;
    let items = build(&state);
    assert!(!top_menus(&items).contains(&KEY_EMULATION.to_owned()));
    let layout = submenu(&items, KEY_LAYOUT);
    assert_eq!(level(layout), ["contextmenu.checks.analyze"]);
    let Some(Item::Entry(analyze)) = layout.first() else {
        panic!("an entry");
    };
    assert_eq!(
        analyze.action,
        crate::context_menu::model::Action::Run {
            command: "breakpoints.analyze".to_owned(),
            args: Value::Null,
        },
        "the analyzer's window opens for Null only"
    );
}

#[test]
fn an_empty_submenu_is_left_out() {
    let mut state = fullest();
    state.known = BTreeSet::new();
    state.emulation = None;
    state.checks.clear();
    state.panels.clear();
    state.view_entries = ViewEntries::default();
    let items = build(&state);
    assert_eq!(top_menus(&items), [KEY_CAPTURE, KEY_CHECKS, KEY_PANELS]);
}

/// The fullest block fits: each submenu an id of its own inside the submenus' range, the
/// entries numbered in menu order from `FIRST_ID`.
#[test]
fn the_fullest_block_fits_the_submenu_range() {
    fn walk(items: &[Item], menus: &mut Vec<i32>, entries: &mut Vec<i32>) {
        for item in items {
            match item {
                Item::Entry(entry) => entries.push(entry.id),
                Item::Separator => {}
                Item::Menu { id, items, .. } => {
                    menus.push(*id);
                    walk(items, menus, entries);
                }
            }
        }
    }
    let items = build(&fullest());
    let (mut menus, mut entries) = (Vec::new(), Vec::new());
    walk(&items, &mut menus, &mut entries);
    // View, Layout, Sync, Emulation (with Geolocation, Time zone, CPU, Text direction, Clock,
    // Animations, All views), Environments > Sessions, Capture > Screenshot, Video, Checks,
    // Tools > Force state (with All views), Fill forms, Flows > Run flow, Recorder, Panels.
    assert_eq!(menus.len(), 25, "{menus:?}");
    let unique: BTreeSet<i32> = menus.iter().copied().collect();
    assert_eq!(unique.len(), menus.len(), "{menus:?}");
    assert!(
        menus
            .iter()
            .all(|id| (FIRST_MENU_ID..=LAST_MENU_ID).contains(id)),
        "{menus:?}"
    );
    assert_eq!(entries.first(), Some(&FIRST_ID));
    assert!(
        entries.windows(2).all(|pair| pair[1] == pair[0] + 1),
        "the entries are numbered in menu order: {entries:?}"
    );
}

/// RB-093-04: Capture > Bug report makes a report of every view, once the command is known.
#[test]
fn the_bug_report_is_under_capture_when_known() {
    let items = build(&fullest());
    let capture = submenu(&items, KEY_CAPTURE);
    let Some(Item::Entry(report)) = capture.last() else {
        panic!("an entry last");
    };
    assert_eq!(
        report.action,
        crate::context_menu::model::Action::Run {
            command: crate::bug_report::CREATE.to_owned(),
            args: serde_json::json!({}),
        }
    );
    let mut state = fullest();
    state.known.remove(crate::bug_report::CREATE);
    let items = build(&state);
    assert_eq!(
        level(submenu(&items, KEY_CAPTURE)),
        ["shell.menu.screenshot>", "shell.menu.video>"]
    );
}
