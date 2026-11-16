use serde_json::json;

use super::*;
use crate::{CoreToShell, MapRect, ShellToCore};

fn to_json<T: serde::Serialize>(value: &T) -> serde_json::Value {
    serde_json::to_value(value).expect("serializes")
}

// RB-090-41: the shell names the button and the rows; plain rows leave `checked` and
// `disabled` out.
#[test]
fn the_request_names_the_button_and_the_rows() {
    let request = ShellToCore::NativeMenu {
        menu: "theme".into(),
        anchor: MapRect {
            x: 600,
            y: 6,
            width: 32,
            height: 32,
        },
        rows: vec![
            NativeMenuRow::Heading {
                label: "Docs".into(),
            },
            NativeMenuRow::Item {
                id: "system".into(),
                label: "System".into(),
                checked: Some(true),
                disabled: false,
            },
            NativeMenuRow::Separator,
            NativeMenuRow::Item {
                id: "add".into(),
                label: "Bookmark this page".into(),
                checked: None,
                disabled: true,
            },
        ],
    };
    let wire = json!({
        "type": "nativeMenu",
        "menu": "theme",
        "anchor": { "x": 600, "y": 6, "width": 32, "height": 32 },
        "rows": [
            { "kind": "heading", "label": "Docs" },
            { "kind": "item", "id": "system", "label": "System", "checked": true },
            { "kind": "separator" },
            { "kind": "item", "id": "add", "label": "Bookmark this page", "disabled": true }
        ]
    });
    assert_eq!(to_json(&request), wire);
    let back: ShellToCore = serde_json::from_value(wire).expect("parses");
    assert_eq!(back, request);
}

#[test]
fn the_answer_names_the_row_chosen_or_none() {
    assert_eq!(
        to_json(&CoreToShell::NativeMenuClosed {
            menu: "theme".into(),
            item: Some("dark".into()),
        }),
        json!({ "type": "nativeMenuClosed", "menu": "theme", "item": "dark" })
    );
    assert_eq!(
        to_json(&CoreToShell::NativeMenuClosed {
            menu: "theme".into(),
            item: None,
        }),
        json!({ "type": "nativeMenuClosed", "menu": "theme" })
    );
}

#[test]
fn an_unknown_row_kind_is_refused() {
    let wire = json!({
        "type": "nativeMenu",
        "menu": "theme",
        "anchor": { "x": 0, "y": 0, "width": 1, "height": 1 },
        "rows": [{ "kind": "submenu", "label": "x" }]
    });
    assert!(crate::parse_request(&wire.to_string()).is_err());
}
