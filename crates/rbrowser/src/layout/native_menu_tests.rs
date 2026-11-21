use rb_ipc::{MapRect, NativeMenuRow, ShellToCore};

use super::*;
use crate::host::ui::recording::UiCall;
use crate::layout::test_support::{config, setup};

fn item(id: &str, label: &str, checked: Option<bool>) -> NativeMenuRow {
    NativeMenuRow::Item {
        id: id.into(),
        label: label.into(),
        checked,
        radio: false,
        accel: None,
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
        "portal-5",
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
        "portals-1",
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

// RB-090-62: the portal's button in the toolbar opens its menu as the other pulldowns do.
#[test]
fn the_portal_button_is_a_pulldown() {
    assert!(PULLDOWNS.contains(&"portal"));
    assert!(is_pulldown("portal-3"));
    assert!(!is_pulldown("portal-x"));
}

// RB-094-20: the toolbar's "+" builds its menu in the shell, and it and the main menu list the
// devices - opening either takes the device list again for the next opening.
#[test]
fn the_add_button_is_a_pulldown_and_the_device_menus_refresh_the_list() {
    assert!(PULLDOWNS.contains(&"add"));
    assert!(is_pulldown("add-4"));
    assert!(lists_devices("add-4"));
    assert!(lists_devices("menu-1"));
    assert!(!lists_devices("theme-2"));
    assert!(!lists_devices("checks-1"));
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

// RB-090-63: the main menu `≡` and the split buttons ask for their menus as the pulldowns do.
#[test]
fn the_main_menu_and_the_split_buttons_are_pulldowns() {
    for kind in ["menu-1", "devtools-2", "capture-3", "video-4", "checks-5"] {
        assert!(is_pulldown(kind), "{kind}");
    }
}

fn submenu(label: &str, rows: Vec<NativeMenuRow>) -> NativeMenuRow {
    NativeMenuRow::Submenu {
        label: label.into(),
        rows,
    }
}

/// `depth` submenus, one in the other, around one row.
fn nested(depth: usize) -> Vec<NativeMenuRow> {
    let mut rows = vec![item("deep", "Deep", None)];
    for level in 0..depth {
        rows = vec![submenu(&format!("Level {level}"), rows)];
    }
    rows
}

fn radio(id: &str, checked: Option<bool>) -> NativeMenuRow {
    NativeMenuRow::Item {
        id: id.into(),
        label: id.into(),
        checked,
        radio: true,
        accel: None,
        disabled: false,
    }
}

fn with_accel(id: &str, accel: &str) -> NativeMenuRow {
    NativeMenuRow::Item {
        id: id.into(),
        label: id.into(),
        checked: None,
        radio: false,
        accel: Some(accel.into()),
        disabled: false,
    }
}

// RB-090-63: a menu with submenus, radio rows and shortcuts is shown; the tree is bounded - its
// depth, its rows in all, ids unique across every level, a radio row says whether it is chosen,
// a shortcut is one of the command registry.
#[tokio::test]
async fn a_tree_of_submenus_is_shown_within_its_bounds() {
    let setup = setup(config());
    let fine = vec![
        submenu(
            "Layout",
            vec![
                radio("grid", Some(true)),
                radio("canvas", Some(false)),
                with_accel("fit", "Ctrl+Shift+Space L A"),
            ],
        ),
        with_accel("one", "Ctrl+Shift+1"),
    ];
    let answer = send(&setup.core, &request("menu-1", fine));
    assert!(answer.contains("\"ack\""), "{answer}");
    let answer = send(&setup.core, &request("menu-2", nested(MAX_DEPTH - 1)));
    assert!(answer.contains("\"ack\""), "{answer}");

    let wide: Vec<NativeMenuRow> = (0..5)
        .map(|block| {
            submenu(
                &format!("Block {block}"),
                (0..MAX_ROWS)
                    .map(|row| item(&format!("{block}.{row}"), "x", None))
                    .collect(),
            )
        })
        .collect();
    let twice_deep = vec![
        item("a", "A", None),
        submenu("More", vec![item("a", "Again", None)]),
    ];
    for (name, rows) in [
        ("too deep", nested(MAX_DEPTH)),
        ("too many in all", wide),
        ("an id twice across levels", twice_deep),
        (
            "an empty submenu",
            vec![item("a", "A", None), submenu("Empty", Vec::new())],
        ),
        ("a radio row without its state", vec![radio("r", None)]),
        (
            "no shortcut",
            vec![with_accel("a", "Ctrl+Shift+Space Q W E")],
        ),
        (
            "a long shortcut",
            vec![with_accel("a", &"x".repeat(MAX_ACCEL + 1))],
        ),
        (
            "only submenus without rows",
            vec![submenu("S", vec![NativeMenuRow::Separator])],
        ),
    ] {
        let answer = send(&setup.core, &request("menu-3", rows));
        assert!(answer.starts_with("error"), "{name}: {answer}");
    }
}
