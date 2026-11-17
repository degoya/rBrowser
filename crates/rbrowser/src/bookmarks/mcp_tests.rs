use rb_config::Environment;
use serde_json::json;

use super::*;
use crate::sync_hub::tests::{Setup, setup};

fn with_bookmarks(setup: &Setup, environments: &[(&str, &str)]) {
    let host = setup.core.host();
    let mut config = host.config();
    config.environments = environments
        .iter()
        .map(|(id, url)| Environment {
            id: (*id).to_owned(),
            url: (*url).to_owned(),
            color: None,
            reload_interval_sec: None,
            asset_origins: Vec::new(),
        })
        .collect();
    config.bookmarks = serde_json::from_value(json!([
        { "id": "cart", "label": "Cart", "path": "/shop/cart?x=1#top", "group": "Shop",
          "shortcut": "Alt+F7" },
        { "id": "docs", "label": "Docs", "url": "https://docs.example.com/guide" }
    ]))
    .expect("bookmarks");
    host.open_config(config, None).expect("opens");
}

#[tokio::test(flavor = "multi_thread")]
async fn each_bookmark_says_whether_it_is_relative_and_where_it_leads_now() {
    let setup = setup();
    with_bookmarks(
        &setup,
        &[
            ("local", "http://localhost:5173"),
            ("staging", "https://staging.example.com/"),
        ],
    );
    let core = &setup.core;
    core.host()
        .update_shell_state(|shell| shell.environment = Some("staging".to_owned()));
    assert_eq!(
        bookmarks_list(core, &json!({})).expect("answers"),
        json!({
            "environment": "staging",
            "base": "https://staging.example.com/",
            "followEnvironment": true,
            "bookmarks": [
                { "id": "cart", "label": "Cart", "group": "Shop", "shortcut": "Alt+F7",
                  "relative": true, "path": "/shop/cart?x=1#top",
                  "url": "https://staging.example.com/shop/cart?x=1#top" },
                { "id": "docs", "label": "Docs", "relative": false,
                  "url": "https://docs.example.com/guide" }
            ]
        })
    );
    core.host()
        .update_shell_state(|shell| shell.environment = Some("local".to_owned()));
    let answer = bookmarks_list(core, &Value::Null).expect("answers");
    assert_eq!(
        answer["bookmarks"][0]["url"],
        "http://localhost:5173/shop/cart?x=1#top"
    );
    assert_eq!(
        answer["bookmarks"][1]["url"],
        "https://docs.example.com/guide"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_path_with_nothing_to_lie_under_has_no_url_and_arguments_are_refused() {
    let setup = setup();
    with_bookmarks(&setup, &[]);
    let core = &setup.core;
    let mut user = core.user_settings();
    user.bookmarks.follow_environment = false;
    core.replace_user_settings(user);
    let answer = bookmarks_list(core, &json!({})).expect("answers");
    assert_eq!(answer["base"], Value::Null);
    assert_eq!(answer["followEnvironment"], false);
    assert_eq!(
        answer["bookmarks"][0],
        json!({ "id": "cart", "label": "Cart", "group": "Shop", "shortcut": "Alt+F7",
                "relative": true, "path": "/shop/cart?x=1#top" })
    );
    let refused = bookmarks_list(core, &json!({ "target": "all" })).expect_err("refused");
    assert_eq!(refused.code, "invalid_arguments");
}
