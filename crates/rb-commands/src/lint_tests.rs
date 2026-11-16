use super::*;
use crate::registry::tests::def;
use crate::{Group, builtin, builtin_reserved};

fn lint(defs: &[CommandDef]) -> Vec<LintIssue> {
    lint_defaults(defs, builtin_reserved(), Platform::Windows)
}

fn kinds(defs: &[CommandDef]) -> Vec<LintKind> {
    lint(defs).into_iter().map(|issue| issue.kind).collect()
}

#[test]
fn the_core_definitions_are_clean() {
    let issues = lint(&builtin::definitions());
    assert!(issues.is_empty(), "{issues:#?}");
}

#[test]
fn a_command_without_a_shortcut_fails_the_coverage_check() {
    assert_eq!(
        kinds(&[def("view.test", Group::View, &[])]),
        [LintKind::Uncovered]
    );
    assert_eq!(
        kinds(&[def("view.test", Group::View, &["Ctrl+Shift+4"])]),
        [LintKind::NoLeaderChord]
    );
}

#[test]
fn a_default_ctrl_t_fails_the_lint() {
    let issues = lint(&[def(
        "view.test",
        Group::View,
        &["Ctrl+Shift+Space V T", "Ctrl+T"],
    )]);
    assert_eq!(issues.len(), 1, "{issues:#?}");
    assert_eq!(issues[0].binding.as_deref(), Some("Ctrl+T"));
    assert!(
        matches!(&issues[0].kind, LintKind::Reserved(reason) if reason.starts_with("Browser: new tab"))
    );
    assert_eq!(
        issues[0].to_string(),
        "view.test `Ctrl+T`: reserved - Browser: new tab (Chrome, Firefox)"
    );
}

#[test]
fn collisions_and_group_rules_are_found() {
    assert_eq!(
        kinds(&[
            def("sync.a", Group::Sync, &["Ctrl+Shift+Space S A"]),
            def("sync.b", Group::Sync, &["Ctrl+Shift+Space S A"]),
        ]),
        [LintKind::Conflict("sync.a".into())]
    );
    assert_eq!(
        kinds(&[def("sync.a", Group::Sync, &["Ctrl+Shift+Space V A"])]),
        [LintKind::ChordOutsideGroup('S')]
    );
    assert_eq!(
        kinds(&[
            def("sync.a", Group::Sync, &["Ctrl+Shift+Space S A"]),
            def("sync.a", Group::Sync, &["Ctrl+Shift+Space S B"]),
        ]),
        [LintKind::DuplicateId]
    );
    assert_eq!(
        kinds(&[def(
            "view.a",
            Group::View,
            &["Ctrl+Shift+Space V A", "Ctrl+Shift+Space"]
        )]),
        [LintKind::Reserved("the leader itself".into())]
    );
    assert_eq!(
        kinds(&[def("view.a", Group::View, &["Ctrl+Shift+Space ,"])]),
        [LintKind::Conflict("commands.group.settings".into())]
    );
}

#[test]
fn the_context_entries_name_a_command_that_exists() {
    let reset = def(
        "canvas.zoom.reset",
        Group::Layout,
        &["Ctrl+Shift+Space L 0"],
    );
    let found = lint_contexts(std::slice::from_ref(&reset), builtin_reserved());
    assert!(found.is_empty(), "{found:#?}");
    // Without the command the entry names nothing.
    let missing = lint_contexts(&[], builtin_reserved());
    assert_eq!(missing.len(), 1, "{missing:#?}");
    assert_eq!(missing[0].command, "canvas.zoom.reset");
    assert_eq!(missing[0].binding.as_deref(), Some("Ctrl+0"));
    assert!(matches!(&missing[0].kind, LintKind::Context(why) if why.contains("no command")));
}

#[test]
fn a_context_entry_is_one_exact_combination() {
    let mut list = builtin_reserved().clone();
    let entry = list
        .entries
        .iter_mut()
        .find(|entry| entry.combo == "Ctrl+0")
        .expect("the entry");
    entry.combo = "Ctrl+<digit>".to_owned();
    let reset = def(
        "canvas.zoom.reset",
        Group::Layout,
        &["Ctrl+Shift+Space L 0"],
    );
    let found = lint_contexts(&[reset], &list);
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(matches!(&found[0].kind, LintKind::Context(why) if why.contains("exact")));
}
