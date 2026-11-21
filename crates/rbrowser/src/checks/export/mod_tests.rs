use rb_plugin_types::FailOn;
use serde_json::Value;

use super::testing::{MARKUP, fixture, with_context};
use super::*;

#[test]
fn a_message_is_its_text_with_the_parameters_filled_in() {
    let file = fixture();
    with_context(|context| {
        let meta = &file.findings[4];
        assert_eq!(
            context.message(&meta.message),
            format!("The meta tag {MARKUP} is missing")
        );
        let unknown = FindingMessage {
            key: "plugin.x.unknown".to_owned(),
            params: BTreeMap::new(),
        };
        assert_eq!(context.message(&unknown), "plugin.x.unknown");
    });
}

#[test]
fn a_view_is_named_by_its_breakpoint_else_by_its_id() {
    let file = fixture();
    with_context(|context| {
        assert_eq!(context.view_name(&file, 2), "desktop");
        assert_eq!(context.view_name(&file, 9), "view(id=9)");
    });
}

#[test]
fn the_engines_of_a_finding_are_each_named_once() {
    let mut finding = fixture().findings[0].clone();
    finding.views = vec![1, 2, 3];
    with_context(|context| {
        assert_eq!(context.engines_of(&finding), ["chromium", "firefox"]);
    });
}

#[test]
fn every_format_has_its_name_and_extension() {
    for (name, extension_of) in [
        ("sarif", "sarif"),
        ("junit", "xml"),
        ("html", "html"),
        ("json", "json"),
    ] {
        let format = parse_format(name).expect("a format");
        assert_eq!(format_name(format), name);
        assert_eq!(extension(format), extension_of);
    }
    assert_eq!(parse_format("pdf"), None);
    assert_eq!(parse_format("SARIF"), None);
}

#[test]
fn the_setting_maps_to_the_threshold() {
    assert_eq!(fail_on(FindingsFailOn::Error), FailOn::Error);
    assert_eq!(fail_on(FindingsFailOn::Warning), FailOn::Warning);
    assert_eq!(fail_on(FindingsFailOn::Info), FailOn::Info);
}

#[test]
fn the_json_export_is_the_run_itself() {
    let file = fixture();
    let text = with_context(|context| render(&file, ExportFormat::Json, context)).expect("json");
    assert!(text.ends_with('\n'));
    let back: FindingsFile = serde_json::from_str(&text).expect("a findings file");
    assert_eq!(back, file);
}

#[test]
fn every_format_renders_the_fixture() {
    let file = fixture();
    for format in [
        ExportFormat::Sarif,
        ExportFormat::Junit,
        ExportFormat::Html,
        ExportFormat::Json,
    ] {
        let text = with_context(|context| render(&file, format, context)).expect("renders");
        assert!(!text.is_empty(), "{format:?}");
        if matches!(format, ExportFormat::Sarif | ExportFormat::Json) {
            serde_json::from_str::<Value>(&text).expect("valid JSON");
        }
    }
}
