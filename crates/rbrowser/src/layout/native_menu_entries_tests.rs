use std::sync::{Arc, Mutex};

use rb_ipc::NativeMenuRow;

use super::*;

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

fn theme_rows() -> Vec<NativeMenuRow> {
    vec![
        item("system", "System (Windows: light)", Some(false)),
        item("light", "Light", Some(false)),
        item("dark", "Dark", Some(true)),
    ]
}

#[test]
fn a_label_keeps_its_ampersand_and_stays_on_one_line() {
    assert_eq!(menu_label("Docs & Blog"), "Docs & Blog");
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
            radio: false,
            accel: None,
            disabled: true,
        },
    ];
    let plain = |index, label: &str, checked, enabled| Entry::Item {
        index,
        label: label.into(),
        checked,
        radio: false,
        accelerator: None,
        enabled,
    };
    assert_eq!(
        entries(&rows),
        [
            Entry::Heading {
                index: 0,
                label: "Docs & more".into()
            },
            plain(1, "System", Some(false), true),
            plain(2, "Dark", Some(true), true),
            Entry::Separator,
            plain(4, "Add", None, false),
        ]
    );
}

// RB-090-63: a submenu takes the index before its rows; the rows after it count on from its
// last row, so every row of the tree has an index of its own.
#[test]
fn a_submenu_and_its_rows_are_numbered_depth_first() {
    let rows = vec![
        item("a", "A", None),
        NativeMenuRow::Submenu {
            label: "Layout & more".into(),
            rows: vec![
                NativeMenuRow::Item {
                    id: "grid".into(),
                    label: "Grid".into(),
                    checked: Some(true),
                    radio: true,
                    accel: None,
                    disabled: false,
                },
                item("b", "B", None),
            ],
        },
        item("c", "C", None),
    ];
    let shown = entries(&rows);
    assert_eq!(shown.len(), 3);
    let Entry::Submenu {
        index,
        label,
        entries: inner,
    } = &shown[1]
    else {
        panic!("a submenu: {shown:?}");
    };
    assert_eq!((*index, label.as_str()), (1, "Layout & more"));
    assert!(matches!(
        inner[0],
        Entry::Item {
            index: 2,
            radio: true,
            checked: Some(true),
            ..
        }
    ));
    assert!(matches!(inner[1], Entry::Item { index: 3, .. }));
    assert!(matches!(shown[2], Entry::Item { index: 4, .. }));
}

// A direct shortcut is the menu's accelerator; a leader chord, which no accelerator shows,
// stands after the label.
#[test]
fn a_direct_shortcut_is_an_accelerator_and_a_chord_follows_the_label() {
    assert_eq!(
        accelerator("Ctrl+Shift+1"),
        Some(Accelerator {
            key_code: 0x31,
            shift: true,
            ctrl: true,
            alt: false,
        })
    );
    assert_eq!(accelerator("F12").map(|found| found.key_code), Some(0x7B));
    assert_eq!(accelerator("Ctrl+Shift+Space L G"), None);
    assert_eq!(accelerator("Win+E"), None);
    assert_eq!(accelerator("no shortcut"), None);
    let rows = vec![
        NativeMenuRow::Item {
            id: "one".into(),
            label: "Breakpoint 1".into(),
            checked: None,
            radio: false,
            accel: Some("Ctrl+Shift+1".into()),
            disabled: false,
        },
        NativeMenuRow::Item {
            id: "grid".into(),
            label: "Grid".into(),
            checked: None,
            radio: false,
            accel: Some("Ctrl+Shift+Space L G".into()),
            disabled: false,
        },
    ];
    let shown = entries(&rows);
    assert!(matches!(
        &shown[0],
        Entry::Item { label, accelerator: Some(_), .. } if label == "Breakpoint 1"
    ));
    assert!(matches!(
        &shown[1],
        Entry::Item { label, accelerator: None, .. }
            if label == "Grid\u{2003}\u{2003}Ctrl+Shift+Space L G"
    ));
}

/// A pending menu whose answer lands in the returned slot (`Some(answer)` once delivered).
fn pending(rows: &[NativeMenuRow]) -> (Pending, Arc<Mutex<Vec<Option<String>>>>) {
    let answers = Arc::new(Mutex::new(Vec::new()));
    let sink = answers.clone();
    let pending = Pending::new(rows, Box::new(move |item| rb_util::lock(&sink).push(item)));
    (pending, answers)
}

// CEF calls `execute_command` before `menu_closed` (it posts the latter): the choice is kept and
// told once, when the menu closed.
#[test]
fn the_choice_is_told_once_when_the_menu_closed() {
    let (menu, answers) = pending(&theme_rows());
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
            radio: false,
            accel: None,
            disabled: true,
        },
        item("on", "On", None),
        NativeMenuRow::Submenu {
            label: "S".into(),
            rows: vec![item("in", "In", None)],
        },
    ];
    for index in [0, 1, 3, 7] {
        let (menu, answers) = pending(&rows);
        menu.choose(index);
        menu.close();
        assert_eq!(*rb_util::lock(&answers), [None], "row {index}");
    }
}

// RB-090-63: a row in a submenu is chosen by its index in the tree.
#[test]
fn a_row_in_a_submenu_is_chosen_by_its_index() {
    let rows = vec![
        item("a", "A", None),
        NativeMenuRow::Submenu {
            label: "S".into(),
            rows: vec![item("in", "In", None)],
        },
        item("after", "After", None),
    ];
    for (index, id) in [(2, "in"), (3, "after"), (0, "a")] {
        let (menu, answers) = pending(&rows);
        menu.choose(index);
        menu.close();
        assert_eq!(
            *rb_util::lock(&answers),
            [Some(id.to_owned())],
            "row {index}"
        );
    }
}
