//! The acceptance fixtures of RB-093-01 at the plugin's side: a page that overflows only from
//! 700 to 740 px, a clean page, and Escape in the middle of a sweep.

use serde_json::json;

use super::*;
use crate::fake::{SweepHost, overflow};

fn hero_between(low: u32, high: u32) -> crate::fake::Page {
    Box::new(move |width| {
        if (low..=high).contains(&width) {
            vec![overflow(".hero img")]
        } else {
            Vec::new()
        }
    })
}

#[test]
fn an_overflow_only_from_700_to_740_is_one_range_within_a_step() {
    let mut host = SweepHost::with_views(&[1, 2, 3]);
    host.page = hero_between(700, 740);
    let answer = run(&mut host, &json!({})).expect("swept");
    assert_eq!(answer["end"], "done");
    assert_eq!(answer["count"], 1);
    let reported = host.reported.last().expect("reported");
    assert_eq!(reported.len(), 1, "exactly one range");
    let finding = &reported[0];
    let bound = |name: &str| -> i64 {
        finding
            .params
            .iter()
            .find(|(key, _)| key == name)
            .and_then(|(_, value)| value.parse().ok())
            .expect("a bound")
    };
    assert!((bound("from") - 700).abs() <= 10, "from {}", bound("from"));
    assert!((bound("to") - 740).abs() <= 10, "to {}", bound("to"));
    assert_eq!(answer["findings"][0]["from"], bound("from"));
    assert_eq!(answer["findings"][0]["selector"], ".hero img");
}

#[test]
fn a_clean_page_reports_no_finding() {
    let mut host = SweepHost::with_views(&[1]);
    let answer = run(&mut host, &json!({})).expect("swept");
    assert_eq!(answer["count"], 0);
    assert_eq!(answer["probes"], 161);
    assert_eq!(
        host.reported,
        vec![Vec::new()],
        "the empty run replaces the last one"
    );
}

#[test]
fn escape_mid_sweep_gives_the_view_back_and_leaves_the_others_alone() {
    let mut host = SweepHost::with_views(&[1, 2, 3]);
    host.active = Some(2);
    host.escape_at = Some(1000);
    let answer = run(&mut host, &json!({})).expect("ended");
    assert_eq!(answer["end"], "escaped");
    assert_eq!(answer["view"], 2);
    assert_eq!(
        host.width, None,
        "the lease ended: the view has its own width and sync"
    );
    assert_eq!(host.methods().last().map(String::as_str), Some(CLEAR));
    assert!(
        host.cdp.iter().all(|(view, _, _)| *view == 2),
        "only the swept view"
    );
    assert!(host.reported.is_empty(), "nothing half-swept is reported");
}

#[test]
fn the_target_names_the_view_and_the_arguments_the_range() {
    let mut host = SweepHost::with_views(&[1, 2]);
    let answer = run(
        &mut host,
        &json!({ "target": 2, "from": 400, "to": 500, "step": 50, "kinds": ["overlap"] }),
    )
    .expect("swept");
    assert_eq!(answer["view"], 2);
    assert_eq!(
        (answer["from"].clone(), answer["to"].clone()),
        (json!(400), json!(500))
    );
    assert_eq!(answer["probes"], 3);
    let failure = run(&mut host, &json!({ "target": 9 })).expect_err("no such view");
    assert_eq!(failure.code(), Some(rb_plugin_api::codes::NOT_FOUND));
    let failure = run(&mut host, &json!({ "target": "two" })).expect_err("bad target");
    assert_eq!(failure.param("argument"), Some("target"));
}

#[test]
fn a_firefox_view_is_not_swept() {
    let mut host = SweepHost::with_views(&[1]);
    host.views[0].engine = "firefox".to_owned();
    let failure = run(&mut host, &json!({})).expect_err("unserved");
    assert_eq!(failure.code(), Some(rb_plugin_api::codes::NOT_AVAILABLE));
    assert!(host.cdp.is_empty(), "its width is never touched");
}

#[test]
fn cancel_ends_the_lease_of_every_chromium_view() {
    let mut host = SweepHost::with_views(&[1, 2]);
    host.width = Some(800);
    let answer = cancel(&mut host).expect("cancelled");
    assert_eq!(answer, json!({ "cancelled": true }));
    assert_eq!(host.methods(), vec![CLEAR, CLEAR]);
    let answer = cancel(&mut host).expect("nothing to cancel");
    assert_eq!(answer, json!({ "cancelled": false }));
}

#[test]
fn arguments_must_be_an_object() {
    assert!(parse("").expect("empty is none").is_object());
    assert_eq!(
        parse("[]").expect_err("no object").code(),
        Some(rb_plugin_api::codes::INVALID)
    );
}
