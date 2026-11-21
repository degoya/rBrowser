use std::sync::Arc;

use rb_engine::ViewId;
use serde_json::Value;
use tokio::runtime::Handle;

use super::*;
use crate::cdp::fake::FakeBrowser;
use crate::control::CoreSetup;
use crate::host::state::default_config;
use crate::host::ui::recording::{RecordingUi, UiCall};

fn host() -> (Arc<RecordingUi>, Arc<Core>) {
    let (_fake, router) = FakeBrowser::connect();
    let ui = Arc::new(RecordingUi::default());
    let host = Host::new(ui.clone(), router, Handle::current());
    host.open_config(default_config(), None).expect("opens");
    let core = Core::new(host, ui.clone(), Handle::current(), CoreSetup::default());
    (ui, core)
}

fn answer(core: &Core, request: &str) -> Value {
    serde_json::from_str(&dispatch(core, request).expect("answered")).expect("json")
}

#[tokio::test]
async fn get_state_lists_the_views_in_grid_order() {
    let (_ui, host) = host();
    let state = answer(&host, r#"{"type":"getState"}"#);
    assert_eq!(state["type"], "state");
    let views = state["views"].as_array().expect("views");
    assert_eq!(views.len(), 3);
    assert_eq!(views[0]["breakpoint"], "desktop");
    assert_eq!(state["layout"]["mode"], "grid");
    assert!(state["project"].is_null());
}

#[tokio::test]
async fn toolbar_requests_reach_every_view() {
    let (ui, host) = host();
    assert_eq!(
        answer(&host, r#"{"type":"reload","ignoreCache":true}"#)["type"],
        "ack"
    );
    assert_eq!(answer(&host, r#"{"type":"back"}"#)["type"], "ack");
    let calls = ui.calls();
    for view in 1..=3 {
        assert!(calls.contains(&UiCall::Reload(ViewId(view), true)));
        assert!(calls.contains(&UiCall::Back(ViewId(view))));
    }
    assert_eq!(
        answer(&host, r#"{"type":"setActiveView","view":2}"#)["type"],
        "ack"
    );
    assert_eq!(host.host().active_view(), Some(ViewId(2)));
}

#[tokio::test]
async fn failures_carry_their_code() {
    let (_ui, host) = host();
    let code = |request: &str| dispatch(&host, request).expect_err("fails").0;
    assert_eq!(code("not json"), IpcErrorCode::BadRequest);
    assert_eq!(code(r#"{"type":"teleport"}"#), IpcErrorCode::BadRequest);
    assert_eq!(
        code(r#"{"type":"setActiveView","view":99}"#),
        IpcErrorCode::NotFound
    );
    assert_eq!(
        code(r#"{"type":"navigate","url":"javascript:alert(1)"}"#),
        IpcErrorCode::BadRequest
    );
    assert_eq!(
        code(r#"{"type":"runCommand","id":"view.teleport"}"#),
        IpcErrorCode::NotFound
    );
    assert_eq!(
        code(r#"{"type":"setSync","feature":"all","enabled":false}"#),
        IpcErrorCode::NotImplemented
    );
}

#[tokio::test]
async fn the_state_event_is_an_rb_ipc_custom_event() {
    let (_ui, host) = host();
    let script = state_event(host.host()).expect("script");
    assert!(script.starts_with("window.dispatchEvent(new CustomEvent(\"rb-ipc\""));
}

#[tokio::test]
async fn view_events_push_one_state_per_burst() {
    let (_ui, host) = host();
    let sent = Arc::new(std::sync::Mutex::new(Vec::new()));
    let sink = sent.clone();
    spawn_state_forwarder(host.host(), &Handle::current(), move |script| {
        crate::cdp::lock(&sink).push(script);
    });
    host.host().set_active_view(ViewId(1)).expect("active");
    host.host().set_active_view(ViewId(2)).expect("active");
    for _ in 0..100 {
        if !crate::cdp::lock(&sent).is_empty() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    tokio::time::sleep(std::time::Duration::from_millis(60)).await;
    let sent = crate::cdp::lock(&sent);
    assert_eq!(sent.len(), 1, "one push for the burst");
    assert!(sent[0].contains("\"activeView\":2"), "{}", sent[0]);
}

#[tokio::test]
async fn the_command_list_carries_the_editor_fields() {
    let (_ui, core) = host();
    let list = answer(&core, r#"{"type":"listCommands"}"#);
    assert_eq!(list["type"], "commands");
    let back = list["commands"]
        .as_array()
        .expect("commands")
        .iter()
        .find(|info| info["id"] == "view.back")
        .expect("view.back")
        .clone();
    assert_eq!(
        back["defaultBindings"],
        serde_json::json!(["Ctrl+Shift+Space V B", "Ctrl+Shift+Space V Left"])
    );
    assert_eq!(back["mcp"], serde_json::json!({ "covered": "back" }));
    let check = answer(
        &core,
        r#"{"type":"checkBinding","id":"view.back","binding":"Ctrl+Shift+Space V F"}"#,
    );
    assert_eq!(check["type"], "bindingCheck");
    assert_eq!(check["conflict"], "view.forward");
    assert_eq!(
        answer(&core, r#"{"type":"exportBindings"}"#)["type"],
        "bindingsExport"
    );
}

#[tokio::test]
async fn a_query_is_answered_through_the_reply() {
    let (_ui, core) = host();
    let (tx, rx) = std::sync::mpsc::channel();
    let reply = move |answer: Result<String, Failure>| {
        let _ = tx.send(answer);
    };
    dispatch_to(&core, r#"{"type":"getState"}"#, reply.clone());
    let state = rx.recv_timeout(Duration::from_secs(5)).expect("answered");
    assert!(state.expect("state").contains("\"type\":\"state\""));
    dispatch_to(
        &core,
        r#"{"type":"runCommand","id":"view.teleport"}"#,
        reply.clone(),
    );
    let failure = rx.recv_timeout(Duration::from_secs(5)).expect("answered");
    assert_eq!(failure.expect_err("unknown").0, IpcErrorCode::NotFound);
    dispatch_to(
        &core,
        r#"{"type":"runCommand","id":"settings.open"}"#,
        reply,
    );
    let ack = rx.recv_timeout(Duration::from_secs(5)).expect("answered");
    assert!(ack.expect("ack").contains("\"type\":\"ack\""));
}

#[tokio::test]
async fn the_shell_gets_the_user_settings_at_start() {
    let (_ui, core) = host();
    let settings = answer(&core, r#"{"type":"getSettings"}"#);
    assert_eq!(settings["type"], "settings");
    let values = settings["values"].as_array().expect("values");
    assert!(
        values
            .iter()
            .any(|value| value["key"] == "ui.theme" && value["value"] == "system")
    );
    assert!(settings.get("schema").is_none());
    // The settings window asks for the schema once (RB-010-08).
    let with_schema = answer(&core, r#"{"type":"getSettings","includeSchema":true}"#);
    assert!(with_schema["schema"]["defs"].is_array());
}

#[tokio::test]
async fn the_state_lists_a_preset_only_breakpoint_at_the_presets_size() {
    let (_ui, core) = host();
    let mut config = default_config();
    config.breakpoints[2].width = None;
    config.breakpoints[2].height = None;
    config.breakpoints[2].device = Some("iphone-15".to_owned());
    core.host().open_config(config, None).expect("opens");
    let state = answer(&core, r#"{"type":"getState"}"#);
    let mobile = &state["breakpoints"][2];
    assert_eq!(mobile["device"], "iphone-15");
    assert_eq!(
        (mobile["width"].clone(), mobile["height"].clone()),
        (393.into(), 852.into())
    );
}

#[tokio::test]
async fn dismissing_the_palette_hides_its_popup() {
    let (ui, core) = host();
    let ack = answer(&core, r#"{"type":"dismissPalette"}"#);
    assert_eq!(ack["type"], "ack");
    assert!(ui.calls().contains(&UiCall::HidePageWindow(
        crate::page_window::PageWindow::Palette
    )));
}

/// RB-090-36: the queries that read files - the settings (with and without the schema), the
/// palette's flows - are answered off the calling thread, which is CEF's UI thread in the app.
#[tokio::test]
async fn file_reads_are_answered_off_the_calling_thread() {
    let (_ui, core) = host();
    let caller = std::thread::current().id();
    for request in [
        r#"{"type":"getSettings"}"#,
        r#"{"type":"getSettings","includeSchema":true}"#,
        r#"{"type":"listFlows"}"#,
    ] {
        let (tx, rx) = std::sync::mpsc::channel();
        dispatch_to(&core, request, move |answer: Result<String, Failure>| {
            let _ = tx.send((std::thread::current().id(), answer));
        });
        let (thread, answer) = rx.recv_timeout(Duration::from_secs(5)).expect("answered");
        assert!(answer.is_ok(), "{request}: {answer:?}");
        assert_ne!(
            thread, caller,
            "{request} was answered on the calling thread"
        );
    }
    // The state reads no file: answered at once.
    let (tx, rx) = std::sync::mpsc::channel();
    dispatch_to(&core, r#"{"type":"getState"}"#, move |_| {
        let _ = tx.send(std::thread::current().id());
    });
    assert_eq!(rx.try_recv().ok(), Some(caller));
}

/// RB-090-32: every query is counted from its arrival to its answer (`ipc.query`), also one
/// answered off the calling thread.
#[tokio::test]
async fn a_query_is_counted_until_its_answer() {
    use rb_util::perf::{global, names};
    let count = || {
        global()
            .snapshot()
            .iter()
            .find(|series| series.name == names::IPC_QUERY)
            .map_or(0, |series| series.count)
    };
    let (_ui, core) = host();
    let before = count();
    let (tx, rx) = std::sync::mpsc::channel();
    let reply = move |answer: Result<String, Failure>| {
        let _ = tx.send(answer);
    };
    dispatch_to(&core, r#"{"type":"getState"}"#, reply.clone());
    rx.recv_timeout(Duration::from_secs(5))
        .expect("answered")
        .expect("state");
    dispatch_to(&core, r#"{"type":"getSettings"}"#, reply);
    rx.recv_timeout(Duration::from_secs(5))
        .expect("answered")
        .expect("settings");
    assert!(count() >= before + 2, "both queries counted");
}
