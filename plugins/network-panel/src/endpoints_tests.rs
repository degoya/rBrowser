use std::collections::BTreeSet;

use rb_plugin_api::ViewId;
use serde_json::json;

use super::{Options, by_view, group, key_of};
use crate::calls::{Call, calls};
use crate::template::templates;

/// A call of `view` to `method url` at `time`, `duration_ms` long.
pub(crate) fn call(view: u32, method: &str, url: &str, time: f64, duration_ms: f64) -> Call {
    let args = json!({ "$console": { "views": [{ "view": view, "network": [
        { "id": format!("{view}.{time}"), "url": url, "method": method, "resourceType": "Fetch",
          "status": 200, "time": time, "durationMs": duration_ms },
    ] }] } });
    calls(&args).remove(0)
}

fn fetches() -> Options {
    Options {
        types: BTreeSet::from(["Fetch".to_owned(), "XHR".to_owned()]),
        ..Options::default()
    }
}

#[test]
fn the_key_is_method_origin_and_templated_path() {
    assert_eq!(
        key_of("get", "https://a.test/api/users/42?x=1", &[]),
        Some((
            "GET".to_owned(),
            "https://a.test".to_owned(),
            "/api/users/{id}".to_owned()
        ))
    );
    assert_eq!(key_of("GET", "data:x", &[]), None);
}

#[test]
fn calls_to_one_endpoint_are_one_group_whatever_their_ids() {
    let found = [
        call(1, "GET", "https://a.test/api/users/1", 1.0, 10.0),
        call(1, "GET", "https://a.test/api/users/2", 2.0, 20.0),
        call(2, "GET", "https://a.test/api/users/3?page=2", 3.0, 30.0),
        call(1, "POST", "https://a.test/api/users/1", 4.0, 40.0),
        call(1, "GET", "https://b.test/api/users/1", 5.0, 50.0),
    ];
    let groups = group(&found, &fetches());
    let keys: Vec<&str> = groups
        .iter()
        .map(|endpoint| endpoint.key.as_str())
        .collect();
    assert_eq!(
        keys,
        [
            "GET https://a.test/api/users/{id}",
            "POST https://a.test/api/users/{id}",
            "GET https://b.test/api/users/{id}",
        ]
    );
    assert_eq!(groups[0].calls.len(), 3);
    let views: Vec<ViewId> = by_view(&groups[0]).into_keys().collect();
    assert_eq!(views, [1, 2]);
}

#[test]
fn the_options_choose_types_views_the_current_page_and_a_search() {
    let mut script = call(1, "GET", "https://a.test/app.js", 1.0, 5.0);
    script.resource_type = "Script".to_owned();
    let mut old = call(1, "GET", "https://a.test/api/old", 2.0, 5.0);
    old.current = false;
    let found = [
        script,
        old,
        call(1, "GET", "https://a.test/api/new", 3.0, 5.0),
        call(2, "GET", "https://a.test/api/other", 4.0, 5.0),
    ];
    let keys = |options: &Options| -> Vec<String> {
        group(&found, options)
            .into_iter()
            .map(|endpoint| endpoint.path)
            .collect()
    };
    assert_eq!(keys(&fetches()), ["/api/old", "/api/new", "/api/other"]);
    assert_eq!(keys(&Options::default()).len(), 4);
    let current = Options {
        current_only: true,
        view: Some(1),
        ..fetches()
    };
    assert_eq!(keys(&current), ["/api/new"]);
    let search = Options {
        search: Some("oth".to_owned()),
        ..fetches()
    };
    assert_eq!(keys(&search), ["/api/other"]);
}

#[test]
fn own_patterns_group_slugs() {
    let found = [
        call(1, "GET", "https://a.test/shop/red-shoes", 1.0, 5.0),
        call(1, "GET", "https://a.test/shop/blue-hat", 2.0, 5.0),
    ];
    let options = Options {
        templates: templates(&["/shop/{slug}".to_owned()]),
        ..fetches()
    };
    let groups = group(&found, &options);
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].path, "/shop/{slug}");
}
