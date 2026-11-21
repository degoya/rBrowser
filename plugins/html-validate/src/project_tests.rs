use rb_plugin_api::{Failure, FailureKind, Severity, codes};
use serde_json::json;

use super::{Level, MAX_FILE_BYTES, PROJECT_FILE, ProjectRules, State, level_of, load, parse};
use crate::fake::FakeHost;

fn reason(text: &str) -> String {
    let failure = parse(text.as_bytes()).expect_err(text);
    assert_eq!(
        failure.code(),
        Some("plugin.html-validate.errors.project_rules"),
        "{text}"
    );
    failure.param("reason").unwrap_or_default().to_owned()
}

#[test]
fn reads_levels_and_options_of_the_plugins_rules() {
    let rules = parse(
        br#"{
            "$schema": "https://html-validate.org/schemas/config.json",
            "rules": {
                "no-dup-id": "off",
                "close-order": "warn",
                "no-unknown-attributes": 2,
                "no-raw-characters": ["error", { "relaxed": false }],
                "void-content": ["warn"]
            }
        }"#,
    )
    .expect("rules");
    let project = ProjectRules::applied(rules);
    assert_eq!(project.level("no-dup-id"), Some(Level::Off));
    assert_eq!(project.level("close-order"), Some(Level::Warn));
    assert_eq!(project.level("no-unknown-attributes"), Some(Level::Error));
    assert_eq!(
        project.config("no-unknown-attributes"),
        Some(&json!("error"))
    );
    assert_eq!(
        project.config("no-raw-characters"),
        Some(&json!(["error", { "relaxed": false }]))
    );
    assert_eq!(project.config("void-content"), Some(&json!("warn")));
    assert_eq!(project.level("element-name"), None);
    assert_eq!(project.state, State::Applied);
}

#[test]
fn an_empty_file_object_sets_nothing() {
    assert!(parse(b"{}").expect("empty").is_empty());
    assert!(parse(br#"{"rules": {}}"#).expect("no rules").is_empty());
}

#[test]
fn only_rules_are_read_never_what_loads_code() {
    for key in [
        "plugins",
        "extends",
        "elements",
        "transform",
        "root",
        "preset",
    ] {
        let text = format!(r#"{{"{key}": ["x"], "rules": {{}}}}"#);
        let reason = reason(&text);
        assert!(reason.contains(key), "{reason}");
    }
}

#[test]
fn a_rule_the_plugin_does_not_offer_is_refused() {
    for id in ["no-inline-style", "nu-error", "Bad Id", "attr-quotes"] {
        let text = json!({ "rules": { id: "error" } }).to_string();
        assert!(reason(&text).contains("no rule"), "{id}");
    }
}

#[test]
fn levels_and_option_shapes_are_held_to_the_subset() {
    for value in [
        json!("fatal"),
        json!(3),
        json!(true),
        json!([]),
        json!(["error", { "a": 1 }, "extra"]),
        json!(["error", "options"]),
        json!({ "level": "error" }),
        json!(["error", { "pattern": "x".repeat(600) }]),
        json!(["error", { "deep": { "deeper": { "deepest": { "too": 1 } } } }]),
        json!(["error", { "list": vec![1; 65] }]),
        json!(["error", { "bad key": 1 }]),
    ] {
        let text = json!({ "rules": { "no-raw-characters": value } }).to_string();
        reason(&text);
    }
}

#[test]
fn the_file_is_a_bounded_json_object() {
    assert!(reason("[1]").contains("object"));
    assert!(reason("{").starts_with("not JSON"));
    assert!(reason(r#"{"rules": []}"#).contains("rules"));
    let big = format!(
        r#"{{"rules": {{}}, "$schema": "{}"}}"#,
        "x".repeat(MAX_FILE_BYTES)
    );
    assert!(reason(&big).contains("64 KiB"));
}

#[test]
fn level_of_reads_html_validates_forms() {
    assert_eq!(level_of(&json!("off")), Some(Level::Off));
    assert_eq!(level_of(&json!(0)), Some(Level::Off));
    assert_eq!(level_of(&json!(1)), Some(Level::Warn));
    assert_eq!(level_of(&json!(["warn", {}])), Some(Level::Warn));
    assert_eq!(level_of(&json!("error")), Some(Level::Error));
    assert_eq!(level_of(&json!("other")), None);
    assert_eq!(Level::Off.severity(), None);
    assert_eq!(Level::Warn.severity(), Some(Severity::Warning));
    assert_eq!(Level::Error.severity(), Some(Severity::Error));
}

#[test]
fn a_trusted_project_applies_its_file() {
    let mut host = FakeHost::default();
    host.project = Some(Ok(br#"{"rules": {"no-dup-id": "off"}}"#.to_vec()));
    host.trusted = true;
    let project = load(&mut host).expect("project");
    assert_eq!(project.state, State::Applied);
    assert_eq!(project.level("no-dup-id"), Some(Level::Off));
    assert_eq!(host.project_reads, [PROJECT_FILE]);
}

#[test]
fn an_untrusted_project_is_never_applied_nor_parsed() {
    let mut host = FakeHost::default();
    // Not even a broken file fails the run: it is never read as rules.
    host.project = Some(Ok(b"{ broken".to_vec()));
    host.trusted = false;
    let project = load(&mut host).expect("project");
    assert_eq!(project.state, State::Untrusted);
    assert_eq!(project.level("no-dup-id"), None);
    assert!(host.logs.iter().any(|(_, line)| line.contains("untrusted")));
}

#[test]
fn no_file_and_no_project_set_nothing() {
    for failure in [
        Failure::coded(FailureKind::NotFound, codes::NOT_FOUND),
        Failure::coded(FailureKind::NotSupported, codes::NOT_AVAILABLE),
        rb_plugin_api::Capability::ProjectRead.denied(),
    ] {
        let mut host = FakeHost::default();
        host.project = Some(Err(failure));
        host.trusted = true;
        let project = load(&mut host).expect("nothing");
        assert_eq!(project, ProjectRules::default());
        assert_eq!(project.state.as_str(), "none");
    }
}

#[test]
fn a_file_that_does_not_read_fails_with_the_reason() {
    let mut host = FakeHost::default();
    host.project = Some(Err(Failure::new(FailureKind::Io)));
    host.trusted = true;
    let failure = load(&mut host).expect_err("io");
    assert_eq!(
        failure.code(),
        Some("plugin.html-validate.errors.project_rules")
    );
    let mut host = FakeHost::default();
    host.project = Some(Ok(br#"{"plugins": ["my-plugin"]}"#.to_vec()));
    host.trusted = true;
    let failure = load(&mut host).expect_err("plugins");
    assert_eq!(
        failure.code(),
        Some("plugin.html-validate.errors.project_rules")
    );
}
