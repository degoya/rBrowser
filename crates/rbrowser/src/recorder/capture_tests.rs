use rb_config::RecordHover;
use rb_sync::SyncSwitches;
use rb_sync::recorder::HoverCapture;

use super::*;

/// RB-050-01 review: a recording the agent would never feed is refused, with the reason.
#[test]
fn a_recording_nothing_feeds_is_refused() {
    let on = SyncSwitches::default();
    assert_eq!(uncaptured(on, false, true), None);
    let off = SyncSwitches {
        enabled: false,
        ..on
    };
    assert!(uncaptured(off, false, true).is_some_and(|why| why.contains("sync is off")));
    assert!(uncaptured(on, true, true).is_some_and(|why| why.contains("excluded")));
    assert!(uncaptured(on, false, false).is_some_and(|why| why.contains("no view")));
}

/// The step kinds a recording lacks because their sync switch is off.
#[test]
fn the_kinds_not_captured_follow_the_switches_and_the_settings() {
    let on = SyncSwitches::default();
    assert!(missing_kinds(on, true, true).is_empty());
    let off = SyncSwitches {
        keyboard: false,
        hover: false,
        scroll: false,
        ..on
    };
    assert_eq!(missing_kinds(off, true, false), ["keyboard", "hover"]);
    assert_eq!(missing_kinds(off, false, true), ["keyboard", "scroll"]);
}

/// RB-090-81: the setting's choice is the recorder's, in one place.
#[test]
fn the_record_hover_setting_is_the_recorders_choice() {
    assert_eq!(hover_capture(RecordHover::Menu), HoverCapture::Menu);
    assert_eq!(hover_capture(RecordHover::All), HoverCapture::All);
    assert_eq!(hover_capture(RecordHover::Off), HoverCapture::Off);
}
