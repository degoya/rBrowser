use std::sync::Arc;

use serde_json::json;

use super::BOOKMARKS_LIST_TOOL;
use crate::testing::{FakeBackend, HOST_TOOL_PREFIX, SECRET_MARK, answer_json, server};

fn host_id() -> String {
    format!("{HOST_TOOL_PREFIX}{BOOKMARKS_LIST_TOOL}")
}

#[tokio::test]
async fn the_host_lists_the_bookmarks_and_a_secret_in_a_url_is_masked() {
    let fake = Arc::new(FakeBackend::default());
    fake.answer(
        &host_id(),
        Ok(json!({
            "environment": "staging",
            "base": "https://staging.example.com/",
            "followEnvironment": true,
            "bookmarks": [
                { "id": "cart", "label": "Cart", "relative": true, "path": "/shop/cart",
                  "url": "https://staging.example.com/shop/cart" },
                { "id": "hook", "label": "Hook", "relative": false,
                  "url": format!("https://hooks.example.com/{SECRET_MARK}") }
            ]
        })),
    );
    let (error, answer) = answer_json(&server(&fake).bookmarks_list().await.expect("answer"));
    assert!(!error, "{answer}");
    assert_eq!(answer["bookmarks"][0]["relative"], true);
    assert_eq!(answer["bookmarks"][0]["path"], "/shop/cart");
    assert_eq!(
        answer["bookmarks"][0]["url"],
        "https://staging.example.com/shop/cart"
    );
    assert_eq!(answer["bookmarks"][1]["relative"], false);
    assert_eq!(answer["bookmarks"][1]["url"], "***");
    assert_eq!(fake.calls(), [(host_id(), json!({}))]);
}

#[tokio::test]
async fn make_relative_runs_the_command_and_answers_the_number() {
    let fake = Arc::new(FakeBackend::default());
    fake.answer(
        "bookmarks.make_relative",
        Ok(json!({ "converted": 2, "ids": ["cart", "pdp"] })),
    );
    let (error, answer) = answer_json(
        &server(&fake)
            .bookmarks_make_relative()
            .await
            .expect("answer"),
    );
    assert!(!error, "{answer}");
    assert_eq!(answer, json!({ "converted": 2, "ids": ["cart", "pdp"] }));
    assert_eq!(
        fake.calls(),
        [("bookmarks.make_relative".to_owned(), json!({}))]
    );
}
