use rb_commands::builtin_reserved;
use serde_json::json;

use super::*;

fn config(shortcuts: &[(&str, &str)]) -> ProjectConfig {
    let bookmarks: Vec<_> = shortcuts
        .iter()
        .map(|(id, shortcut)| json!({ "id": id, "label": id, "path": "/", "shortcut": shortcut }))
        .collect();
    serde_json::from_value(json!({ "version": 1, "bookmarks": bookmarks }))
        .expect("the test config parses")
}

fn keys(shortcuts: &[(&str, &str)], held: &[(&str, &str)]) -> (Vec<BookmarkKey>, Vec<Conflict>) {
    let held: Vec<(String, Binding)> = held
        .iter()
        .map(|(id, text)| ((*id).to_owned(), text.parse().expect("a binding")))
        .collect();
    bookmark_keys(
        &config(shortcuts),
        &held,
        builtin_reserved(),
        Platform::Windows,
    )
}

fn shortcut(text: &str) -> Shortcut {
    text.parse().expect("a shortcut")
}

#[test]
fn a_free_combination_opens_its_bookmark() {
    let (keys, conflicts) = keys(&[("checkout", "Ctrl+Shift+F9")], &[]);
    assert!(conflicts.is_empty(), "{conflicts:?}");
    assert_eq!(
        bookmark_for(&keys, &shortcut("Ctrl+Shift+F9")),
        Some("checkout")
    );
    assert_eq!(bookmark_for(&keys, &shortcut("Ctrl+Shift+F10")), None);
}

#[test]
fn a_reserved_combination_is_refused_with_its_reason() {
    let (keys, conflicts) = keys(&[("checkout", "Ctrl+T")], &[]);
    assert!(keys.is_empty());
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].bookmark, "checkout");
    assert_eq!(conflicts[0].binding, "Ctrl+T");
    assert!(
        conflicts[0].reason_key.starts_with("shortcuts.reserved."),
        "{:?}",
        conflicts[0]
    );
}

#[test]
fn a_leader_chord_or_garbage_is_no_bookmark_shortcut() {
    let (keys, conflicts) = keys(&[("a", "Ctrl+Shift+Space P Z"), ("b", "Ctrl+Nope")], &[]);
    assert!(keys.is_empty());
    let reasons: Vec<&str> = conflicts.iter().map(|c| c.reason_key.as_str()).collect();
    assert_eq!(reasons, [REJECTED_CHORD, REJECTED_INVALID]);
}

#[test]
fn a_combination_a_command_or_a_breakpoint_holds_stays_theirs() {
    let (keys, conflicts) = keys(
        &[("a", "Ctrl+Shift+F9"), ("b", "Ctrl+Shift+F11")],
        &[
            ("view.set_breakpoint.1", "Ctrl+Shift+F9"),
            ("breakpoint mobile", "Ctrl+Shift+F11"),
        ],
    );
    assert!(keys.is_empty());
    let others: Vec<(&str, Option<&str>)> = conflicts
        .iter()
        .map(|c| (c.reason_key.as_str(), c.other.as_deref()))
        .collect();
    assert_eq!(
        others,
        [
            (REJECTED_CONFLICT, Some("view.set_breakpoint.1")),
            (REJECTED_CONFLICT, Some("breakpoint mobile")),
        ]
    );
}

#[test]
fn a_combination_two_bookmarks_claim_belongs_to_neither() {
    let (keys, conflicts) = keys(&[("a", "Ctrl+Shift+F9"), ("b", "Ctrl+Shift+F9")], &[]);
    assert!(keys.is_empty());
    assert_eq!(conflicts.len(), 2);
    assert_eq!(conflicts[0].other.as_deref(), Some("b"));
    assert_eq!(conflicts[1].other.as_deref(), Some("a"));
}
