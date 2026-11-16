//! The plugin through its native trait against `MemoryHost` (no capability granted), and the
//! export of the K12 fixture recording held against the files in
//! `tests/e2e/recorder/generated/` - the ones `tests/e2e/recorder/run.sh` type-checks and runs
//! with `npx playwright test`. `RB_UPDATE_SNAPSHOTS=1 cargo test` rewrites them.

use std::path::PathBuf;

use rb_plugin_api::FailureKind;
use rb_plugin_api::export::{Exporter, valid_file_name};
use rb_plugin_api::test_support::MemoryHost;

use super::{CONFIG_FILE, FLOW, PLAYWRIGHT_CONFIG, PLUGIN_ID, RecorderExport, SPEC, render};

fn e2e_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/e2e/recorder")
}

fn fixture() -> String {
    std::fs::read_to_string(e2e_dir().join("login.export.json")).expect("the K12 fixture")
}

fn rendered(format: &str) -> (String, String) {
    let output = RecorderExport
        .render(&mut MemoryHost::new(), format, &fixture())
        .expect("rendered");
    (output.file_name, output.text)
}

#[test]
fn it_lists_the_flow_the_spec_and_the_config_with_locale_titles() {
    let locales: serde_json::Value =
        serde_json::from_str(include_str!("../locales/en.json")).expect("the plugin's locales");
    let formats = RecorderExport.formats();
    let ids: Vec<&str> = formats.iter().map(|format| format.id.as_str()).collect();
    assert_eq!(ids, [FLOW, SPEC, PLAYWRIGHT_CONFIG]);
    for format in &formats {
        let pointer = format!("/{}", format.title.replace('.', "/"));
        assert!(
            locales.pointer(&pointer).is_some(),
            "{} has no text",
            format.title
        );
    }
}

#[test]
fn every_error_key_has_a_text() {
    let locales: serde_json::Value =
        serde_json::from_str(include_str!("../locales/en.json")).expect("the plugin's locales");
    for key in ["bad_recording", "version", "step", "too_large"] {
        assert!(
            locales
                .pointer(&format!("/plugin/{PLUGIN_ID}/errors/{key}"))
                .is_some(),
            "{key}"
        );
    }
}

#[test]
fn the_files_are_named_after_the_recording() {
    assert_eq!(rendered(FLOW).0, "login.ts");
    assert_eq!(rendered(SPEC).0, "login.spec.ts");
    assert_eq!(rendered(PLAYWRIGHT_CONFIG).0, CONFIG_FILE);
    for format in [FLOW, SPEC, PLAYWRIGHT_CONFIG] {
        assert!(valid_file_name(&rendered(format).0));
    }
}

#[test]
fn a_name_no_file_may_have_becomes_recording() {
    let input = fixture().replacen("\"name\": \"login\"", "\"name\": \"../../evil\"", 1);
    let output = render(SPEC, &input).expect("rendered");
    assert_eq!(output.file_name, "recording.spec.ts");
}

#[test]
fn an_unknown_format_is_not_supported() {
    let failure = render("cypress", &fixture()).expect_err("unknown");
    assert_eq!(failure.kind, FailureKind::NotSupported);
    assert_eq!(failure.code(), Some("plugins.error.unknown_contribution"));
    assert_eq!(failure.param("name"), Some("cypress"));
}

#[test]
fn what_the_conformance_run_sends_is_refused_on_purpose() {
    let failure = render(FLOW, "{}").expect_err("no recording");
    assert_eq!(failure.kind, FailureKind::BadData);
    assert_eq!(
        failure.code(),
        Some("plugin.recorder-export.errors.bad_recording")
    );
}

#[test]
fn another_format_version_names_it() {
    let input = fixture().replacen("\"version\": 1", "\"version\": 7", 1);
    let failure = render(SPEC, &input).expect_err("version 7");
    assert_eq!(
        failure.code(),
        Some("plugin.recorder-export.errors.version")
    );
    assert_eq!(failure.param("version"), Some("7"));
}

#[test]
fn the_k12_exports_match_the_files_the_e2e_run_uses() {
    let update = std::env::var_os("RB_UPDATE_SNAPSHOTS").is_some();
    for format in [FLOW, SPEC, PLAYWRIGHT_CONFIG] {
        let (name, text) = rendered(format);
        let path = e2e_dir().join("generated").join(&name);
        if update {
            std::fs::write(&path, &text).expect("the snapshot written");
            continue;
        }
        let expected = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        assert_eq!(
            text, expected,
            "{name} changed: RB_UPDATE_SNAPSHOTS=1 cargo test, then review the diff"
        );
    }
}

#[test]
fn no_secret_is_written_in_clear() {
    for format in [FLOW, SPEC] {
        let (_, text) = rendered(format);
        assert!(!text.contains("{{secret"), "{format}");
        assert!(text.contains("rbSecret('RB_SECRET_PASSWORD')"), "{format}");
        assert!(!text.contains("?? ''"), "{format}");
        assert!(text.contains("RB_SECRET_PASSWORD"), "{format}");
    }
}

#[test]
fn every_breakpoint_is_one_project() {
    let (_, text) = rendered(PLAYWRIGHT_CONFIG);
    for name in ["mobile", "tablet", "desktop"] {
        assert_eq!(
            text.matches(&format!("      name: '{name}',\n")).count(),
            1,
            "{name}"
        );
    }
    assert_eq!(text.matches("      name: ").count(), 3);
}

#[test]
fn only_in_steps_are_guarded_by_the_project_or_the_breakpoint() {
    let (_, spec) = rendered(SPEC);
    assert_eq!(
        spec.matches("if (['mobile'].includes(test.info().project.name)) {")
            .count(),
        2
    );
    let (_, flow) = rendered(FLOW);
    assert_eq!(
        flow.matches("if (['mobile'].includes(view.breakpoint)) {")
            .count(),
        2
    );
}

#[test]
fn the_locator_preference_changes_the_locators() {
    let input = fixture().replacen(
        "\"locatorPreference\": \"auto\"",
        "\"locatorPreference\": \"css\"",
        1,
    );
    let text = render(SPEC, &input).expect("rendered").text;
    assert!(!text.contains("getByTestId"), "{text}");
    assert!(text.contains("page.locator('#email').fill('alice@example.org');"));
}
