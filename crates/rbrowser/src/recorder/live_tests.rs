use rb_engine::ViewId;
use rb_sync::recorder::{RecordedBreakpoint, RecorderOptions, RecordingView, StartInfo};

use super::*;

const VIEW: ViewId = ViewId(1);

fn recording() -> Recorder {
    let mut recorder = Recorder::new(RecorderOptions::default());
    recorder
        .start(StartInfo {
            name: "rec".to_owned(),
            start_url: Some("http://localhost:3000/".to_owned()),
            environment: None,
            app_version: "0.9.0".to_owned(),
            views: vec![RecordingView {
                view: VIEW,
                breakpoint: RecordedBreakpoint {
                    id: "w2000".to_owned(),
                    width: 2000,
                    height: 1250,
                    device_scale_factor: 1.0,
                },
            }],
            leader: Some(VIEW),
            now_ms: 1_000,
        })
        .expect("starts");
    recorder
}

fn shape(push: Option<RecordingSteps>) -> Option<(String, u32, usize, LiveState)> {
    push.map(|push| (push.name, push.from, push.steps.len(), push.state))
}

fn recording_from(from: u32, steps: usize) -> Option<(String, u32, usize, LiveState)> {
    Some(("rec".to_owned(), from, steps, LiveState::Recording))
}

#[test]
fn the_start_sends_every_step_then_only_what_changed() {
    let mut recorder = recording();
    let mut shown = None;
    assert_eq!(
        shape(next(&mut shown, &mut recorder, true, false)),
        recording_from(0, 1),
        "the start's navigation"
    );
    assert_eq!(next(&mut shown, &mut recorder, true, false), None);
    recorder.on_navigated(VIEW, "http://localhost:3000/shop", 2_000);
    assert_eq!(
        shape(next(&mut shown, &mut recorder, true, false)),
        recording_from(1, 1),
        "only the new step"
    );
    // A panel catching up gets every step.
    assert_eq!(
        shape(next(&mut shown, &mut recorder, true, true)),
        recording_from(0, 2)
    );
    assert_eq!(next(&mut shown, &mut recorder, true, false), None);
}

#[test]
fn the_setting_off_hides_the_list_once_and_on_again_sends_it_all() {
    let mut recorder = recording();
    let mut shown = None;
    next(&mut shown, &mut recorder, true, false);
    assert_eq!(
        shape(next(&mut shown, &mut recorder, false, false)),
        Some(("rec".to_owned(), 0, 0, LiveState::Hidden)),
        "no end: the recording goes on"
    );
    recorder.on_navigated(VIEW, "http://localhost:3000/shop", 2_000);
    assert_eq!(next(&mut shown, &mut recorder, false, false), None);
    // Off, the changes are let go: they would make every event publish.
    assert!(!recorder.changed());
    assert_eq!(
        next(&mut shown, &mut recorder, false, true),
        None,
        "nothing to catch up"
    );
    assert_eq!(
        shape(next(&mut shown, &mut recorder, true, false)),
        recording_from(0, 2)
    );
}

#[test]
fn the_saved_recording_ends_the_list_once() {
    let mut recorder = recording();
    let mut shown = None;
    next(&mut shown, &mut recorder, true, false);
    recorder.stop().expect("stops");
    assert_eq!(
        shape(next(&mut shown, &mut recorder, true, false)),
        Some(("rec".to_owned(), 0, 0, LiveState::Ended))
    );
    assert_eq!(next(&mut shown, &mut recorder, true, false), None);
    assert_eq!(next(&mut shown, &mut recorder, true, true), None);
}
