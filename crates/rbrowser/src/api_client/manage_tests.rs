use serde_json::json;

use super::*;
use crate::api_client::fakes::{UrlSender, context, no_env};
use crate::api_client::ops;

fn op(value: serde_json::Value) -> Op {
    let answer = json!({ "$apiClient": value }).to_string();
    crate::api_client::protocol::requested(&answer)
        .expect("asks")
        .expect("reads")
}

#[test]
fn a_collection_and_a_request_in_it_through_the_ops() {
    let dir = tempfile::tempdir().expect("temp dir");
    let ctx = context(dir.path());
    let sender = UrlSender::new(vec![]);
    let made = ops::run(
        op(json!({ "op": "createFolder", "folder": "users" })),
        &ctx,
        &sender,
        &no_env,
    )
    .expect("made");
    assert_eq!(made.answer, json!({ "folder": "users" }));
    let state = ops::run(Op::State, &ctx, &sender, &no_env).expect("state");
    assert_eq!(state.answer["folders"], json!(["users"]));
    let request = json!({ "name": "Me", "method": "GET", "url": "{{baseUrl}}/me" });
    let saved = ops::run(
        op(json!({ "op": "save", "folder": "users", "request": request })),
        &ctx,
        &sender,
        &no_env,
    )
    .expect("saved");
    assert_eq!(saved.answer["id"], "users/me.http");
    let deleted = ops::run(
        op(json!({ "op": "deleteFolder", "folder": "users" })),
        &ctx,
        &sender,
        &no_env,
    )
    .expect("deleted");
    assert_eq!(deleted.answer, json!({ "deleted": "users", "count": 1 }));
    assert!(deleted.secrets.is_none());
}

#[test]
fn a_private_value_set_is_kept_by_the_window_and_never_answered() {
    let dir = tempfile::tempdir().expect("temp dir");
    let ctx = context(dir.path());
    let sender = UrlSender::new(vec![]);
    let set = ops::run(
        op(json!({ "op": "setSecret", "name": "password", "value": "pw-very-secret" })),
        &ctx,
        &sender,
        &no_env,
    )
    .expect("set");
    assert_eq!(set.answer, json!({ "set": "password" }));
    assert_eq!(
        set.secrets
            .as_ref()
            .map(|secrets| secrets["$shared"]["password"].clone()),
        Some(json!("pw-very-secret"))
    );
    assert!(!format!("{set:?}").contains("pw-very-secret"));
    assert!(
        !format!(
            "{:?}",
            op(json!({ "op": "setSecret", "name": "p", "value": "pw-very-secret" }))
        )
        .contains("pw-very-secret")
    );
    let mut after = ctx.clone();
    after.secrets = set.secrets.expect("kept");
    let missing = ops::run(
        op(json!({ "op": "secrets", "names": ["password", "other"] })),
        &after,
        &sender,
        &no_env,
    )
    .expect("checked");
    assert_eq!(missing.answer, json!({ "missing": ["other"] }));
}
