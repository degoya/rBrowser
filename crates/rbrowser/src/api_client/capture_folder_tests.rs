//! The folder of a host captured before its folder name was tagged (the w32 review).

use rb_api_model::ApiRequest;

use super::*;
use crate::api_client::fakes::context;

fn get(url: &str) -> Option<Captured> {
    Some(Captured {
        method: "GET".into(),
        url: url.into(),
        headers: Vec::new(),
        body: None,
        body_unread: false,
    })
}

const SAVE: Wanted = Wanted {
    save: true,
    open: false,
    keep: false,
};

fn saved_before(root: &std::path::Path, url: &str) {
    let mut request = ApiRequest::new("GET", url);
    request.name = "GET /old".into();
    Store::new(root)
        .save(None, Some("api-x-test"), &request)
        .expect("saved");
}

#[test]
fn a_host_captured_before_the_tag_keeps_its_untagged_folder() {
    let dir = tempfile::tempdir().expect("temp dir");
    saved_before(dir.path(), "https://api-x.test/old");
    let ctx = context(dir.path());
    let taken = run(&ctx, vec![get("https://api-x.test/new")], SAVE).expect("saves");
    assert_eq!(taken.answer["saved"][0]["id"], "api-x-test/get-new.http");
}

#[test]
fn an_untagged_folder_another_host_uses_is_not_shared() {
    let dir = tempfile::tempdir().expect("temp dir");
    saved_before(dir.path(), "https://api.x.test/old");
    let ctx = context(dir.path());
    let taken = run(&ctx, vec![get("https://api-x.test/new")], SAVE).expect("saves");
    let id = taken.answer["saved"][0]["id"].as_str().expect("an id");
    assert!(id.starts_with("api-x-test-"), "{id}");
    assert_eq!(folder_of_id(id), folder_of("api-x.test"));
}
