use std::fs;

use rb_ipc::{RecordingEntry, RecordingExportFormat, ShellToCore};
use rb_sync::recorder::{DEFAULT_DIR, RecordingStore};
use serde_json::json;

use super::*;

const PROTECTED: &[&str] = &["flows", "snippets", "plugins", "mocks", "scripts"];

fn recording(name: &str) -> Value {
    json!({
        "version": 1,
        "name": name,
        "startedAtMs": 1,
        "appVersion": "0.9.0",
        "steps": [
            { "id": 1, "action": { "type": "navigate", "url": "http://x.test/" }, "view": "desktop", "atMs": 0 },
            { "id": 2, "name": "Open the menu", "action": { "type": "click",
              "target": { "selectors": [{ "kind": "css", "path": "#menu" }] } },
              "view": "mobile", "atMs": 400, "onlyIn": ["mobile"] }
        ]
    })
}

#[test]
fn an_edited_recording_is_read_with_the_model() {
    let read = edited("login", &recording("login")).expect("valid");
    assert_eq!(read.steps.len(), 2);
    assert_eq!(read.steps[1].name.as_deref(), Some("Open the menu"));
    assert_eq!(read.steps[1].only_in, vec!["mobile".to_owned()]);
}

#[test]
fn an_edited_recording_keeps_its_file_name_unique_ids_and_the_schema() {
    assert!(
        edited("other", &recording("login"))
            .expect_err("renamed")
            .contains("login")
    );
    let mut twice = recording("login");
    twice["steps"][1]["id"] = json!(1);
    assert_eq!(
        edited("login", &twice).expect_err("same id"),
        "two steps have the id 1"
    );
    // RB-092-01: an editor's pause or wait above 10 s is refused.
    let mut pause = recording("login");
    pause["steps"][0]["pauseMs"] = json!(10_001);
    assert!(
        edited("login", &pause)
            .expect_err("too long")
            .contains("10000")
    );
    let mut unknown = recording("login");
    unknown["steps"][0]["colour"] = json!("red");
    assert!(edited("login", &unknown).is_err(), "unknown field");
    let mut version = recording("login");
    version["version"] = json!(2);
    assert!(edited("login", &version).is_err(), "another version");
    assert!(edited("../x", &recording("../x")).is_err(), "a bad name");
}

#[test]
fn the_recordings_are_listed_newest_first_with_their_steps() {
    let dir = tempfile::tempdir().expect("temp dir");
    fs::create_dir_all(dir.path().join(".rbrowser")).expect("config dir");
    let store = RecordingStore::open(dir.path(), DEFAULT_DIR, PROTECTED).expect("store");
    for name in ["alpha", "beta"] {
        let recording = edited(name, &recording(name)).expect("valid");
        store.save(&recording).expect("saved");
    }
    let listed = entries(&store).expect("listed");
    let names: Vec<&str> = listed.iter().map(|entry| entry.name.as_str()).collect();
    assert_eq!(names.len(), 2);
    assert!(names.contains(&"alpha") && names.contains(&"beta"));
    assert!(listed.iter().all(|entry| entry.steps == 2));
    assert!(
        listed
            .windows(2)
            .all(|pair| pair[0].modified_ms >= pair[1].modified_ms)
    );
}

#[test]
fn newest_first_orders_by_time_then_name() {
    let entry = |name: &str, modified_ms| RecordingEntry {
        name: name.to_owned(),
        steps: 0,
        modified_ms,
    };
    let mut entries = vec![
        entry("b", 5),
        entry("old", 1),
        entry("a", 5),
        entry("new", 9),
    ];
    newest_first(&mut entries);
    let names: Vec<&str> = entries.iter().map(|entry| entry.name.as_str()).collect();
    assert_eq!(names, ["new", "a", "b", "old"]);
}

#[test]
fn the_panels_requests_run_off_the_ui_thread() {
    assert!(is_off_thread(&ShellToCore::SaveRecording {
        name: "login".to_owned(),
        recording: recording("login"),
    }));
    assert!(is_off_thread(&ShellToCore::ExportRecording {
        name: "login".to_owned(),
        format: RecordingExportFormat::Flow,
        export_name: None,
        write: false,
        overwrite: false,
    }));
    assert!(is_off_thread(&ShellToCore::GetRecordings));
    assert!(is_off_thread(&ShellToCore::GetFlowRun));
    // It waits for the recorder, which a stop holds while it writes the file (RB-090-89).
    assert!(is_off_thread(&ShellToCore::ResendRecordingSteps));
    assert!(!is_off_thread(&ShellToCore::ListFlows));
}

#[test]
fn the_panel_formats_are_the_exports() {
    assert_eq!(format_of(RecordingExportFormat::Flow), ExportFormat::Flow);
    assert_eq!(format_of(RecordingExportFormat::Spec), ExportFormat::Spec);
    assert_eq!(format_of(RecordingExportFormat::Both), ExportFormat::Both);
}
