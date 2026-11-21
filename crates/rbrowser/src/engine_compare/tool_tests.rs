use serde_json::json;

use super::*;

#[test]
fn null_is_no_argument_and_every_field_reads() {
    assert_eq!(
        CompareEnginesArgs::parse(&serde_json::Value::Null).expect("none"),
        CompareEnginesArgs::default()
    );
    let args = CompareEnginesArgs::parse(&json!({
        "breakpoint": "mobile",
        "engines": ["firefox", "webkit"],
        "url": "https://example.test/",
        "view": 3,
        "threshold": 0.2,
        "maxDiffPercent": 1.5,
        "open": false
    }))
    .expect("reads");
    assert_eq!(args.breakpoint.as_deref(), Some("mobile"));
    assert_eq!(args.engines, ["firefox", "webkit"]);
    assert_eq!(args.view, Some(3));
    assert_eq!(args.open, Some(false));
    assert_eq!(args.overrides().threshold, Some(0.2));
    assert_eq!(args.overrides().max_diff_percent, Some(1.5));
}

#[test]
fn arguments_out_of_bounds_are_refused() {
    for refused in [
        json!({ "threshold": 1.5 }),
        json!({ "maxDiffPercent": -1 }),
        json!({ "maxDiffPercent": 101 }),
        json!({ "engines": ["chromium", "firefox", "webkit", "chromium"] }),
        json!({ "engines": [""] }),
        json!({ "breakpoint": "" }),
        json!({ "url": "x".repeat(MAX_URL_LEN + 1) }),
        json!({ "unknown": true }),
    ] {
        assert!(CompareEnginesArgs::parse(&refused).is_err(), "{refused}");
    }
}
