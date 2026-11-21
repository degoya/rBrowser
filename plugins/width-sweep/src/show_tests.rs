//! "Show at N px" on a finding (RB-093-01): the width held through the host's lease, ended at
//! Escape or after the time given, left alone when another width took the view.

use rb_plugin_api::{FailureKind, codes};
use serde_json::json;

use super::*;
use crate::fake::SweepHost;
use crate::probe::{CLEAR, SET};

#[test]
fn a_width_is_leased_held_and_given_back_at_escape() {
    let mut host = SweepHost::with_views(&[1, 2]);
    host.hold_end = "escape";
    let answer = show(&mut host, &json!({ "target": 2, "width": 730 })).expect("shown");
    assert_eq!(answer, json!({ "view": 2, "width": 730, "end": "escape" }));
    assert_eq!(host.methods(), [SET, CLEAR]);
    assert_eq!(host.cdp[0].0, 2, "the view the finding names");
    assert_eq!(host.cdp[0].2, json!({ "width": 730 }));
    assert_eq!(host.width, None, "the lease ended");
    assert_eq!(
        host.progress,
        [(0, 0)],
        "the activity bar shows it, with Cancel"
    );
}

#[test]
fn the_time_given_ends_it_too() {
    let mut host = SweepHost::with_views(&[1]);
    let answer = show(&mut host, &json!({ "width": 900 })).expect("shown");
    assert_eq!(answer["end"], "elapsed");
    assert_eq!(answer["view"], 1, "the active view without a target");
    assert_eq!(host.methods(), [SET, CLEAR]);
}

#[test]
fn another_width_on_the_view_is_left_alone() {
    let mut host = SweepHost::with_views(&[1]);
    host.hold_end = "resized";
    let answer = show(&mut host, &json!({ "width": 900 })).expect("shown");
    assert_eq!(answer["end"], "resized");
    assert_eq!(
        host.methods(),
        [SET],
        "the lease is no longer this call's: a cancel ended it or another width holds it"
    );
}

#[test]
fn the_hold_is_given_the_setting_in_milliseconds() {
    let mut host = SweepHost::with_views(&[1]);
    host.set("showSeconds", "30");
    show(&mut host, &json!({ "width": 900 })).expect("shown");
    let script = host.scripts.last().expect("evaluated");
    assert!(script.starts_with(crate::probe::DETECT));
    let call = script.trim_start_matches(crate::probe::DETECT);
    assert_eq!(
        call,
        format!(
            "\n;WidthSweepDetect.hold({})",
            json!({ "width": 900, "ms": 30_000 })
        )
    );
}

#[test]
fn a_width_out_of_range_or_missing_is_refused_before_anything_is_sent() {
    for args in [
        json!({}),
        json!({ "width": 119 }),
        json!({ "width": 3841 }),
        json!({ "width": "730" }),
    ] {
        let mut host = SweepHost::with_views(&[1]);
        let failure = show(&mut host, &args).expect_err("refused");
        assert_eq!(failure.code(), Some(codes::INVALID), "{args}");
        assert!(host.cdp.is_empty(), "{args}");
    }
}

#[test]
fn a_failing_page_still_ends_the_lease() {
    let mut host = SweepHost::with_views(&[1]);
    host.broken = true;
    let failure = show(&mut host, &json!({ "width": 900 })).expect_err("no answer");
    assert_eq!(failure.kind, FailureKind::BadData);
    assert_eq!(host.methods(), [SET, CLEAR]);
}

#[test]
fn a_view_of_another_engine_is_never_leased() {
    let mut host = SweepHost::with_views(&[1]);
    host.views[0].engine = "firefox".to_owned();
    assert!(show(&mut host, &json!({ "width": 900 })).is_err());
    assert!(host.cdp.is_empty());
}
