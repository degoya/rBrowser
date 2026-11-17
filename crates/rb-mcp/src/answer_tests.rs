use serde_json::json;

use super::capped;

#[test]
fn a_small_value_passes_and_a_large_one_is_cut() {
    assert_eq!(capped(json!({ "a": 1 }), 100), json!({ "a": 1 }));
    let cut = capped(json!("\u{2603}".repeat(100)), 11);
    assert_eq!(cut["truncated"], true);
    assert_eq!(cut["bytes"], 302);
    assert!(cut["preview"].as_str().is_some_and(|text| text.len() <= 11));
}
