//! The snapshot test of RB-050-01: a recording on the fixture site (`tests/fixtures/site/`,
//! `form.html` and `mobile-menu.html`) - log in with name and password, send, open the mobile
//! menu, follow a link - fed as the agent's messages (`docs/sync-protocol.md`), against the
//! expected recording `tests/fixtures/recording-fixture.json`. Rewrite that file with
//! `RB_UPDATE_SNAPSHOT=1 cargo nextest run -p rb-sync --test recording_fixture` and review it.

use std::fs;
use std::path::Path;

use rb_engine::ViewId;
use rb_sync::EventMessage;
use rb_sync::ReplayOutcome;
use rb_sync::recorder::{
    RecordedBreakpoint, Recorder, RecorderOptions, RecordingView, StartInfo, from_json, to_json,
};
use serde_json::{Value, json};

const DESKTOP: ViewId = ViewId(1);
const MOBILE: ViewId = ViewId(2);
const START: u64 = 1_762_800_000_000;
const SITE: &str = "http://127.0.0.1:8000";
const PASSWORD: &str = "s3cret!Pw";

fn message(seq: u64, event: Value) -> EventMessage {
    serde_json::from_value(json!({ "v": 1, "top": true, "seq": seq, "time": 0, "event": event }))
        .expect("an agent message")
}

fn target(element: Value) -> Value {
    json!({ "element": element, "offset": { "x": 0.5, "y": 0.5 } })
}

fn click(element: Value) -> Value {
    json!({ "type": "click", "button": "left", "detail": 1, "modifiers": 0,
            "target": target(element) })
}

fn key(name: &str, element: &Value) -> Value {
    json!({ "type": "keydown", "key": name, "code": "", "keyCode": 0, "location": 0,
            "repeat": false, "modifiers": 0, "target": target(element.clone()) })
}

fn input(element: &Value, value: &str, sensitive: bool) -> Value {
    let mut event = json!({ "type": "input", "inputType": "insertText",
                            "data": value.chars().last().map(String::from), "value": value,
                            "target": target(element.clone()) });
    if sensitive {
        event["sensitive"] = json!(true);
    }
    event
}

fn views() -> Vec<RecordingView> {
    [
        (DESKTOP, "desktop", 1280, 800, 1.0),
        (MOBILE, "mobile", 390, 844, 3.0),
    ]
    .into_iter()
    .map(|(view, id, width, height, dpr)| RecordingView {
        view,
        breakpoint: RecordedBreakpoint {
            id: id.into(),
            width,
            height,
            device_scale_factor: dpr,
        },
    })
    .collect()
}

/// Types `text` into `field` in the desktop view, one key and one input per character.
fn type_into(
    recorder: &mut Recorder,
    seq: &mut u64,
    ms: &mut u64,
    field: &Value,
    text: &str,
    sensitive: bool,
) {
    for (i, c) in text.char_indices() {
        let typed = &text[..i + c.len_utf8()];
        for event in [key(&c.to_string(), field), input(field, typed, sensitive)] {
            *seq += 1;
            *ms += 30;
            recorder.on_event(DESKTOP, &message(*seq, event), &[MOBILE], START + *ms);
        }
    }
}

fn record() -> String {
    let mut recorder = Recorder::new(RecorderOptions::default());
    recorder
        .start(StartInfo {
            name: "fixture-login".into(),
            start_url: Some(format!("{SITE}/form.html")),
            environment: Some("local".into()),
            app_version: "0.1.0".into(),
            views: views(),
            leader: Some(DESKTOP),
            now_ms: START,
        })
        .expect("starts");
    let (mut seq, mut ms) = (0, 0);
    let name = json!({ "kind": "role", "role": "textbox", "name": "Name", "path": "#name" });
    let password = json!({ "kind": "css", "path": "#password" });
    type_into(&mut recorder, &mut seq, &mut ms, &name, "Ada", false);
    type_into(&mut recorder, &mut seq, &mut ms, &password, PASSWORD, true);
    let submit = click(json!({ "kind": "testid", "testid": "submit", "path": "#order > button" }));
    recorder.on_event(DESKTOP, &message(100, submit), &[MOBILE], START + 2_000);
    recorder.on_navigated(DESKTOP, &format!("{SITE}/form.html?posts=1"), START + 2_300);
    recorder.on_navigated(MOBILE, &format!("{SITE}/form.html?posts=1"), START + 2_400);
    // The mobile view leads: the menu page typed into the address bar.
    recorder.set_leader(Some(MOBILE));
    recorder.on_navigated(MOBILE, &format!("{SITE}/mobile-menu.html"), START + 10_000);
    let burger = message(
        1,
        click(json!({ "kind": "role", "role": "button", "name": "Menu", "path": "#burger" })),
    );
    recorder.on_event(MOBILE, &burger, &[DESKTOP], START + 11_000);
    recorder.on_replayed(DESKTOP, &burger.event, &ReplayOutcome::NotVisible);
    let about = message(
        2,
        click(json!({ "kind": "testid", "testid": "nav-about", "path": "#mobile-menu > a" })),
    );
    recorder.on_event(MOBILE, &about, &[DESKTOP], START + 12_000);
    recorder.on_replayed(DESKTOP, &about.event, &ReplayOutcome::Done);
    let recording = recorder.stop().expect("stops");
    to_json(&recording).expect("serialises")
}

#[test]
fn the_fixture_recording_matches_the_snapshot() {
    let text = record();
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/recording-fixture.json");
    if std::env::var_os("RB_UPDATE_SNAPSHOT").is_some_and(|value| value == "1") {
        fs::write(&path, &text).expect("write the snapshot");
    }
    let expected = fs::read_to_string(&path).expect("the snapshot is part of the repository");
    assert_eq!(text, expected);
    // It reads back as a recording of this version.
    assert_eq!(
        to_json(&from_json(&text).expect("reads")).expect("writes"),
        text
    );
}

#[test]
fn the_fixture_recording_holds_no_password() {
    let text = record();
    assert!(!text.contains(PASSWORD));
    assert!(!text.contains("s3cret"));
    assert!(text.contains("{{secret:password}}"));
}
