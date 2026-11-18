use rb_sync::recorder::AutofillKind;
use serde_json::json;

use super::*;

fn target(path: &str) -> Value {
    json!({ "element": { "kind": "css", "path": path }, "offset": { "x": 0.5, "y": 0.5 } })
}

#[test]
fn a_post_names_its_view_and_the_fields_that_hold() {
    let fill = parse(&json!({ "action": RECORD_FILL, "view": 3, "fields": [
        { "target": target("#email"), "kind": "fill", "value": "ada@example.com" },
        { "target": target("#terms"), "kind": "check" },
        // A field that does not hold is left out, the rest stays.
        { "target": target("#x"), "kind": "click" },
        { "kind": "fill", "value": "no target" },
        "#y"
    ] }))
    .expect("a fill");
    assert_eq!(fill.view, ViewId(3));
    assert_eq!(fill.fields.len(), 2);
    assert_eq!(fill.fields[0].value.as_deref(), Some("ada@example.com"));
    assert_eq!(fill.fields[1].kind, AutofillKind::Check);
}

#[test]
fn a_post_without_a_view_or_fields_is_none() {
    assert!(parse(&json!({ "action": RECORD_FILL, "fields": [] })).is_none());
    assert!(parse(&json!({ "action": RECORD_FILL, "view": -1, "fields": [] })).is_none());
    assert!(
        parse(&json!({ "action": RECORD_FILL, "view": 4_294_967_296_u64, "fields": [] })).is_none()
    );
    assert!(parse(&json!({ "action": RECORD_FILL, "view": 1 })).is_none());
    assert!(parse(&json!({ "action": RECORD_FILL, "view": 1, "fields": {} })).is_none());
}

#[test]
fn a_post_takes_at_most_the_fields_a_page_reads() {
    let fields: Vec<Value> = (0..MAX_FIELDS + 5)
        .map(|at| json!({ "target": target(&format!("#f{at}")), "kind": "fill", "value": "x" }))
        .collect();
    let fill = parse(&json!({ "view": 1, "fields": fields })).expect("a fill");
    assert_eq!(fill.fields.len(), MAX_FIELDS);
}
