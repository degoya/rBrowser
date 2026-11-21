use std::sync::Arc;

use rb_engine::{ViewHost, ViewId};
use rb_ipc::RecorderPhase;
use rb_sync::{RecordingReplay, ReplayOutcome};
use serde_json::{Value, json};

use super::*;
use crate::host::state::default_config;
use crate::sync_hub::tests::{Setup, attach_all, emit_from, setup, until};

const PASSWORD: &str = "hunter2-Secret";

fn event(seq: u64, event: Value) -> Value {
    json!({ "kind": "event", "v": 1, "top": true, "seq": seq, "time": 0, "event": event })
}

fn at(path: &str) -> Value {
    json!({ "element": { "kind": "css", "path": path }, "offset": { "x": 0.5, "y": 0.5 } })
}

fn click(seq: u64, path: &str) -> Value {
    event(
        seq,
        json!({ "type": "click", "button": "left", "detail": 1, "modifiers": 0, "target": at(path) }),
    )
}

fn typed(seq: u64, path: &str, value: &str, sensitive: bool) -> Value {
    event(
        seq,
        json!({ "type": "input", "inputType": "insertText", "value": value,
                "sensitive": sensitive, "target": at(path) }),
    )
}

/// A window with a project at a temporary root, the SyncHub running, view 1 active and view 3
/// missing every element.
async fn project() -> (Setup, tempfile::TempDir, Arc<RecordingReplay>) {
    let setup = setup();
    let root = tempfile::tempdir().expect("temp dir");
    std::fs::create_dir_all(root.path().join(".rbrowser")).expect("config dir");
    let host = setup.core.host();
    host.open_config(default_config(), Some(root.path().to_path_buf()))
        .expect("opens");
    crate::sync_hub::on_start(&setup.core);
    let sync = crate::sync_hub::for_host(host).expect("sync runs");
    let replay = Arc::new(RecordingReplay::default());
    replay.answer(ViewId(3), ReplayOutcome::NotFound);
    sync.set_replay(replay.clone());
    attach_all(&setup).await;
    host.set_active_view(ViewId(1)).expect("a view");
    until(|| sync.hub().with(|hub| hub.active()) == Some(ViewId(1))).await;
    (setup, root, replay)
}

fn run(setup: &Setup, id: &str, args: Value) -> Result<Value, CommandError> {
    run_command(&setup.core, id, &args).expect("a recorder command")
}

#[test]
fn other_commands_are_not_the_recorder_s() {
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let _inside = runtime.enter();
    let setup = setup();
    assert!(run_command(&setup.core, "sync.toggle", &Value::Null).is_none());
}

/// The runtime rule: a command can come from the CEF UI thread, outside the runtime.
#[test]
fn a_command_outside_the_runtime_makes_the_recorder_on_the_core_s_runtime() {
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let setup = {
        let _inside = runtime.enter();
        setup()
    };
    // No project root: the start fails, but the recorder was made without a panic.
    let error = run(&setup, START, Value::Null).expect_err("no project");
    assert!(error.to_string().contains("no project is open"), "{error}");
    assert!(for_host(setup.core.host()).is_some());
    let error = run(&setup, STOP, Value::Null).expect_err("not recording");
    assert!(error.to_string().contains("no recording"), "{error}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_recording_is_written_with_its_steps_and_without_the_password() {
    let (setup, root, _replay) = project().await;
    let started = run(&setup, START, json!({ "name": "login" })).expect("starts");
    let recorder = for_host(setup.core.host()).expect("made");
    // A `navigate` to the start URL when the view has one.
    let base = recorder.outcome().steps;
    assert_eq!(started["state"], "recording");
    let indicator = setup.core.host().shell_state().recorder.expect("shown");
    assert_eq!(indicator.phase, RecorderPhase::Recording);
    for (seq, value) in ["h", "hunter", PASSWORD].iter().enumerate() {
        emit_from(&setup, 1, &typed(seq as u64 + 1, "#password", value, true));
    }
    emit_from(&setup, 1, &typed(10, "#name", "Ada", false));
    emit_from(&setup, 1, &click(11, "#burger"));
    until(|| recorder.outcome().steps == base + 3).await;
    // View 3's replay of the click fails: it drops out of the click's `onlyIn`.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let stopped = run(&setup, STOP, Value::Null).expect("stops");
    assert_eq!(stopped["state"], "idle");
    assert_eq!(stopped["path"], ".rbrowser/out/recordings/login.json");
    assert!(setup.core.host().shell_state().recorder.is_none());
    let text = std::fs::read_to_string(root.path().join(".rbrowser/out/recordings/login.json"))
        .expect("written");
    assert!(
        !text.contains(PASSWORD) && !text.contains("hunter"),
        "{text}"
    );
    assert!(text.contains("{{secret:password}}"), "{text}");
    assert!(!stopped.to_string().contains("hunter"));
    let recording = rb_sync::recorder::from_json(&text).expect("a recording");
    let click = recording.steps.last().expect("steps");
    assert_eq!(click.only_in, ["desktop", "tablet"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn pause_suppresses_steps_and_a_second_stop_takes_a_new_name() {
    let (setup, root, _replay) = project().await;
    run(&setup, START, json!({ "name": "flow" })).expect("starts");
    let paused = run(&setup, PAUSE, Value::Null).expect("pauses");
    assert_eq!(paused["state"], "paused");
    assert_eq!(
        setup.core.host().shell_state().recorder.map(|r| r.phase),
        Some(RecorderPhase::Paused)
    );
    emit_from(&setup, 1, &click(1, "#ignored"));
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let resumed = run(&setup, PAUSE, Value::Null).expect("resumes");
    let recorder = for_host(setup.core.host()).expect("made");
    let base = recorder.outcome().steps;
    assert_eq!(resumed["state"], "recording");
    emit_from(&setup, 1, &click(2, "#kept"));
    until(|| recorder.outcome().steps == base + 1).await;
    run(&setup, STOP, Value::Null).expect("stops");
    let text = std::fs::read_to_string(root.path().join(".rbrowser/out/recordings/flow.json"))
        .expect("written");
    assert!(
        !text.contains("#ignored") && text.contains("#kept"),
        "{text}"
    );
    // The same name again is never overwritten.
    run(&setup, START, json!({ "name": "flow" })).expect("starts");
    let stopped = run(&setup, STOP, Value::Null).expect("stops");
    assert_eq!(stopped["name"], "flow-2");
    assert_eq!(
        std::fs::read_to_string(root.path().join(".rbrowser/out/recordings/flow.json"))
            .expect("kept"),
        text
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_bad_name_or_folder_is_refused_before_recording() {
    let (setup, root, _replay) = project().await;
    let error = run(&setup, START, json!({ "name": "../evil" })).expect_err("bad name");
    assert!(matches!(error, CommandError::InvalidArgs(_)), "{error}");
    let mut config = default_config();
    config.automation.recorder.dir = ".rbrowser/flows".into();
    setup
        .core
        .host()
        .open_config(config, Some(root.path().to_path_buf()))
        .expect("reopens");
    let error = run(&setup, START, Value::Null).expect_err("protected folder");
    assert!(error.to_string().contains("protected"), "{error}");
    assert!(setup.core.host().shell_state().recorder.is_none());
}

/// RB-090-91: `recorder.delete` checks the id, keeps the recording being recorded and deletes
/// a stored one off the calling thread.
#[tokio::test(flavor = "multi_thread")]
async fn a_stored_recording_is_deleted_but_never_the_one_being_recorded() {
    let (setup, _root, _replay) = project().await;
    let store = glue::store_of(setup.core.host()).expect("a store");
    let old = rb_sync::recorder::from_json(
        r#"{ "version": 1, "name": "old", "breakpoints": [], "startedAtMs": 1, "appVersion": "0.9.0", "steps": [] }"#,
    )
    .expect("a recording");
    store.save(&old).expect("saved");
    for bad in [
        json!({ "recordingId": "../old" }),
        json!({ "recordingId": "a/b" }),
        json!({ "recordingId": 3 }),
    ] {
        let error = run(&setup, DELETE, bad.clone()).expect_err("refused");
        assert!(
            matches!(error, CommandError::InvalidArgs(_)),
            "{bad}: {error}"
        );
    }
    run(&setup, START, json!({ "name": "login" })).expect("starts");
    let error = run(&setup, DELETE, json!({ "recordingId": "login" })).expect_err("being recorded");
    assert!(error.to_string().contains("being recorded"), "{error}");
    run(&setup, DELETE, json!({ "recordingId": "old" })).expect("asked");
    let file = store.path_of("old").expect("a path");
    until(|| !file.exists()).await;
}

/// RB-050-01 review: `target` switches the active view only once the checks passed.
#[tokio::test(flavor = "multi_thread")]
async fn a_refused_start_leaves_the_active_view_alone() {
    let (setup, _root, _replay) = project().await;
    let error =
        run(&setup, START, json!({ "name": "../evil", "target": 2 })).expect_err("bad name");
    assert!(matches!(error, CommandError::InvalidArgs(_)), "{error}");
    assert_eq!(setup.core.host().active_view(), Some(ViewId(1)));
    let error = run(&setup, START, json!({ "target": 99 })).expect_err("no such view");
    assert!(matches!(error, CommandError::InvalidArgs(_)), "{error}");
    assert!(setup.core.host().shell_state().recorder.is_none());
}

/// RB-050-01 review: with sync off the agent captures nothing - the start says so instead of
/// recording an empty flow.
#[tokio::test(flavor = "multi_thread")]
async fn a_start_with_sync_off_is_refused() {
    let (setup, _root, _replay) = project().await;
    let sync = crate::sync_hub::for_host(setup.core.host()).expect("sync runs");
    sync.hub().with(|hub| hub.set_enabled(false));
    let error = run(&setup, START, Value::Null).expect_err("sync is off");
    assert!(error.to_string().contains("sync is off"), "{error}");
    assert!(setup.core.host().shell_state().recorder.is_none());
    sync.hub().with(|hub| {
        hub.set_enabled(true);
        hub.set_kind(rb_sync::SyncKind::Keyboard, false);
    });
    let started = run(&setup, START, Value::Null).expect("starts");
    assert_eq!(started["notCaptured"], json!(["keyboard"]));
}

/// RB-050-01 review: a stop whose save fails keeps the recording; the next stop saves it.
#[tokio::test(flavor = "multi_thread")]
async fn a_failed_save_keeps_the_recording() {
    let (setup, root, _replay) = project().await;
    run(&setup, START, json!({ "name": "kept" })).expect("starts");
    let recorder = for_host(setup.core.host()).expect("made");
    let base = recorder.outcome().steps;
    emit_from(&setup, 1, &click(1, "#kept"));
    until(|| recorder.outcome().steps == base + 1).await;
    let folder = root.path().join(".rbrowser/out/recordings");
    std::fs::create_dir_all(folder.parent().expect("a parent")).expect("out dir");
    std::fs::write(&folder, "in the way").expect("a file where the folder goes");
    run(&setup, STOP, Value::Null).expect_err("cannot save");
    assert_eq!(
        recorder.outcome().state,
        rb_sync::recorder::RecorderState::Recording
    );
    assert_eq!(recorder.outcome().steps, base + 1);
    std::fs::remove_file(&folder).expect("cleared");
    let stopped = run(&setup, STOP, Value::Null).expect("stops");
    assert_eq!(stopped["path"], ".rbrowser/out/recordings/kept.json");
    let text = std::fs::read_to_string(folder.join("kept.json")).expect("written");
    assert!(text.contains("#kept"), "{text}");
}

/// RB-090-87: only the recorded view's agent gets the recording's wishes, and only while the
/// recording takes steps.
#[tokio::test(flavor = "multi_thread")]
async fn the_recorded_views_agent_has_the_wishes_while_steps_are_taken() {
    let (setup, _root, _replay) = project().await;
    let host = setup.core.host();
    let sync = crate::sync_hub::for_host(host).expect("sync runs");
    let config = host.config().sync;
    let wishes = |view: u32| sync.agent_config(&config, ViewId(view)).record;
    assert_eq!(agent_record::agent_record(host), None);
    run(&setup, START, json!({ "name": "menus" })).expect("starts");
    let expected = agent_record::wishes(&host.config().automation.recorder);
    assert_eq!(agent_record::agent_record(host), Some(expected));
    assert_eq!(wishes(1), Some(expected));
    assert_eq!(wishes(2), None, "a view that does not capture");
    run(&setup, PAUSE, Value::Null).expect("pauses");
    assert_eq!(wishes(1), None);
    run(&setup, PAUSE, Value::Null).expect("resumes");
    assert_eq!(wishes(1), Some(expected));
    run(&setup, STOP, Value::Null).expect("stops");
    assert_eq!(wishes(1), None);
}
