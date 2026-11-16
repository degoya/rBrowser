//! `cdp.send` against a scripted service: the allowlist, the plugin's own world and the view's
//! own cookies (review findings H1-H3 of Charge 6).

use std::sync::Arc;

use rb_plugin_types::codes;
use serde_json::{Value, json};

use crate::PluginContext;
use crate::manifest::Grants;
use crate::test_support::ScriptedServices;

fn plugin(domains: &str, services: &Arc<ScriptedServices>) -> PluginContext {
    let table = format!("cdp = {{ domains = [{domains}] }}");
    let grants = Grants::parse(&toml::from_str(&table).expect("toml")).expect("grants");
    let services: Arc<dyn crate::HostServices> = services.clone();
    PluginContext::new("sample", "1.0.0", grants, services)
}

/// A service whose main frame is `F1` and whose `rbplugin-sample` world is context 7.
fn services() -> Arc<ScriptedServices> {
    let services = Arc::new(ScriptedServices::default());
    services.cdp_answers.lock().expect("answers").extend([
        (
            "Page.getFrameTree".to_owned(),
            json!({ "frameTree": { "frame": { "id": "F1" } } }).to_string(),
        ),
        (
            "Page.createIsolatedWorld".to_owned(),
            json!({ "executionContextId": 7 }).to_string(),
        ),
    ]);
    services.urls.lock().expect("urls").extend([
        (1, "https://www.example.com/shop?x=1".to_owned()),
        (2, "about:blank".to_owned()),
    ]);
    services
}

fn sent(services: &ScriptedServices) -> Vec<(String, Value)> {
    services
        .cdp_sent
        .lock()
        .expect("sent")
        .iter()
        .map(|(_, method, params)| {
            (
                method.clone(),
                serde_json::from_str(params).unwrap_or(Value::Null),
            )
        })
        .collect()
}

fn reason(failure: &rb_plugin_types::Failure) -> &str {
    assert_eq!(failure.code(), Some(codes::CDP_DOMAIN_DENIED));
    failure.param("reason").unwrap_or_default()
}

#[tokio::test]
async fn a_method_off_the_allowlist_is_refused_even_in_a_listed_domain() {
    let services = services();
    let context = plugin("\"Page\", \"Runtime\", \"DOM\", \"Network\"", &services);
    for method in [
        "Page.navigate",
        "Page.close",
        "Page.disable",
        "Page.removeScriptToEvaluateOnNewDocument",
        "Runtime.addBinding",
        "Runtime.callFunctionOn",
        "Runtime.disable",
        "DOM.setFileInputFiles",
        "DOM.disable",
        "Network.getAllCookies",
        "Network.disable",
    ] {
        let refused = context.cdp_send(1, method, "{}").await.expect_err(method);
        assert_eq!(reason(&refused), "method", "{method}");
        assert_eq!(refused.param("method"), Some(method), "{method}");
    }
    assert!(sent(&services).is_empty());
    let answer = context.cdp_send(1, "DOM.getDocument", "{}").await;
    assert_eq!(answer.expect("listed"), "{}");
}

#[tokio::test]
async fn a_domain_the_manifest_does_not_list_stays_refused() {
    let services = services();
    let context = plugin("\"DOM\"", &services);
    let refused = context
        .cdp_send(1, "CSS.enable", "{}")
        .await
        .expect_err("not in the manifest");
    assert_eq!(refused.code(), Some(codes::CDP_DOMAIN_DENIED));
    assert!(sent(&services).is_empty());
}

#[tokio::test]
async fn the_main_world_is_reached_without_a_context() {
    let services = services();
    let context = plugin("\"Runtime\"", &services);
    let params = json!({ "expression": "1", "includeCommandLineAPI": true }).to_string();
    context
        .cdp_send(1, "Runtime.evaluate", &params)
        .await
        .expect("main world");
    assert_eq!(sent(&services).len(), 1);
}

#[tokio::test]
async fn the_plugins_own_world_is_made_and_evaluated_in() {
    // The snippets plugin's path: its frame, its world, its context.
    let services = services();
    let context = plugin("\"Runtime\", \"Page\"", &services);
    let tree = context
        .cdp_send(1, "Page.getFrameTree", "{}")
        .await
        .expect("tree");
    assert!(tree.contains("F1"));
    let world =
        json!({ "frameId": "F1", "worldName": "rbplugin-sample", "grantUniveralAccess": false });
    let made = context
        .cdp_send(1, "Page.createIsolatedWorld", &world.to_string())
        .await
        .expect("own world");
    assert!(made.contains('7'));
    let params = json!({ "expression": "1", "contextId": 7 }).to_string();
    context
        .cdp_send(1, "Runtime.evaluate", &params)
        .await
        .expect("own context");
    let sent = sent(&services);
    let last = sent.last().expect("evaluate sent");
    assert_eq!(last.0, "Runtime.evaluate");
    assert_eq!(last.1["contextId"], 7);
    // The host asked for the plugin's own world itself before letting the context through.
    let checked = &sent[sent.len() - 2];
    assert_eq!(checked.0, "Page.createIsolatedWorld");
    assert_eq!(checked.1["worldName"], "rbplugin-sample");
    assert_eq!(checked.1["frameId"], "F1");
    assert_eq!(checked.1["grantUniveralAccess"], false);
}

#[tokio::test]
async fn another_context_is_refused_and_never_reached() {
    let services = services();
    let context = plugin("\"Runtime\", \"Page\"", &services);
    for id in [1, 2, 3, 8] {
        let params = json!({ "expression": "1", "contextId": id }).to_string();
        let refused = context
            .cdp_send(1, "Runtime.evaluate", &params)
            .await
            .expect_err("not the plugin's world");
        assert_eq!(reason(&refused), "context", "{id}");
    }
    assert!(
        sent(&services)
            .iter()
            .all(|(method, _)| method != "Runtime.evaluate")
    );
    for world in ["rbrowser", "rbplugin-other"] {
        let params = json!({ "frameId": "F1", "worldName": world }).to_string();
        let refused = context
            .cdp_send(1, "Page.createIsolatedWorld", &params)
            .await
            .expect_err("a foreign world");
        assert_eq!(reason(&refused), "world", "{world}");
    }
}

#[tokio::test]
async fn cookies_stay_with_the_views_own_site() {
    let services = services();
    let context = plugin("\"Network\"", &services);
    let own = json!({ "urls": ["https://www.example.com/shop?x=1"] }).to_string();
    context
        .cdp_send(1, "Network.getCookies", &own)
        .await
        .expect("own origin");
    let set = json!({ "name": "a", "value": "b", "domain": ".example.com" }).to_string();
    context
        .cdp_send(1, "Network.setCookie", &set)
        .await
        .expect("parent domain");
    let delete = json!({ "name": "a", "url": "https://www.example.com/" }).to_string();
    context
        .cdp_send(1, "Network.deleteCookies", &delete)
        .await
        .expect("own host");
    assert_eq!(sent(&services).len(), 3);

    for (view, method, params) in [
        (
            1,
            "Network.getCookies",
            json!({ "urls": ["https://bank.example/"] }),
        ),
        (
            1,
            "Network.getCookies",
            json!({ "urls": ["http://www.example.com/"] }),
        ),
        (
            1,
            "Network.setCookie",
            json!({ "name": "a", "value": "b", "domain": "bank.example" }),
        ),
        (
            1,
            "Network.deleteCookies",
            json!({ "name": "a", "url": "https://bank.example/" }),
        ),
        (
            2,
            "Network.getCookies",
            json!({ "urls": ["https://www.example.com/"] }),
        ),
        (
            9,
            "Network.getCookies",
            json!({ "urls": ["https://www.example.com/"] }),
        ),
    ] {
        let refused = context
            .cdp_send(view, method, &params.to_string())
            .await
            .expect_err("another site");
        assert_eq!(reason(&refused), "origin", "{view} {method} {params}");
    }
    assert_eq!(sent(&services).len(), 3);
}

#[tokio::test]
async fn a_response_body_comes_from_the_views_own_session() {
    let services = services();
    let context = plugin("\"Network\"", &services);
    let params = json!({ "requestId": "1000.1" }).to_string();
    context
        .cdp_send(1, "Network.getResponseBody", &params)
        .await
        .expect("by request id");
    let sent = services.cdp_sent.lock().expect("sent").clone();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].0, 1, "sent to the view asked for, nowhere else");
}

fn screenshot_plugin(window_image: bool, services: &Arc<ScriptedServices>) -> PluginContext {
    let table = format!("cdp = {{ domains = [\"Page\"], window_image = {window_image} }}");
    let grants = Grants::parse(&toml::from_str(&table).expect("toml")).expect("grants");
    let services: Arc<dyn crate::HostServices> = services.clone();
    PluginContext::new("sample", "1.0.0", grants, services)
}

#[tokio::test]
async fn the_window_image_needs_its_own_grant() {
    // Review S2 of RB-080-64: a plugin with the Page domain alone stays at small clips.
    let window = r#"{"format":"png","captureBeyondViewport":false,"fromSurface":false}"#;
    let services = services();
    let refused = screenshot_plugin(false, &services)
        .cdp_send(1, "Page.captureScreenshot", window)
        .await
        .expect_err("no grant");
    assert_eq!(reason(&refused), "window_image");
    assert!(sent(&services).is_empty());
    screenshot_plugin(true, &services)
        .cdp_send(1, "Page.captureScreenshot", window)
        .await
        .expect("granted");
    assert_eq!(sent(&services).len(), 1);
}

#[tokio::test]
async fn screenshots_are_spread_to_their_rate() {
    // Review S2: every screenshot takes a slot of SCREENSHOTS_PER_SECOND; the next one waits.
    let services = services();
    let plugin = screenshot_plugin(true, &services);
    let clip = r#"{"clip":{"x":0,"y":0,"width":10,"height":10}}"#;
    plugin
        .cdp_send(1, "Page.captureScreenshot", clip)
        .await
        .expect("shot");
    let next = plugin.screenshots.reserve();
    assert!(next > std::time::Duration::ZERO, "{next:?}");
    assert!(
        next <= std::time::Duration::from_secs(1) / crate::context::SCREENSHOTS_PER_SECOND,
        "{next:?}"
    );
    // Other methods take no slot.
    let fresh = screenshot_plugin(true, &services);
    fresh
        .cdp_send(1, "Page.getFrameTree", "{}")
        .await
        .expect("frame tree");
    assert_eq!(fresh.screenshots.reserve(), std::time::Duration::ZERO);
}
