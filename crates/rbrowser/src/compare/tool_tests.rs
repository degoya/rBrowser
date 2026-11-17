use serde_json::json;

use super::*;

#[test]
fn the_compare_arguments_read_and_hold_their_limits() {
    let args = CompareArgs::parse(&json!({
        "environments": ["local", "staging"],
        "paths": ["/", "/about"],
        "breakpoints": ["mobile"],
        "views": [2],
        "fullPage": false,
        "threshold": 0.2
    }))
    .expect("reads");
    assert_eq!(args.environments, ["local", "staging"]);
    assert_eq!(args.views, [2]);
    assert_eq!(args.full_page, Some(false));
    assert_eq!(CompareArgs::parse(&json!(null)), Ok(CompareArgs::default()));
    assert!(CompareArgs::parse(&json!({ "target": "all" })).is_err());
    assert!(CompareArgs::parse(&json!({ "threshold": 2 })).is_err());
    assert!(CompareArgs::parse(&json!({ "paths": [""] })).is_err());
    let many: Vec<String> = (0..=MAX_PATHS).map(|n| format!("/{n}")).collect();
    assert!(CompareArgs::parse(&json!({ "paths": many })).is_err());
}

#[test]
fn the_result_and_live_arguments_read() {
    assert_eq!(
        ResultArgs::parse(&json!({ "runId": "r1" }))
            .expect("reads")
            .run_id
            .as_deref(),
        Some("r1")
    );
    assert!(ResultArgs::parse(&json!({ "runId": "" })).is_err());
    let live = LiveArgs::parse(&json!({ "environments": ["a", "b"], "view": 3, "enabled": true }))
        .expect("reads");
    assert_eq!(live.view, Some(3));
    assert_eq!(live.enabled, Some(true));
    assert!(LiveArgs::parse(&json!({ "target": 3 })).is_err());
}
