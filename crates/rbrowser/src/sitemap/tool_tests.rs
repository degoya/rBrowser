use serde_json::json;

use super::*;

#[test]
fn capture_args_read_and_check() {
    let args = CaptureArgs::parse(&json!({
        "url": "https://www.example.test/",
        "breakpoints": ["mobile", "desktop"],
        "env": "local",
        "max": 20,
        "compare": true
    }))
    .expect("reads");
    assert_eq!(args.breakpoints, ["mobile", "desktop"]);
    assert_eq!(args.env.as_deref(), Some("local"));
    assert!(args.compare);
    assert_eq!(CaptureArgs::parse(&json!(null)), Ok(CaptureArgs::default()));
}

#[test]
fn capture_args_out_of_bounds_are_refused() {
    let refused = |value: serde_json::Value| CaptureArgs::parse(&value).is_err();
    assert!(refused(json!({ "max": MAX_PAGES + 1 })));
    assert!(refused(json!({ "url": "" })));
    assert!(refused(
        json!({ "url": "https://a.test/", "bookmarks": true })
    ));
    assert!(refused(json!({ "breakpoints": vec!["a"; MAX_NAMES + 1] })));
    assert!(refused(json!({ "unknown": 1 })));
}

#[test]
fn run_args_take_an_optional_id() {
    assert_eq!(
        RunArgs::parse(&json!({ "runId": "2026-11-11_00-00-00" }))
            .expect("reads")
            .run_id
            .as_deref(),
        Some("2026-11-11_00-00-00")
    );
    assert_eq!(RunArgs::parse(&json!(null)), Ok(RunArgs::default()));
    assert!(RunArgs::parse(&json!({ "id": 1 })).is_err());
}

#[test]
fn the_tools_have_their_names() {
    assert_eq!(
        [CAPTURE_TOOL, STATUS_TOOL, CANCEL_TOOL],
        ["sitemap_capture", "sitemap_status", "sitemap_cancel"]
    );
    let started = serde_json::to_value(CaptureStarted {
        run_id: "r".to_owned(),
        folder: "f".to_owned(),
        pending: 76,
    })
    .expect("serialises");
    assert_eq!(
        started,
        json!({ "runId": "r", "folder": "f", "pending": 76 })
    );
}
