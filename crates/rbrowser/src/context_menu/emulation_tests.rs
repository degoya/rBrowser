use std::collections::BTreeSet;

use rb_config::{EmulationUserSettings, Geolocation, GeolocationPreset};
use rb_engine::ViewId;
use serde_json::json;

use super::*;
use crate::context_menu::model::{
    Entry, FIRST_MENU_ID, LAST_ID, LAST_MENU_ID, MenuState, build, find, plain_label,
};

pub(super) fn emulation() -> EmulationState {
    EmulationState::of(None, &[], &EmulationUserSettings::default())
}

pub(super) fn state(emulation: Option<EmulationState>) -> MenuState {
    MenuState {
        view: ViewId(2),
        point: Some((30, 40)),
        menu_enabled: true,
        attached: true,
        detached: false,
        devtools_open: false,
        open_on_inspect: true,
        emulation,
        recording: None,
        checks_busy: false,
        checks: Vec::new(),
        panels: Vec::new(),
        known: ["breakpoints.analyze"]
            .into_iter()
            .map(str::to_owned)
            .collect::<BTreeSet<String>>(),
        footer_visible: true,
    }
}

pub(super) fn text(label: &Label) -> String {
    match label {
        Label::Key(key) => (*key).to_owned(),
        Label::Text(text) => format!("'{text}'"),
        Label::KeyOr { key, .. } => key.clone(),
    }
}

/// The menu as lines: `label` for an entry, `[x] label` / `[ ] label` for a switch, `(o) label`
/// / `( ) label` for a choice, `label>`
/// for a submenu with its lines indented, `-` for a separator.
fn lines(items: &[Item], indent: usize, out: &mut Vec<String>) {
    let pad = "  ".repeat(indent);
    for item in items {
        match item {
            Item::Entry(entry) => {
                let mark = match entry.mark {
                    Mark::Check(true) => "[x] ",
                    Mark::Check(false) => "[ ] ",
                    Mark::Radio(true) => "(o) ",
                    Mark::Radio(false) => "( ) ",
                    Mark::None => "",
                };
                out.push(format!("{pad}{mark}{}", text(&entry.label)));
            }
            Item::Separator => out.push(format!("{pad}-")),
            Item::Menu { label, items, .. } => {
                out.push(format!("{pad}{}>", text(label)));
                lines(items, indent + 1, out);
            }
        }
    }
}

pub(super) fn submenu<'a>(items: &'a [Item], key: &str) -> &'a [Item] {
    items
        .iter()
        .find_map(|item| match item {
            Item::Menu { label, items, .. } if text(label) == key => Some(items.as_slice()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no submenu {key}"))
}

pub(super) fn entry<'a>(items: &'a [Item], label: &str) -> &'a Entry {
    items
        .iter()
        .find_map(|item| match item {
            Item::Entry(entry) if text(&entry.label) == label => Some(entry),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no entry {label}"))
}

pub(super) fn action(entry: &Entry) -> (String, serde_json::Value) {
    match &entry.action {
        Action::Run { command, args } => (command.clone(), args.clone()),
        Action::SkipNextClick => panic!("a command"),
    }
}

#[test]
fn the_block_shows_the_toolbars_switches_and_analyze_breakpoints() {
    let items = build(&state(Some(emulation())));
    let block = submenu(&items, "contextmenu.emulation.menu");
    let mut shown = Vec::new();
    lines(block, 0, &mut shown);
    assert_eq!(
        shown,
        [
            "( ) emulation.toolbar.light",
            "( ) emulation.toolbar.dark",
            "(o) contextmenu.emulation.scheme_none",
            "-",
            "[ ] emulation.toolbar.reducedMotion",
            "[ ] emulation.extras.offline",
            "-",
            "emulation.extras.geolocation>",
            "  (o) emulation.extras.off",
            "  ( ) emulation.geolocation.preset.berlin",
            "  ( ) emulation.geolocation.preset.new-york",
            "  ( ) emulation.geolocation.preset.tokyo",
            "  ( ) emulation.geolocation.preset.sydney",
            "emulation.extras.timezone>",
            "  (o) emulation.extras.off",
            "  ( ) 'UTC'",
            "  ( ) 'Europe/Berlin'",
            "  ( ) 'Europe/London'",
            "  ( ) 'America/New_York'",
            "  ( ) 'America/Los_Angeles'",
            "  ( ) 'America/Sao_Paulo'",
            "  ( ) 'Asia/Tokyo'",
            "  ( ) 'Australia/Sydney'",
            "-",
            "emulation.toolbar.reset",
            "contextmenu.emulation.all_views>",
            "  ( ) emulation.toolbar.light",
            "  ( ) emulation.toolbar.dark",
            "  ( ) contextmenu.emulation.scheme_none",
            "  -",
            "  [ ] emulation.toolbar.reducedMotion",
            "  -",
            "  emulation.toolbar.reset",
        ],
        "no view at all: nothing in common, nothing marked under All views"
    );
    // The submenu and the analyzer follow Panels; the analyzer is not under Checks then.
    let mut top = Vec::new();
    lines(&items, 0, &mut top);
    let top: Vec<&str> = top
        .iter()
        .map(String::as_str)
        .filter(|line| !line.starts_with(' '))
        .collect();
    assert_eq!(
        top[5..9],
        [
            "contextmenu.panels.menu>",
            "contextmenu.emulation.menu>",
            "contextmenu.checks.analyze",
            "-",
        ]
    );
    let checks = submenu(&items, "contextmenu.checks.menu");
    assert!(
        !checks
            .iter()
            .any(|item| matches!(item, Item::Entry(entry) if text(&entry.label) == "contextmenu.checks.analyze")),
        "once in the menu"
    );
    assert_eq!(
        action(entry(&items, "contextmenu.checks.analyze")),
        ("breakpoints.analyze".to_owned(), serde_json::Value::Null),
        "the toolbar's command, the window for Null"
    );
}

/// Each entry is the toolbar's command for the clicked view.
#[test]
fn every_switch_runs_the_toolbars_command_for_the_clicked_view() {
    let items = build(&state(Some(emulation())));
    let block = submenu(&items, "contextmenu.emulation.menu");
    let set = |fields: serde_json::Value| {
        (
            "emulation.set".to_owned(),
            json!({ "target": 2, "fields": fields }),
        )
    };
    assert_eq!(
        action(entry(block, "emulation.toolbar.light")),
        set(json!({ "colorScheme": "light" }))
    );
    assert_eq!(
        action(entry(block, "emulation.toolbar.dark")),
        set(json!({ "colorScheme": "dark" }))
    );
    assert_eq!(
        action(entry(block, "contextmenu.emulation.scheme_none")),
        set(json!({ "colorScheme": null })),
        "drops the override only"
    );
    assert_eq!(
        action(entry(block, "emulation.toolbar.reducedMotion")),
        set(json!({ "reducedMotion": true }))
    );
    assert_eq!(
        action(entry(block, "emulation.extras.offline")),
        (
            "view.emulate.offline".to_owned(),
            json!({ "target": 2, "offline": true })
        )
    );
    assert_eq!(
        action(entry(block, "emulation.toolbar.reset")),
        ("emulation.reset_view".to_owned(), json!({ "target": 2 }))
    );
    let location = submenu(block, "emulation.extras.geolocation");
    assert_eq!(
        action(entry(location, "emulation.extras.off")),
        (
            "view.emulate.geolocation".to_owned(),
            json!({ "target": 2, "geolocation": null })
        )
    );
    assert_eq!(
        action(entry(location, "emulation.geolocation.preset.tokyo")),
        (
            "view.emulate.geolocation".to_owned(),
            json!({ "target": 2, "preset": "tokyo" })
        )
    );
    let timezone = submenu(block, "emulation.extras.timezone");
    assert_eq!(
        action(entry(timezone, "emulation.extras.off")),
        (
            "view.emulate.timezone".to_owned(),
            json!({ "target": 2, "timezone": "" })
        )
    );
    assert_eq!(
        action(entry(timezone, "'Asia/Tokyo'")),
        (
            "view.emulate.timezone".to_owned(),
            json!({ "target": 2, "timezone": "Asia/Tokyo" })
        )
    );
    let all = submenu(block, "contextmenu.emulation.all_views");
    assert_eq!(
        action(entry(all, "emulation.toolbar.dark")),
        (
            "emulation.set".to_owned(),
            json!({ "target": "all", "fields": { "colorScheme": "dark" } })
        )
    );
    assert_eq!(
        action(entry(all, "emulation.toolbar.reset")),
        (
            "emulation.reset_view".to_owned(),
            json!({ "target": "all" })
        )
    );
}

pub(super) fn emulated() -> rb_engine::Emulation {
    rb_engine::Emulation {
        color_scheme: Some(ColorScheme::Dark),
        reduced_motion: Some(true),
        network: Some(rb_config::NetworkConditions {
            offline: true,
            ..rb_config::NetworkConditions::default()
        }),
        geolocation: Some(Geolocation {
            latitude: 35.6762,
            longitude: 139.6503,
            accuracy: 50.0,
        }),
        timezone: Some("Asia/Kolkata".to_owned()),
        ..rb_engine::Emulation::default()
    }
}

/// A user's preset shows its label; a built-in one the catalogue's text for its id, derived
/// from the id; a user's preset without a label its id. Names from outside the catalogue are
/// rendered plain.
#[test]
fn the_presets_are_named_by_label_catalogue_or_id() {
    let preset = |id: &str, label: Option<&str>, latitude: f64| GeolocationPreset {
        id: id.to_owned(),
        label: label.map(str::to_owned),
        position: Geolocation {
            latitude,
            longitude: 2.0,
            accuracy: 50.0,
        },
    };
    let settings = EmulationUserSettings {
        geolocation_presets: vec![
            preset("office", Some("R&D office"), 1.0),
            preset("lab", None, 3.0),
        ],
        ..EmulationUserSettings::default()
    };
    let mut emulation = emulated();
    emulation.geolocation = Some(Geolocation {
        latitude: 1.0,
        longitude: 2.0,
        accuracy: 10.0,
    });
    let state_of = EmulationState::of(Some(&emulation), &[], &settings);
    let items = build(&state(Some(state_of)));
    let block = submenu(&items, "contextmenu.emulation.menu");
    let location = submenu(block, "emulation.extras.geolocation");
    let office = entry(location, "'R&D office'");
    assert_eq!(office.mark, Mark::Radio(true));
    assert_eq!(action(office).1, json!({ "target": 2, "preset": "office" }));
    let Label::Text(name) = &office.label else {
        panic!("a text");
    };
    assert_eq!(plain_label(name), "R&&D office", "as the menu shows it");
    let labels: Vec<&Label> = location
        .iter()
        .filter_map(|item| match item {
            Item::Entry(entry) => Some(&entry.label),
            _ => None,
        })
        .collect();
    assert!(labels.contains(&&Label::KeyOr {
        key: "emulation.geolocation.preset.berlin".to_owned(),
        text: "berlin".to_owned(),
    }));
    assert!(labels.contains(&&Label::KeyOr {
        key: "emulation.geolocation.preset.lab".to_owned(),
        text: "lab".to_owned(),
    }));
    // The catalogue has the built-in presets' names under the derived keys.
    for id in ["berlin", "new-york", "tokyo", "sydney"] {
        let key = format!("emulation.geolocation.preset.{id}");
        assert_ne!(rb_i18n::t(&key), key);
    }
}

/// `ui.pageContextMenuEmulation` off: no submenu, and the analyzer is under Checks as before.
#[test]
fn the_setting_off_leaves_the_block_out() {
    let items = build(&state(None));
    assert!(
        !items
            .iter()
            .any(|item| matches!(item, Item::Menu { label, .. } if text(label) == "contextmenu.emulation.menu"))
    );
    assert!(
        !items
            .iter()
            .any(|item| matches!(item, Item::Entry(entry) if text(&entry.label) == "contextmenu.checks.analyze")),
        "not an entry of the menu itself"
    );
    let checks = submenu(&items, "contextmenu.checks.menu");
    entry(checks, "contextmenu.checks.analyze");
}

/// The fullest block fits: every submenu has an id of its own inside the submenus' range,
/// every entry one inside CEF's user range, and every entry is found by its id.
#[test]
fn the_fullest_block_fits_the_ids() {
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
    let items = build(&state(Some(emulation())));
    let (mut menus, mut entries) = (Vec::new(), Vec::new());
    walk(&items, &mut menus, &mut entries);
    assert_eq!(menus.len(), 8, "four of the block, four of the emulation");
    let unique: BTreeSet<i32> = menus.iter().copied().collect();
    assert_eq!(unique.len(), menus.len(), "{menus:?}");
    assert!(
        menus
            .iter()
            .all(|id| (FIRST_MENU_ID..=LAST_MENU_ID).contains(id)),
        "the block needs more submenu ids than the range has: {menus:?}"
    );
    let unique: BTreeSet<i32> = entries.iter().copied().collect();
    assert_eq!(unique.len(), entries.len());
    assert!(entries.iter().all(|id| *id <= LAST_ID));
    for id in entries {
        assert_eq!(find(&items, id).map(|found| found.id), Some(id));
    }
}
