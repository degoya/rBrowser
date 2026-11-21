use rb_plugin_api::Capability;
use rb_plugin_api::test_support::MemoryHost;
use serde_json::json;

use super::*;

fn config() -> String {
    json!({ "environments": [
        { "id": "local", "url": "http://localhost:3000" },
        { "id": "staging", "url": "https://staging.example.test/" },
        { "id": "prod", "url": "HTTPS://Example.test:443" },
        { "id": "bad id!", "url": "https://x.test" },
        { "id": "ftp", "url": "ftp://files.example.test" },
        { "id": "staging", "url": "https://twice.example.test" },
        { "url": "https://no-id.example.test" },
        "not an object"
    ]})
    .to_string()
}

#[test]
fn the_environments_are_read_with_their_origins() {
    let environments = parse(config().as_bytes());
    let read: Vec<(&str, &str)> = environments
        .iter()
        .map(|environment| (environment.id.as_str(), environment.origin.as_str()))
        .collect();
    assert_eq!(
        read,
        [
            ("local", "http://localhost:3000"),
            ("staging", "https://staging.example.test"),
            ("prod", "https://example.test")
        ]
    );
}

#[test]
fn a_broken_config_has_no_environments() {
    for text in ["", "not json", "[]", "{\"environments\": {}}"] {
        assert!(parse(text.as_bytes()).is_empty(), "{text}");
    }
}

#[test]
fn at_most_the_bound_is_read() {
    let many: Vec<Value> = (0..MAX_ENVIRONMENTS + 10)
        .map(|n| json!({ "id": format!("e{n}"), "url": format!("https://e{n}.example.test") }))
        .collect();
    let text = json!({ "environments": many }).to_string();
    assert_eq!(parse(text.as_bytes()).len(), MAX_ENVIRONMENTS);
}

#[test]
fn the_config_is_read_through_project_read() {
    let mut host = MemoryHost::new()
        .grant(Capability::ProjectRead)
        .with_file(CONFIG_PATH, config().as_bytes());
    assert_eq!(read(&mut host).len(), 3);
    assert!(read(&mut MemoryHost::new()).is_empty());
}

#[test]
fn a_url_belongs_to_the_environment_of_its_origin() {
    let environments = parse(config().as_bytes());
    let id = |url: &str| of_url(&environments, url).map(|environment| environment.id.as_str());
    assert_eq!(id("https://example.test/shop?x=1"), Some("prod"));
    assert_eq!(id("http://localhost:3000/"), Some("local"));
    assert_eq!(id("http://localhost:3001/"), None);
    assert_eq!(id("about:blank"), None);
}

#[test]
fn the_path_is_kept_in_another_environment() {
    assert_eq!(
        path_of("https://example.test/shop/a?x=1#top"),
        "/shop/a?x=1"
    );
    assert_eq!(path_of("https://example.test"), "/");
    assert_eq!(path_of("https://example.test?x=1"), "/?x=1");
    assert_eq!(path_of("not a url"), "/");
    let staging = Environment {
        id: "staging".to_owned(),
        origin: "https://staging.example.test".to_owned(),
    };
    assert_eq!(
        rebase(&staging, "/shop?x=1"),
        "https://staging.example.test/shop?x=1"
    );
}
