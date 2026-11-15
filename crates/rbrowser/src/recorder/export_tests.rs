use rb_config::{ColorScheme, Environment, LocatorPreference};
use rb_sync::recorder::{RECORDING_VERSION, RecordedBreakpoint, Recording, RecordingStore};
use serde_json::{Value, json};

use super::*;
use crate::host::state::default_config;
use crate::recorder::run_command;
use crate::sync_hub::tests::{Setup, setup};

fn recording(breakpoints: &[(&str, u32, u32)]) -> Recording {
    Recording {
        schema: None,
        version: RECORDING_VERSION,
        name: "login".into(),
        start_url: Some("http://127.0.0.1:8000/".into()),
        environment: Some("local".into()),
        breakpoints: breakpoints
            .iter()
            .map(|(id, width, height)| RecordedBreakpoint {
                id: (*id).into(),
                width: *width,
                height: *height,
                device_scale_factor: 2.0,
            })
            .collect(),
        started_at_ms: 1,
        app_version: "0.5.0".into(),
        steps: Vec::new(),
    }
}

#[test]
fn each_format_names_the_plugins_files() {
    assert_eq!(plugin_formats(ExportFormat::Flow, true), ["flow"]);
    assert_eq!(
        plugin_formats(ExportFormat::Spec, true),
        ["spec", "playwright-config"]
    );
    assert_eq!(plugin_formats(ExportFormat::Spec, false), ["spec"]);
    assert_eq!(
        plugin_formats(ExportFormat::Both, true),
        ["flow", "spec", "playwright-config"]
    );
}

#[test]
fn the_commands_set_their_format_over_the_arguments() {
    assert_eq!(
        with_format(&Value::Null, ExportFormat::Flow),
        json!({ "format": "flow" })
    );
    assert_eq!(
        with_format(
            &json!({ "recordingId": "login", "format": "flow" }),
            ExportFormat::Spec
        ),
        json!({ "recordingId": "login", "format": "spec" })
    );
}

#[test]
fn the_arguments_default_to_both_and_refuse_unknown_fields() {
    let args: RecorderExportArgs =
        serde_json::from_value(json!({ "recordingId": "login" })).expect("parses");
    assert_eq!(args.format, ExportFormat::Both);
    assert!(
        serde_json::from_value::<RecorderExportArgs>(json!({ "path": "/etc/passwd" })).is_err()
    );
    assert!(serde_json::from_value::<RecorderExportArgs>(json!({ "format": "cypress" })).is_err());
}

#[test]
fn each_recorded_breakpoint_is_a_project_with_the_configs_emulation() {
    let mut config = default_config();
    let mobile = config
        .breakpoints
        .iter_mut()
        .find(|bp| bp.id == "mobile")
        .expect("mobile");
    mobile.emulation.locale = Some("de-DE".into());
    mobile.emulation.color_scheme = Some(ColorScheme::Dark);
    mobile.emulation.dpr = Some(3.0);
    let projects = projects_of(
        &recording(&[("mobile", 393, 852), ("500x800", 500, 800)]),
        &config,
    );
    assert_eq!(projects.len(), 2);
    let mobile = &projects[0];
    assert_eq!(mobile.name, "mobile");
    assert_eq!(mobile.engine, "chromium");
    assert_eq!((mobile.width, mobile.height), (393, 852));
    assert_eq!(mobile.device_scale_factor, Some(3.0));
    assert_eq!(mobile.has_touch, Some(true));
    assert_eq!(mobile.is_mobile, Some(true));
    assert_eq!(mobile.locale.as_deref(), Some("de-DE"));
    assert_eq!(mobile.color_scheme.as_deref(), Some("dark"));
    let free = &projects[1];
    assert_eq!(free.name, "500x800");
    assert_eq!((free.width, free.height), (500, 800));
    assert_eq!(free.device_scale_factor, Some(2.0));
    assert_eq!(free.is_mobile, None);
}

#[test]
fn a_recording_without_breakpoints_gets_every_configured_one() {
    let names: Vec<String> = projects_of(&recording(&[]), &default_config())
        .into_iter()
        .map(|project| project.name)
        .collect();
    assert_eq!(names, ["desktop", "tablet", "mobile"]);
}

#[test]
fn the_request_carries_the_preference_and_the_environments_origin() {
    let mut config = default_config();
    config.environments.push(Environment {
        id: "local".into(),
        url: "http://127.0.0.1:8000".into(),
        color: None,
        reload_interval_sec: None,
    });
    config.automation.recorder_export.locator_preference = LocatorPreference::Role;
    let request: Value = serde_json::from_str(
        &request_json(&recording(&[("mobile", 393, 852)]), &config).expect("json"),
    )
    .expect("parses");
    assert_eq!(request["recording"]["name"], "login");
    assert_eq!(request["options"]["locatorPreference"], "role");
    assert_eq!(request["options"]["baseUrl"], "http://127.0.0.1:8000");
    assert_eq!(request["options"]["projects"][0]["name"], "mobile");
    config.environments.clear();
    assert_eq!(base_url_of(&recording(&[]), &config), None);
}

#[test]
fn the_description_promises_no_clear_secrets_and_no_overwrite() {
    assert!(RECORDER_EXPORT_DESCRIPTION.contains("RB_SECRET_"));
    assert!(RECORDER_EXPORT_DESCRIPTION.contains("nothing is overwritten"));
    assert_eq!(RECORDER_EXPORT_TOOL, "recorder_export");
}

/// A window outside the runtime, as on the CEF UI thread, with a project at a temporary root.
fn window(runtime: &tokio::runtime::Runtime) -> (Setup, tempfile::TempDir) {
    let setup = {
        let _inside = runtime.enter();
        setup()
    };
    let root = tempfile::tempdir().expect("temp dir");
    std::fs::create_dir_all(root.path().join(".rbrowser")).expect("config dir");
    setup
        .core
        .host()
        .open_config(default_config(), Some(root.path().to_path_buf()))
        .expect("opens");
    (setup, root)
}

fn run(setup: &Setup, id: &str, args: Value) -> Result<Value, CommandError> {
    run_command(&setup.core, id, &args).expect("a recorder command")
}

#[test]
fn without_a_recording_the_export_asks_for_one() {
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let (setup, _root) = window(&runtime);
    let error = run(&setup, EXPORT_SPEC, Value::Null).expect_err("nothing recorded");
    assert!(matches!(error, CommandError::InvalidArgs(_)), "{error}");
    let error = run(&setup, EXPORT_FLOW, json!({ "recordingId": "../x" })).expect_err("bad id");
    assert!(matches!(error, CommandError::InvalidArgs(_)), "{error}");
    let error = run(&setup, EXPORT_FLOW, json!({ "target": 1 })).expect_err("a view");
    assert!(matches!(error, CommandError::InvalidArgs(_)), "{error}");
}

#[test]
fn without_the_plugin_nothing_is_written() {
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let (setup, root) = window(&runtime);
    let store = RecordingStore::open(root.path(), ".rbrowser/out/recordings", &[]).expect("store");
    store
        .save(&recording(&[("mobile", 393, 852)]))
        .expect("saved");
    // The test binary has no bundled plugins beside it.
    let error = run(&setup, EXPORT_SPEC, Value::Null).expect_err("no plugin");
    assert!(
        error.to_string().contains("plugins.error.not_available"),
        "{error}"
    );
    assert!(!root.path().join(".rbrowser/out/exports").exists());
}
