use rb_commands::builtin_reserved;
use serde_json::json;

use super::*;

fn config(shortcuts: &[(&str, &str)]) -> ProjectConfig {
    let breakpoints: Vec<_> = shortcuts
        .iter()
        .enumerate()
        .map(|(index, (id, shortcut))| {
            json!({ "id": id, "width": 400 + index, "height": 800, "shortcut": shortcut })
        })
        .collect();
    serde_json::from_value(json!({ "version": 1, "breakpoints": breakpoints }))
        .expect("the test config parses")
}

fn keys(shortcuts: &[(&str, &str)], held: &[(&str, &str)]) -> (Vec<ProjectKey>, Vec<Conflict>) {
    let held: Vec<(String, Binding)> = held
        .iter()
        .map(|(id, text)| ((*id).to_owned(), text.parse().expect("a binding")))
        .collect();
    project_keys(
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
fn breakpoint_shortcuts_register_a_free_project_shortcut() {
    let (keys, conflicts) = keys(&[("mobile", "Ctrl+Shift+F9")], &[]);
    assert!(conflicts.is_empty(), "{conflicts:?}");
    assert_eq!(
        keys,
        [ProjectKey {
            shortcut: shortcut("Ctrl+Shift+F9"),
            breakpoint: "mobile".to_owned()
        }]
    );
    assert_eq!(
        breakpoint_for(&keys, &shortcut("Ctrl+Shift+F9")),
        Some("mobile")
    );
    assert_eq!(breakpoint_for(&keys, &shortcut("Ctrl+Shift+F10")), None);
}

#[test]
fn breakpoint_shortcuts_report_a_reserved_project_shortcut() {
    // The plan's example: Alt+<letter> is a page accesskey.
    let (keys, conflicts) = keys(&[("mobile", "Alt+M")], &[]);
    assert!(keys.is_empty());
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].breakpoint, "mobile");
    assert_eq!(conflicts[0].binding, "Alt+M");
    assert!(
        conflicts[0].reason_key.starts_with("shortcuts.reserved."),
        "{:?}",
        conflicts[0]
    );
}

#[test]
fn breakpoint_shortcuts_report_a_shortcut_two_breakpoints_claim() {
    let (keys, conflicts) = keys(
        &[
            ("mobile", "Ctrl+Shift+F9"),
            ("tablet", "Ctrl+Shift+F9"),
            ("desktop", "Ctrl+Shift+F11"),
        ],
        &[],
    );
    assert_eq!(keys.len(), 1);
    assert_eq!(keys[0].breakpoint, "desktop");
    let reported: Vec<(&str, Option<&str>)> = conflicts
        .iter()
        .map(|c| (c.breakpoint.as_str(), c.other.as_deref()))
        .collect();
    assert_eq!(
        reported,
        [("mobile", Some("tablet")), ("tablet", Some("mobile"))]
    );
    assert!(conflicts.iter().all(|c| c.reason_key == REJECTED_CONFLICT));
}

#[test]
fn breakpoint_shortcuts_report_a_shortcut_a_command_holds() {
    let (keys, conflicts) = keys(
        &[("mobile", "Ctrl+Shift+2")],
        &[("view.set_breakpoint.2", "Ctrl+Shift+2")],
    );
    assert!(keys.is_empty());
    assert_eq!(conflicts[0].reason_key, REJECTED_CONFLICT);
    assert_eq!(conflicts[0].other.as_deref(), Some("view.set_breakpoint.2"));
}

#[test]
fn breakpoint_shortcuts_refuse_chords_and_malformed_project_shortcuts() {
    let (keys, conflicts) = keys(
        &[("mobile", "Ctrl+Shift+Space B M"), ("tablet", "Ctrl+Nope")],
        &[],
    );
    assert!(keys.is_empty());
    let reasons: Vec<&str> = conflicts.iter().map(|c| c.reason_key.as_str()).collect();
    assert_eq!(reasons, [REJECTED_CHORD, REJECTED_INVALID]);
}

#[test]
fn breakpoint_shortcuts_skip_plain_typing_early() {
    assert!(!may_be_project_key(&shortcut("A")));
    assert!(!may_be_project_key(&shortcut("Shift+A")));
    assert!(may_be_project_key(&shortcut("Ctrl+Shift+F9")));
    assert!(may_be_project_key(&shortcut("F4")));
    assert!(may_be_project_key(&shortcut("Alt+M")));
}

#[test]
fn breakpoint_shortcuts_list_every_project_shortcut_in_config_order_with_its_conflict() {
    let shortcuts = [
        ("wide", "Ctrl+Shift+F9"),
        ("tablet", "Alt+M"),
        ("mobile", "Ctrl+Shift+F10"),
        ("phone", "Ctrl+Shift+F10"),
        ("watch", "Ctrl+Shift+F11"),
    ];
    let (found, conflicts) = keys(&shortcuts, &[("view.reload", "Ctrl+Shift+F11")]);
    let rows = listing(&config(&shortcuts), &found, &conflicts);
    let order: Vec<&str> = rows.iter().map(|row| row.breakpoint.as_str()).collect();
    assert_eq!(order, ["wide", "tablet", "mobile", "phone", "watch"]);
    assert_eq!(rows[0].binding, "Ctrl+Shift+F9");
    assert_eq!(rows[0].reason_key, None, "in force");
    assert!(
        rows[1]
            .reason_key
            .as_deref()
            .is_some_and(|key| key.starts_with("shortcuts.reserved.")),
        "{:?}",
        rows[1]
    );
    assert_eq!(rows[1].binding, "Alt+M");
    // Two breakpoints claim the same combination: neither has it.
    assert_eq!(rows[2].reason_key.as_deref(), Some(REJECTED_CONFLICT));
    assert_eq!(rows[2].other.as_deref(), Some("phone"));
    assert_eq!(rows[3].other.as_deref(), Some("mobile"));
    assert_eq!(rows[4].reason_key.as_deref(), Some(REJECTED_CONFLICT));
    assert_eq!(rows[4].other.as_deref(), Some("view.reload"));
}

#[test]
fn breakpoint_shortcuts_list_nothing_without_project_shortcuts() {
    let config: ProjectConfig = serde_json::from_value(json!({
        "version": 1,
        "breakpoints": [{ "id": "mobile", "width": 393, "height": 852 }]
    }))
    .expect("the test config parses");
    assert!(listing(&config, &[], &[]).is_empty());
}
