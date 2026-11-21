use serde_json::json;

use super::*;
use crate::recorder::RecordedBreakpoint;

fn recording(steps: serde_json::Value) -> Recording {
    serde_json::from_value(json!({
        "version": 1, "name": "search", "startedAtMs": 0, "appVersion": "0.9.2",
        "breakpoints": [
            { "id": "mobile", "width": 390, "height": 844, "deviceScaleFactor": 3.0 },
            { "id": "desktop", "width": 1440, "height": 900, "deviceScaleFactor": 1.0 }
        ],
        "steps": steps
    }))
    .expect("a recording")
}

fn three() -> Recording {
    recording(json!([
        { "id": 1, "action": { "type": "navigate", "url": "https://example.test/" }, "view": "mobile", "atMs": 0 },
        { "id": 4, "action": { "type": "press", "key": "Enter" }, "view": "mobile", "atMs": 900, "disabled": true },
        { "id": 2, "action": { "type": "waitForUrl", "url": "https://example.test/s" }, "view": "desktop", "atMs": 2000 }
    ]))
}

fn ids(recording: &Recording) -> Vec<u32> {
    recording.steps.iter().map(|step| step.id).collect()
}

#[test]
fn the_next_id_is_one_above_the_highest() {
    assert_eq!(next_id(&three()), 5);
    assert_eq!(next_id(&recording(json!([]))), 1);
}

#[test]
fn a_new_step_takes_the_view_and_time_of_the_step_before_it() {
    let base = three();
    let step = new_step(&base, Place::After(4), Action::Wait { ms: 500 }, Some(300))
        .expect("step 4 exists");
    assert_eq!(
        (step.id, step.view.as_str(), step.at_ms),
        (5, "mobile", 900)
    );
    assert_eq!(step.pause_ms, Some(300));
    assert!(!step.disabled && step.only_in.is_empty());
    let first = new_step(&base, Place::Start, Action::Wait { ms: 1 }, None).expect("the start");
    assert_eq!((first.view.as_str(), first.at_ms), ("mobile", 0));
    let empty = recording(json!([]));
    let lone = new_step(&empty, Place::End, Action::Wait { ms: 1 }, None).expect("the end");
    assert_eq!((lone.id, lone.view.as_str()), (1, "mobile"));
    assert_eq!(
        new_step(&base, Place::After(9), Action::Wait { ms: 1 }, None),
        Err(EditError::NoStep(9))
    );
}

#[test]
fn a_step_is_inserted_at_its_place() {
    let mut edited = three();
    let step = new_step(&edited, Place::After(1), Action::Wait { ms: 1 }, None).expect("step");
    assert_eq!(insert(&mut edited, Place::After(1), step), Ok(1));
    assert_eq!(ids(&edited), [1, 5, 4, 2]);
    let start = new_step(&edited, Place::Start, Action::Wait { ms: 1 }, None).expect("step");
    assert_eq!(insert(&mut edited, Place::Start, start), Ok(0));
    let end = new_step(&edited, Place::End, Action::Wait { ms: 1 }, None).expect("step");
    assert_eq!(insert(&mut edited, Place::End, end), Ok(5));
    assert_eq!(ids(&edited), [6, 1, 5, 4, 2, 7]);
}

#[test]
fn an_id_already_taken_is_refused() {
    let mut edited = three();
    let mut step = new_step(&edited, Place::End, Action::Wait { ms: 1 }, None).expect("step");
    step.id = 2;
    assert_eq!(
        insert(&mut edited, Place::End, step),
        Err(EditError::DuplicateId(2))
    );
}

#[test]
fn place_reads_after_as_the_start_at_zero_and_the_end_when_absent() {
    assert_eq!(Place::from_after(None), Place::End);
    assert_eq!(Place::from_after(Some(0)), Place::Start);
    assert_eq!(Place::from_after(Some(3)), Place::After(3));
}

#[test]
fn a_patch_changes_only_what_it_names() {
    let mut edited = three();
    let patch: StepPatch = serde_json::from_value(json!({
        "name": "Open the start", "pauseMs": 700, "onlyIn": ["desktop"]
    }))
    .expect("a patch");
    apply(&mut edited, 1, &patch).expect("step 1 exists");
    let step = &edited.steps[0];
    assert_eq!(step.name.as_deref(), Some("Open the start"));
    assert_eq!(step.pause_ms, Some(700));
    assert_eq!(step.only_in, ["desktop"]);
    assert_eq!(step.at_ms, 0);
    // `null` takes a name or a pause away; an absent field leaves it.
    let clear: StepPatch =
        serde_json::from_value(json!({ "name": null, "pauseMs": null })).expect("a patch");
    apply(&mut edited, 1, &clear).expect("step 1 exists");
    assert_eq!(
        (edited.steps[0].name.clone(), edited.steps[0].pause_ms),
        (None, None)
    );
    assert_eq!(edited.steps[0].only_in, ["desktop"]);
    assert_eq!(apply(&mut edited, 8, &clear), Err(EditError::NoStep(8)));
}

#[test]
fn a_patch_with_an_unknown_field_is_refused() {
    assert!(serde_json::from_value::<StepPatch>(json!({ "id": 3 })).is_err());
}

#[test]
fn a_step_is_switched_off_and_on_and_removed() {
    let mut edited = three();
    assert_eq!(set_disabled(&mut edited, 1, true), Ok(()));
    assert!(edited.steps[0].disabled);
    assert_eq!(set_disabled(&mut edited, 4, false), Ok(()));
    assert!(!edited.steps[1].disabled);
    let removed = remove(&mut edited, 4).expect("step 4 exists");
    assert_eq!(removed.id, 4);
    assert_eq!(ids(&edited), [1, 2]);
    assert_eq!(remove(&mut edited, 4), Err(EditError::NoStep(4)));
}

#[test]
fn the_check_refuses_twin_ids_and_too_long_pauses_and_waits() {
    assert_eq!(check(&three()), Ok(()));
    let twins = recording(json!([
        { "id": 1, "action": { "type": "wait", "ms": 1 }, "view": "mobile", "atMs": 0 },
        { "id": 1, "action": { "type": "wait", "ms": 1 }, "view": "mobile", "atMs": 0 }
    ]));
    assert_eq!(check(&twins), Err(EditError::DuplicateId(1)));
    let pause = recording(json!([
        { "id": 1, "action": { "type": "wait", "ms": 1 }, "view": "mobile", "atMs": 0, "pauseMs": 10001 }
    ]));
    assert_eq!(
        check(&pause),
        Err(EditError::Pause {
            step: 1,
            ms: 10_001
        })
    );
    let wait = recording(json!([
        { "id": 3, "action": { "type": "wait", "ms": 60000 }, "view": "mobile", "atMs": 0 }
    ]));
    assert_eq!(
        check(&wait),
        Err(EditError::Wait {
            step: 3,
            ms: 60_000
        })
    );
}

#[test]
fn playing_leaves_out_the_steps_switched_off() {
    assert_eq!(ids(&playable(&three())), [1, 2]);
}

#[test]
fn from_a_step_on_plays_it_and_the_ones_after() {
    assert_eq!(ids(&from_step(&three(), 4).expect("step 4")), [2]);
    assert_eq!(ids(&from_step(&three(), 1).expect("step 1")), [1, 2]);
    assert_eq!(from_step(&three(), 7), Err(EditError::NoStep(7)));
}

#[test]
fn up_to_a_step_plays_the_ones_before_it_and_it() {
    assert_eq!(ids(&up_to(&three(), 2).expect("step 2")), [1, 2]);
    assert_eq!(ids(&up_to(&three(), 4).expect("step 4")), [1]);
    assert_eq!(up_to(&three(), 7), Err(EditError::NoStep(7)));
}

#[test]
fn one_step_alone_is_tried_even_when_switched_off() {
    let only = only_step(&three(), 4).expect("step 4");
    assert_eq!(ids(&only), [4]);
    assert!(!only.steps[0].disabled);
    assert_eq!(only.name, "search");
}

/// "Continue recording from here": the new steps land after the step, with new ids and times
/// from it on; the steps after keep their order and the pauses between them.
#[test]
fn a_recording_continued_from_a_step_takes_the_new_steps_after_it() {
    let base = three();
    let segment = recording(json!([
        { "id": 1, "action": { "type": "wait", "ms": 1 }, "view": "tablet", "atMs": 300 },
        { "id": 2, "action": { "type": "wait", "ms": 2 }, "view": "tablet", "atMs": 1300, "onlyIn": ["tablet"] }
    ]));
    let mut segment = segment;
    segment.breakpoints.push(RecordedBreakpoint {
        id: "tablet".to_owned(),
        width: 820,
        height: 1180,
        device_scale_factor: 2.0,
    });
    let spliced = splice(&base, 1, &segment).expect("step 1 exists");
    assert_eq!(ids(&spliced), [1, 5, 6, 4, 2]);
    let at: Vec<u64> = spliced.steps.iter().map(|step| step.at_ms).collect();
    assert_eq!(at, [0, 300, 1300, 2200, 3300]);
    assert_eq!(spliced.steps[2].only_in, ["tablet"]);
    let breakpoints: Vec<&str> = spliced
        .breakpoints
        .iter()
        .map(|breakpoint| breakpoint.id.as_str())
        .collect();
    assert_eq!(breakpoints, ["mobile", "desktop", "tablet"]);
    assert_eq!(spliced.name, "search");
    assert_eq!(splice(&base, 9, &segment), Err(EditError::NoStep(9)));
}

#[test]
fn an_empty_continuation_leaves_the_recording_as_it_was() {
    let base = three();
    assert_eq!(
        splice(&base, 4, &recording(json!([]))).expect("spliced"),
        base
    );
}

fn secret_fill(name: &str, path: &str) -> Action {
    serde_json::from_value(json!({
        "type": "fill", "target": { "selectors": [{ "kind": "css", "path": path }] },
        "value": format!("{{{{secret:{name}}}}}"), "secret": true
    }))
    .expect("a secret fill")
}

/// RB-092-01 security review: a secret fill is the recorder's own - an agent's edit neither adds
/// one nor changes one, so a trusted replay never types a secret into an element it chose.
#[test]
fn an_edit_neither_adds_nor_changes_a_secret_fill() {
    let plain: Action = serde_json::from_value(json!({
        "type": "fill", "target": { "selectors": [{ "kind": "css", "path": "#q" }] }, "value": "shoes"
    }))
    .expect("a fill");
    let secret = secret_fill("password", "#password");
    assert_eq!(check_secret(7, None, &plain), Ok(()));
    assert_eq!(check_secret(7, None, &secret), Err(EditError::Secret(7)));
    // Left as it is - a patch that names the same action - it passes.
    assert_eq!(check_secret(7, Some(&secret), &secret), Ok(()));
    // Another field, another secret, or no secret any more: refused.
    assert_eq!(
        check_secret(7, Some(&secret), &secret_fill("password", "#evil")),
        Err(EditError::Secret(7))
    );
    assert_eq!(
        check_secret(7, Some(&secret), &secret_fill("token", "#password")),
        Err(EditError::Secret(7))
    );
    assert_eq!(
        check_secret(7, Some(&secret), &plain),
        Err(EditError::Secret(7))
    );
    assert_eq!(
        check_secret(7, Some(&plain), &secret),
        Err(EditError::Secret(7))
    );
    assert_eq!(check_secret(7, Some(&plain), &plain), Ok(()));
}
