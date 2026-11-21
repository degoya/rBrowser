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

#[test]
fn a_flow_played_as_recorded_carries_the_exporters_pauses() {
    // RB-090-84: the pauses come from the exporter (`export::recorded_pauses`), the one rule;
    // the run adds them to its timeout.
    let recording = recording(json!([]));
    let file = || "x.json".to_owned();
    let fast = played_flow(&recording, files(), file(), 0).expect("a flow");
    assert_eq!(fast.pause_ms, 0);
    let recorded = played_flow(&recording, files(), file(), 31_000).expect("a flow");
    assert_eq!(recorded.pause_ms, 31_000);
    assert_eq!(recorded.source, fast.source);
}

fn three_steps() -> Recording {
    let text = json!({
        "version": 1, "name": "checkout", "startedAtMs": 1, "appVersion": "0.9.2",
        "steps": [
            { "id": 1, "action": { "type": "navigate", "url": "http://x.test/" }, "view": "desktop", "atMs": 0 },
            { "id": 2, "action": { "type": "wait", "ms": 1500 }, "view": "desktop", "atMs": 10, "disabled": true },
            { "id": 3, "action": { "type": "wait", "ms": 40000 }, "view": "desktop", "atMs": 20 }
        ]
    })
    .to_string();
    rb_sync::recorder::from_json(&text).expect("a recording")
}

fn ids(recording: &Recording) -> Vec<u32> {
    recording.steps.iter().map(|step| step.id).collect()
}

/// RB-092-01: "Play" leaves out the steps switched off, "Play from here" starts at a step,
/// "Try" plays one step alone, and the steps before "Continue recording from here" end at it.
#[test]
fn a_part_of_the_recording_plays() {
    let all = three_steps();
    assert_eq!(ids(&select(&all, &Part::default()).expect("all")), [1, 3]);
    let from = Part {
        from_step: Some(2),
        ..Part::default()
    };
    assert_eq!(ids(&select(&all, &from).expect("from 2")), [3]);
    let only = Part {
        only_step: Some(2),
        ..Part::default()
    };
    assert_eq!(ids(&select(&all, &only).expect("only 2")), [2]);
    let up_to = Part {
        up_to: Some(2),
        ..Part::default()
    };
    assert_eq!(ids(&select(&all, &up_to).expect("up to 2")), [1]);
    let missing = Part {
        from_step: Some(9),
        ..Part::default()
    };
    assert!(matches!(
        select(&all, &missing),
        Err(CommandError::InvalidArgs(_))
    ));
}

#[test]
fn a_fixed_wait_adds_to_the_runs_budget_capped_like_the_export() {
    // Step 2 is off and plays not; step 3 waits at most 10 s.
    assert_eq!(
        waits_ms(&select(&three_steps(), &Part::default()).expect("all")),
        10_000
    );
}

#[test]
fn the_arguments_name_a_part_and_a_draft() {
    let read = args(
        &json!({ "recordingId": "checkout", "fromStep": 3, "onlyStep": 2,
                              "draft": { "version": 1 } }),
    )
    .expect("a part");
    assert_eq!(read.from_step, Some(3));
    assert_eq!(read.only_step, Some(2));
    assert_eq!(read.draft, Some(json!({ "version": 1 })));
}
