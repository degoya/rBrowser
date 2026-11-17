use super::*;

#[test]
fn every_rule_is_described_once() {
    let described = describe();
    let mut ids: Vec<&str> = described.iter().map(|rule| rule.id.as_str()).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), RULES.len());
    assert!(
        RENDERED_RULES
            .iter()
            .all(|id| RULES.iter().any(|rule| rule.id == *id))
    );
}

#[test]
fn the_sink_keeps_the_rules_the_run_asks_for() {
    let enabled = vec![TEXT_TOO_SMALL.to_owned()];
    let mut sink = Sink::new(&enabled, 3, "http://x/");
    let at = Located {
        selector: "p".to_owned(),
        path: "html > body > p".to_owned(),
    };
    sink.emit(
        TEXT_TOO_SMALL,
        TEXT_TOO_SMALL,
        &At::Element(&at),
        &[("size", "11".to_owned())],
    );
    sink.emit(
        FACE_UNUSED,
        FACE_UNUSED,
        &At::Named("@font-face X".to_owned()),
        &[],
    );
    sink.emit("no-such-rule", "x", &At::Element(&at), &[]);
    assert_eq!(sink.findings.len(), 1);
    let finding = &sink.findings[0];
    assert_eq!(finding.message, "plugin.fonts.findings.text-too-small");
    assert_eq!(finding.view, Some(3));
    assert_eq!(finding.selector.as_deref(), Some("p"));
    assert_eq!(finding.params, vec![("size".to_owned(), "11".to_owned())]);
}

#[test]
fn a_named_finding_has_a_path_but_no_selector() {
    let mut sink = Sink::new(&[], 1, "http://x/");
    sink.emit(
        FACE_UNUSED,
        FACE_UNUSED,
        &At::Named("@font-face X 400 normal".to_owned()),
        &[],
    );
    assert_eq!(sink.findings[0].selector, None);
    assert_eq!(
        sink.findings[0].path.as_deref(),
        Some("@font-face X 400 normal")
    );
    assert_eq!(sink.findings[0].severity, Severity::Info);
}

#[test]
fn numbers_are_shown_short() {
    assert_eq!(shown(14.0), "14");
    assert_eq!(shown(1.2), "1.2");
    assert_eq!(shown(1.155), "1.16");
    assert_eq!(shown(16.754), "16.75");
}
