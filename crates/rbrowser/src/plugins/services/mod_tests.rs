use std::time::Duration;

use rb_plugin_host::{Caller, HostServices};
use rb_plugin_types::{Finding, codes};
use serde_json::json;

use super::{AppServices, CALLER_KEY, local_origins, net::Origin, project_origins, with_caller};

#[test]
fn the_projects_origins_are_its_start_url_and_environments() {
    let config: rb_config::ProjectConfig = serde_json::from_value(json!({
        "version": 1,
        "startUrl": "http://localhost:3000/home",
        "environments": [
            { "id": "local", "url": "http://127.0.0.1:8080" },
            { "id": "staging", "url": "https://staging.site.test" },
        ],
    }))
    .expect("config");
    assert_eq!(
        project_origins(&config),
        vec![
            Origin::of("http://localhost:3000").expect("origin"),
            Origin::of("http://127.0.0.1:8080").expect("origin"),
            Origin::of("https://staging.site.test").expect("origin"),
        ]
    );
}

#[tokio::test]
async fn without_a_window_the_page_services_are_not_available() {
    let services = AppServices::new();
    let caller = Caller {
        plugin_id: "sample",
        timeout: Duration::from_secs(1),
    };
    assert!(services.views().is_empty());
    assert_eq!(services.active_view(), None);
    assert_eq!(services.project_root(), None);
    assert!(!services.workspace_trusted(), "no window, no trust");
    let failure = services
        .evaluate(&caller, 1, "1")
        .await
        .expect_err("no window");
    assert_eq!(failure.code(), Some(codes::NOT_AVAILABLE));
    let failure = services
        .cdp_send(&caller, 1, "DOM.getDocument", "{}")
        .await
        .expect_err("no window");
    assert_eq!(failure.code(), Some(codes::NOT_AVAILABLE));
    let failure = services
        .report_findings("sample", vec![Finding::default()])
        .expect_err("no window");
    assert_eq!(failure.code(), Some(codes::NOT_AVAILABLE));
}

#[test]
fn settings_answer_only_a_plugin_with_a_schema() {
    let services = AppServices::new();
    services.set_schemas(vec![(
        "seo".to_owned(),
        json!({ "type": "object", "properties": { "maxPages": {
            "type": "integer", "default": 10,
            "x-category": "checks", "x-section": "seo", "x-order": 1, "x-scope": ["user"]
        } } }),
    )]);
    assert!(services.setting("other", "maxPages").is_none());
    assert!(services.plugin_values("other").is_empty());
}

#[tokio::test]
async fn a_project_plugin_reaches_no_service_without_workspace_trust() {
    let services = AppServices::new();
    services.set_schemas(vec![(
        "shared".to_owned(),
        json!({ "type": "object", "properties": { "depth": {
            "type": "integer", "default": 2,
            "x-category": "checks", "x-section": "seo", "x-order": 1, "x-scope": ["user"]
        } } }),
    )]);
    services.set_project_plugins(["shared".to_owned()].into());
    let caller = Caller {
        plugin_id: "shared",
        timeout: Duration::from_secs(1),
    };
    let untrusted = Some(codes::UNTRUSTED_WORKSPACE);
    // No window, so no trust: refused before anything else is looked at.
    let failure = services
        .evaluate(&caller, 1, "1")
        .await
        .expect_err("stopped");
    assert_eq!(failure.code(), untrusted);
    let failure = services
        .cdp_send(&caller, 1, "DOM.getDocument", "{}")
        .await
        .expect_err("stopped");
    assert_eq!(failure.code(), untrusted);
    let failure = services
        .sidecar_call(&caller, "lighthouse.run", "{}")
        .await
        .expect_err("stopped");
    assert_eq!(failure.code(), untrusted);
    let failure = services
        .report_findings("shared", vec![Finding::default()])
        .expect_err("stopped");
    assert_eq!(failure.code(), untrusted);
    assert_eq!(services.setting("shared", "depth"), None);
}

#[test]
fn only_a_trusted_project_opens_its_own_origins_to_the_plugins() {
    // A cloned repository naming the metadata service or a local port as its environment opens
    // nothing before a person trusts it (review L2, Charge 6).
    let config: rb_config::ProjectConfig = serde_json::from_value(json!({
        "version": 1,
        "startUrl": "http://127.0.0.1:2375/",
        "environments": [{ "id": "cloud", "url": "http://169.254.169.254" }],
    }))
    .expect("config");
    assert!(local_origins(&config, false).is_empty());
    assert_eq!(local_origins(&config, true), project_origins(&config));
}

#[test]
fn a_sidecar_call_names_its_plugin_and_the_plugin_cannot_forge_it() {
    // The sidecar keeps each caller to its own Lighthouse jobs (review L4 of Charge 7).
    let params = with_caller(
        json!({ "job": "lh-1", "$caller": "lighthouse" }),
        "intruder",
    );
    assert_eq!(
        params,
        Some(json!({ "job": "lh-1", CALLER_KEY: "intruder" }))
    );
    assert_eq!(
        with_caller(json!({}), "lighthouse"),
        Some(json!({ "$caller": "lighthouse" }))
    );
}

#[test]
fn a_sidecar_call_without_an_object_is_refused() {
    // Without an object there is no `$caller`, and the sidecar would take the call for
    // rBrowser's own (review L4 of Charge 8).
    for params in [json!([1]), json!(null), json!("x"), json!(3)] {
        assert_eq!(with_caller(params.clone(), "lighthouse"), None, "{params}");
    }
}
