use rb_engine::{Target, ViewId};
use serde_json::json;

use super::*;

#[test]
fn a_message_goes_to_every_other_view_never_back() {
    let views = [ViewId(1), ViewId(2), ViewId(3)];
    assert_eq!(relay_targets(&views, ViewId(1)), vec![ViewId(2), ViewId(3)]);
    assert_eq!(relay_targets(&views, ViewId(9)), views.to_vec());
    assert!(relay_targets(&[ViewId(1)], ViewId(1)).is_empty());
}

#[test]
fn bus_send_takes_target_type_and_payload() {
    let args: BusSendArgs = serde_json::from_value(json!({
        "target": 2, "type": "flow:step", "payload": { "n": 1 }
    }))
    .expect("valid arguments");
    assert_eq!(args.target, Target::View(ViewId(2)));
    assert_eq!(args.r#type, "flow:step");
    let bare: BusSendArgs =
        serde_json::from_value(json!({ "type": "ping" })).expect("target and payload optional");
    assert_eq!(bare.target, Target::All);
    assert_eq!(bare.payload, Value::Null);
    assert!(
        serde_json::from_value::<BusSendArgs>(json!({ "type": "x", "origin": "evil" })).is_err(),
        "unknown fields are refused"
    );
}

#[test]
fn the_answer_lists_delivered_and_skipped_views() {
    let answer = BusSendAnswer {
        delivered: vec![ViewId(2)],
        skipped: vec![ViewId(3)],
    };
    assert_eq!(
        serde_json::to_value(answer).expect("serializes"),
        json!({ "delivered": [2], "skipped": [3] })
    );
}
