use std::sync::Arc;

use rb_engine::{Target, ViewId};
use serde_json::json;

use super::{Answers, joined, totals, with_target};
use crate::target::TargetArg;
use crate::testing::{FakeBackend, HOST_TOOL_PREFIX, server, view};

#[test]
fn joined_appends_the_lists_and_keeps_the_last_of_the_rest() {
    let first = json!({ "views": [{ "view": 1 }], "mode": "css" });
    let second = json!({ "views": [{ "view": 3 }], "mode": "full" });
    assert_eq!(
        joined(first, second),
        json!({ "views": [{ "view": 1 }, { "view": 3 }], "mode": "full" })
    );
    assert_eq!(joined(json!(null), json!({ "a": 1 })), json!({ "a": 1 }));
}

#[test]
fn totals_joins_the_lists_and_adds_up_the_counts() {
    let first = json!({ "runId": "a", "comparisons": [1], "regressions": 1, "unreliable": 0 });
    let second = json!({ "runId": "b", "comparisons": [2], "regressions": 2, "unreliable": 1 });
    assert_eq!(
        totals(first, second),
        json!({ "runId": "b", "comparisons": [1, 2], "regressions": 3, "unreliable": 1 })
    );
    assert_eq!(totals(json!(null), json!({ "n": 2 })), json!({ "n": 2 }));
}

#[test]
fn with_target_sets_all_or_the_view_and_keeps_the_rest() {
    let args = json!({ "selector": "#go" });
    assert_eq!(
        with_target(&args, Some(Target::View(ViewId(3)))).expect("args"),
        json!({ "selector": "#go", "target": 3 })
    );
    assert_eq!(
        with_target(&args, Some(Target::All)).expect("args"),
        json!({ "selector": "#go", "target": "all" })
    );
    assert_eq!(with_target(&args, None).expect("args"), args);
    assert_eq!(with_target(&json!(null), None).expect("args"), json!({}));
}

#[tokio::test]
async fn a_breakpoint_shown_twice_runs_the_host_tool_once_per_view_and_joins_the_views() {
    let fake = Arc::new(FakeBackend::new(vec![
        view(1, "mobile", "about:blank"),
        view(2, "desktop", "about:blank"),
        view(3, "mobile", "about:blank"),
    ]));
    let id = format!("{HOST_TOOL_PREFIX}click");
    fake.answer(
        &id,
        Ok(json!({ "views": [{ "view": 9, "status": "done" }] })),
    );
    let answer = server(&fake)
        .host_per_target(
            "click",
            Some(&TargetArg::Name("mobile".to_owned())),
            json!({ "selector": "a" }),
            Answers::Joined,
        )
        .await
        .expect("answer");
    assert_eq!(answer["views"].as_array().map(Vec::len), Some(2));
    assert_eq!(
        fake.calls(),
        vec![
            (id.clone(), json!({ "selector": "a", "target": 1 })),
            (id, json!({ "selector": "a", "target": 3 })),
        ]
    );
}

#[tokio::test]
async fn without_target_the_host_tool_takes_its_own_default() {
    let fake = Arc::new(FakeBackend::default());
    server(&fake)
        .host_per_target("scroll", None, json!({ "to": "top" }), Answers::Joined)
        .await
        .expect("answer");
    assert_eq!(
        fake.calls(),
        vec![(format!("{HOST_TOOL_PREFIX}scroll"), json!({ "to": "top" }))]
    );
}

#[tokio::test]
async fn an_unknown_target_answers_the_known_views() {
    let fake = Arc::new(FakeBackend::default());
    let failure = server(&fake)
        .host_per_target(
            "hover",
            Some(&TargetArg::Name("tablet".to_owned())),
            json!({ "selector": "a" }),
            Answers::Joined,
        )
        .await
        .expect_err("refused");
    assert_eq!(failure.code, crate::target::VIEW_NOT_FOUND);
    assert!(fake.calls().is_empty());
}
