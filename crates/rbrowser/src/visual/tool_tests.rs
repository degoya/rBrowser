use rb_engine::ViewId;
use rb_visual::{Status, Subject};
use serde_json::json;

use super::*;

#[test]
fn compare_takes_a_target_and_a_threshold() {
    assert_eq!(
        CompareArgs::parse(&json!(null)).expect("none"),
        CompareArgs::default()
    );
    let args = CompareArgs::parse(&json!({ "target": 3, "threshold": 0.2 })).expect("args");
    assert_eq!(args.target, Some(Target::View(ViewId(3))));
    assert_eq!(args.threshold, Some(0.2));
    assert_eq!(
        CompareArgs::parse(&json!({ "target": "all" }))
            .expect("all")
            .target,
        Some(Target::All)
    );
}

#[test]
fn compare_refuses_a_threshold_out_of_range_and_unknown_fields() {
    for bad in [
        json!({ "threshold": 1.5 }),
        json!({ "threshold": -0.1 }),
        json!({ "thresh": 0.1 }),
        json!({ "target": "some" }),
    ] {
        assert!(CompareArgs::parse(&bad).is_err(), "{bad}");
    }
}

#[test]
fn update_takes_a_target_only() {
    assert_eq!(
        UpdateArgs::parse(&json!({ "target": 2 }))
            .expect("args")
            .target,
        Some(Target::View(ViewId(2)))
    );
    assert!(UpdateArgs::parse(&json!({ "threshold": 0.1 })).is_err());
}

#[test]
fn the_answer_counts_regressions_and_unreliable_views() {
    let subject = |view| Subject {
        view,
        ..Subject::default()
    };
    let differs = Comparison {
        subject: subject(1),
        status: Status::Differs,
        ..Comparison::default()
    };
    let size = Comparison {
        subject: subject(2),
        status: Status::SizeMismatch,
        meta_missing: true,
        ..Comparison::default()
    };
    let new = Comparison {
        subject: subject(3),
        status: Status::New,
        ..Comparison::default()
    };
    let answer = CompareAnswer::of("run-1".to_owned(), vec![differs, size, new], None);
    assert_eq!((answer.regressions, answer.unreliable), (2, 1));
    let value = serde_json::to_value(&answer).expect("serializes");
    assert_eq!(value["runId"], json!("run-1"));
    assert_eq!(value["comparisons"][1]["status"], json!("sizeMismatch"));
}

#[test]
fn the_tools_are_core_tools_without_a_prefix() {
    assert_eq!(COMPARE_TOOL, "visual_compare");
    assert_eq!(UPDATE_TOOL, "visual_baseline_update");
    assert!(!COMPARE_DESCRIPTION.is_empty() && !UPDATE_DESCRIPTION.is_empty());
}
