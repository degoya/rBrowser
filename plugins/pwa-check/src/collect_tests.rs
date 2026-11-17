use rb_plugin_api::Capability;
use rb_plugin_api::test_support::MemoryHost;
use serde_json::json;

use super::*;

#[test]
fn the_collector_answer_is_read() {
    let answer = json!({
        "url": "https://a.test/app/", "secure": true,
        "manifests": ["https://a.test/app/manifest.webmanifest"],
        "themeColor": true, "appleTouchIcon": "https://a.test/apple.png",
        "sw": { "supported": true, "controlled": true, "error": null, "registration": {
            "scope": "https://a.test/app/", "script": "https://a.test/app/sw.js",
            "state": "activated", "active": true } }
    });
    let mut host = MemoryHost::new().grant(Capability::Evaluate);
    host.evaluate_answer = answer.to_string();
    let state = collect(&mut host, 3).expect("state");
    assert_eq!(host.evaluated[0].0, 3);
    assert_eq!(host.evaluated[0].1, COLLECTOR);
    assert_eq!(state.url, "https://a.test/app/");
    assert!(state.secure && state.theme_color);
    assert_eq!(state.manifests.len(), 1);
    let registration = state.sw.registration.expect("registration");
    assert!(registration.active);
    assert_eq!(registration.state.as_deref(), Some("activated"));
}

#[test]
fn a_page_without_anything_reads_as_empty() {
    let mut host = MemoryHost::new().grant(Capability::Evaluate);
    host.evaluate_answer = json!({ "url": "http://a.test/", "secure": false }).to_string();
    let state = collect(&mut host, 1).expect("state");
    assert!(state.manifests.is_empty());
    assert!(!state.sw.supported && state.sw.registration.is_none());
}

#[test]
fn an_answer_that_is_no_state_is_bad_output() {
    let mut host = MemoryHost::new().grant(Capability::Evaluate);
    host.evaluate_answer = "null".to_owned();
    let failure = collect(&mut host, 1).expect_err("bad");
    assert_eq!(failure.code(), Some(codes::BAD_OUTPUT));
}

#[test]
fn the_offline_probe_embeds_the_url_as_a_json_string() {
    let script = offline_probe("https://a.test/app/?a=\"b\"</script>");
    assert!(script.contains(r#"caches.match("https://a.test/app/?a=\"b\"</script>""#));
    let mut host = MemoryHost::new().grant(Capability::Evaluate);
    host.evaluate_answer = "true".to_owned();
    assert_eq!(cached(&mut host, 1, "https://a.test/"), Some(true));
    host.evaluate_answer = "null".to_owned();
    assert_eq!(cached(&mut host, 1, "https://a.test/"), None);
}
