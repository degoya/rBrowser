use std::sync::{Arc, Mutex};

use rb_ipc::{MapRect, NativeMenuRow, ShellToCore};

use super::*;
use crate::host::ui::recording::UiCall;
use crate::layout::test_support::{config, setup};

fn item(id: &str, label: &str, checked: Option<bool>) -> NativeMenuRow {
    NativeMenuRow::Item {
        id: id.into(),
        label: label.into(),
        checked,
        disabled: false,
    }
}

fn button(x: i32) -> MapRect {
    MapRect {
        x,
        y: 6,
        width: 32,
        height: 32,
    }
}

fn theme_rows() -> Vec<NativeMenuRow> {
    vec![
        item("system", "System (Windows: light)", Some(false)),
        item("light", "Light", Some(false)),
        item("dark", "Dark", Some(true)),
    ]
}

/// The answer, or the refusal's words after `error:`.
fn send(core: &Core, message: &ShellToCore) -> String {
    let request = serde_json::to_string(message).expect("encodes");
    crate::ipc::dispatch(core, &request).unwrap_or_else(|(_, refusal)| format!("error: {refusal}"))
}

fn request(menu: &str, rows: Vec<NativeMenuRow>) -> ShellToCore {
    ShellToCore::NativeMenu {
        menu: menu.into(),
        anchor: button(600),
        rows,
    }
}

// RB-090-41: the theme pulldown names its button and its rows; the core shows them as a native
// menu and tells the shell the row chosen once it closed.
#[tokio::test]
async fn a_pulldown_opens_a_native_menu_and_hears_the_row_chosen() {
    let setup = setup(config());
    *rb_util::lock(&setup.ui.menu_choice) = Some("light".into());
    let answer = send(&setup.core, &request("theme", theme_rows()));
    assert!(answer.contains("\"ack\""), "{answer}");
    let calls = setup.ui.calls();
    let shown: Vec<&NativeMenu> = calls
        .iter()
        .filter_map(|call| match call {
            UiCall::NativeMenu(menu) => Some(menu),
            _ => None,
        })
        .collect();
    assert_eq!(
        shown,
        [&NativeMenu {
            menu: "theme".into(),
            anchor: button(600),
            rows: theme_rows(),
        }]
    );
    let closed: Vec<&String> = calls
        .iter()
        .filter_map(|call| match call {
            UiCall::Shell(script) if script.contains(r#""type":"nativeMenuClosed""#) => {
                Some(script)
            }
            _ => None,
        })
        .collect();
    assert_eq!(closed.len(), 1, "{calls:?}");
    assert!(closed[0].contains(r#""menu":"theme""#), "{}", closed[0]);
    assert!(closed[0].contains(r#""item":"light""#), "{}", closed[0]);
}

#[tokio::test]
async fn a_menu_closed_without_a_choice_names_no_row() {
    let setup = setup(config());
    let answer = send(&setup.core, &request("theme", theme_rows()));
    assert!(answer.contains("\"ack\""), "{answer}");
    let closed = setup
        .ui
        .calls()
        .into_iter()
        .find_map(|call| match call {
            UiCall::Shell(script) if script.contains("nativeMenuClosed") => Some(script),
            _ => None,
        })
        .expect("the shell hears that the menu closed");
    assert!(!closed.contains("\"item\""), "{closed}");
}

#[tokio::test]
async fn a_menu_that_cannot_be_drawn_is_refused_and_not_shown() {
    let setup = setup(config());
    let too_many: Vec<NativeMenuRow> = (0..=MAX_ROWS)
        .map(|index| item(&index.to_string(), "x", None))
        .collect();
    let twice = vec![item("a", "A", None), item("a", "B", None)];
    let nothing_to_choose = vec![
        NativeMenuRow::Heading { label: "H".into() },
        NativeMenuRow::Separator,
    ];
    let long_label = vec![item("a", &"x".repeat(MAX_TEXT + 1), None)];
    let empty_id = vec![item("", "A", None)];
    let long_menu = "m".repeat(MAX_MENU + 1);
    for (menu, rows) in [
        ("theme", Vec::new()),
        ("theme", too_many),
        ("theme", twice),
        ("theme", nothing_to_choose),
        ("theme", long_label),
        ("theme", empty_id),
        ("", theme_rows()),
        ("a menu", theme_rows()),
        (long_menu.as_str(), theme_rows()),
    ] {
        let answer = send(&setup.core, &request(menu, rows));
        assert!(answer.starts_with("error"), "{menu}: {answer}");
    }
    let far = ShellToCore::NativeMenu {
        menu: "theme".into(),
        anchor: MapRect {
            x: i32::MIN,
            y: 0,
            width: 1,
            height: 1,
        },
        rows: theme_rows(),
    };
    assert!(send(&setup.core, &far).starts_with("error"));
    let huge = ShellToCore::NativeMenu {
        menu: "theme".into(),
        anchor: MapRect {
            x: 0,
            y: 0,
            width: u32::MAX,
            height: 1,
        },
        rows: theme_rows(),
    };
    assert!(send(&setup.core, &huge).starts_with("error"));
    assert!(
        !setup
            .ui
            .calls()
            .iter()
            .any(|call| matches!(call, UiCall::NativeMenu(_)))
    );
}

// RB-090-45: only the toolbar's pulldowns ask for a native menu - each instance named
// `<kind>-<n>` by the shell (`useNativeMenu`); a name of another kind is refused.
#[tokio::test]
async fn only_a_toolbar_pulldown_may_ask_for_a_menu() {
    let setup = setup(config());
    for menu in [
        "theme",
        "theme-1",
        "environment-2",
        "layout-3",
        "bookmarks-4",
        "overflow-12",
    ] {
        let answer = send(&setup.core, &request(menu, theme_rows()));
        assert!(answer.contains("\"ack\""), "{menu}: {answer}");
    }
    let shown = setup
        .ui
        .calls()
        .iter()
        .filter(|call| matches!(call, UiCall::NativeMenu(_)))
        .count();
    for menu in [
        "zoom",
        "theme-",
        "theme-x",
        "themes-1",
        "overflow-1-2",
        "-1",
        "view:menu",
    ] {
        let answer = send(&setup.core, &request(menu, theme_rows()));
        assert!(answer.starts_with("error"), "{menu}: {answer}");
    }
    let after = setup
        .ui
        .calls()
        .iter()
        .filter(|call| matches!(call, UiCall::NativeMenu(_)))
        .count();
    assert_eq!(after, shown);
}

#[test]
fn the_pulldowns_are_named_by_their_kind() {
    assert!(is_pulldown("bookmarks"));
    assert!(is_pulldown("bookmarks-7"));
    assert!(!is_pulldown("bookmarks-7a"));
    assert!(!is_pulldown("Bookmarks-7"));
    assert!(!is_pulldown(""));
}

// The menu hangs below the button: from its left end while the button is in the left half of
// the shell view, from its right end in the right half - so it opens towards the window's
// middle and does not stand out of it (the owner's screenshot, 2026-11-16).
#[test]
fn the_menu_opens_below_the_button_towards_the_middle() {
    assert_eq!(
        placement(button(100), 1200),
        Placement {
            x: 100,
            y: 38,
            corner: Corner::TopLeft,
        }
    );
    assert_eq!(
        placement(button(1100), 1200),
        Placement {
            x: 1132,
            y: 38,
            corner: Corner::TopRight,
        }
    );
    // A view of no width (not laid out yet): from the left end.
    assert_eq!(placement(button(1100), 0).corner, Corner::TopLeft);
}

#[test]
fn a_label_keeps_its_ampersand_and_stays_on_one_line() {
    assert_eq!(menu_label("Docs & Blog"), "Docs && Blog");
    assert_eq!(menu_label("a\tb\nc"), "a b c");
    assert_eq!(menu_label("Dark (current)"), "Dark (current)");
}

#[test]
fn rows_become_entries_with_their_checks() {
    let rows = vec![
        NativeMenuRow::Heading {
            label: "Docs & more".into(),
        },
        item("system", "System", Some(false)),
        item("dark", "Dark", Some(true)),
        NativeMenuRow::Separator,
        NativeMenuRow::Item {
            id: "add".into(),
            label: "Add".into(),
            checked: None,
            disabled: true,
        },
    ];
    assert_eq!(
        entries(&rows),
        [
            Entry::Heading {
                index: 0,
                label: "Docs && more".into()
            },
            Entry::Item {
                index: 1,
                label: "System".into(),
                checked: Some(false),
                enabled: true,
            },
            Entry::Item {
                index: 2,
                label: "Dark".into(),
                checked: Some(true),
                enabled: true,
            },
            Entry::Separator,
            Entry::Item {
                index: 4,
                label: "Add".into(),
                checked: None,
                enabled: false,
            },
        ]
    );
}

/// A pending menu whose answer lands in the returned slot (`Some(answer)` once delivered).
fn pending(rows: Vec<NativeMenuRow>) -> (Pending, Arc<Mutex<Vec<Option<String>>>>) {
    let answers = Arc::new(Mutex::new(Vec::new()));
    let sink = answers.clone();
    let pending = Pending::new(rows, Box::new(move |item| rb_util::lock(&sink).push(item)));
    (pending, answers)
}

// CEF calls `execute_command` before `menu_closed` (it posts the latter): the choice is kept and
// told once, when the menu closed.
#[test]
fn the_choice_is_told_once_when_the_menu_closed() {
    let (menu, answers) = pending(theme_rows());
    menu.choose(2);
    assert!(rb_util::lock(&answers).is_empty());
    menu.close();
    menu.close();
    assert_eq!(*rb_util::lock(&answers), [Some("dark".to_owned())]);
}

#[test]
fn a_heading_a_disabled_row_or_no_row_chooses_nothing() {
    let rows = vec![
        NativeMenuRow::Heading { label: "H".into() },
        NativeMenuRow::Item {
            id: "off".into(),
            label: "Off".into(),
            checked: None,
            disabled: true,
        },
        item("on", "On", None),
    ];
    for index in [0, 1, 7] {
        let (menu, answers) = pending(rows.clone());
        menu.choose(index);
        menu.close();
        assert_eq!(*rb_util::lock(&answers), [None], "row {index}");
    }
}
