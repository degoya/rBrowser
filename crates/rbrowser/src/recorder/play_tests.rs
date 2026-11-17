use serde_json::json;

use super::*;

fn recording(breakpoints: Value) -> Recording {
    let text = json!({
        "version": 1,
        "name": "checkout",
        "breakpoints": breakpoints,
        "startedAtMs": 1,
        "appVersion": "0.9.0",
        "steps": [
            { "id": 1, "action": { "type": "navigate", "url": "http://x.test/" }, "view": "desktop", "atMs": 0 }
        ]
    })
    .to_string();
    rb_sync::recorder::from_json(&text).expect("a recording")
}

fn files() -> Vec<(String, String)> {
    vec![(
        "checkout.ts".to_owned(),
        "await page.goto('http://x.test/');\n".to_owned(),
    )]
}

#[test]
fn a_recording_plays_as_the_plugins_flow_in_its_breakpoints() {
    let recording = recording(json!([
        { "id": "mobile", "width": 375, "height": 812, "deviceScaleFactor": 3.0 },
        { "id": "desktop", "width": 1440, "height": 900, "deviceScaleFactor": 1.0 }
    ]));
    let flow = inline_flow(
        &recording,
        files(),
        ".rbrowser/out/recordings/checkout.json".to_owned(),
    )
    .expect("a flow");
    assert_eq!(flow.source, "await page.goto('http://x.test/');\n");
    assert_eq!(flow.file, ".rbrowser/out/recordings/checkout.json");
    assert_eq!(flow.views, ["mobile", "desktop"]);
}

#[test]
fn a_recording_without_breakpoints_plays_in_every_view() {
    let flow = inline_flow(&recording(json!([])), files(), "x.json".to_owned()).expect("a flow");
    assert!(flow.views.is_empty());
}

#[test]
fn no_flow_from_the_plugin_is_a_failure_not_an_empty_run() {
    let only_spec = vec![("checkout.spec.ts".to_owned(), String::new())];
    assert!(matches!(
        inline_flow(&recording(json!([])), only_spec, "x.json".to_owned()),
        Err(CommandError::Failed(_))
    ));
    assert!(matches!(
        inline_flow(&recording(json!([])), Vec::new(), "x.json".to_owned()),
        Err(CommandError::Failed(_))
    ));
}

#[test]
fn the_arguments_name_a_recording_views_and_whether_to_wait() {
    assert_eq!(args(&Value::Null).expect("none"), PlayArgs::default());
    let read = args(&json!({ "recordingId": "checkout", "views": [1, "mobile"],
                              "pace": "recorded", "wait": false }))
    .expect("all");
    assert_eq!(read.pace, Some(PlaybackPace::Recorded));
    assert_eq!(read.recording_id.as_deref(), Some("checkout"));
    assert_eq!(read.views, Some(json!([1, "mobile"])));
    assert_eq!(read.wait, Some(false));
    assert!(matches!(
        args(&json!({ "recording": "checkout" })),
        Err(CommandError::InvalidArgs(_))
    ));
    assert!(matches!(
        args(&json!({ "pace": "slow" })),
        Err(CommandError::InvalidArgs(_))
    ));
}

#[test]
fn the_command_and_the_tool_keep_their_names() {
    assert_eq!(PLAY, "recorder.play");
    assert_eq!(RECORDER_PLAY_TOOL, "recorder_play");
}

#[test]
fn a_flow_refusal_keeps_its_code_for_mcp() {
    let failure = flow_failure(&FlowError::Untrusted);
    assert_eq!(failure.code, "flows.untrusted");
    let failure = flow_failure(&FlowError::Busy("login".to_owned()));
    assert_eq!(failure.code, "flows.busy");
}

#[test]
fn a_play_that_did_not_start_names_its_recording_or_the_last() {
    let CoreToShell::Notification(named) = failed_notice(Some("checkout"), "no such recording")
    else {
        panic!("a notification");
    };
    assert_eq!(named.message_key, FAILED_NOTICE);
    assert_eq!(named.args.get("name").map(String::as_str), Some("checkout"));
    let CoreToShell::Notification(last) = failed_notice(None, "no recording yet") else {
        panic!("a notification");
    };
    assert_eq!(last.message_key, FAILED_LAST_NOTICE);
    assert!(!last.args.contains_key("name"));
    assert_eq!(
        last.args.get("error").map(String::as_str),
        Some("no recording yet")
    );
}
