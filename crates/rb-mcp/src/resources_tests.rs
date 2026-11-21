use std::sync::Arc;

use base64::Engine as _;
use serde_json::{Value, json};

use super::{
    CONFIG, DISABLED, FINDINGS, NO_FINDINGS, NO_SCREENSHOTS, NOT_FOUND, SCREENSHOT_TEMPLATE,
    SCREENSHOTS, SETTING, TOO_LARGE, VIEWS, list, templates, with_uris,
};
use crate::backend::{McpOptions, ToolFailure};
use crate::testing::{FakeBackend, FakeLatest, SECRET_MARK, answer_json, server};

fn uris(resources: &[rmcp::model::Resource]) -> Vec<&str> {
    resources
        .iter()
        .map(|resource| resource.uri.as_str())
        .collect()
}

/// The one content of `uri` as JSON on the wire: `uri`, `mimeType`, `text` or `blob`.
async fn read(fake: &Arc<FakeBackend>, uri: &str) -> Result<Value, ToolFailure> {
    let result = server(fake).resource_read(uri).await?;
    let wire = serde_json::to_value(&result).expect("JSON");
    assert_eq!(wire["contents"].as_array().map(Vec::len), Some(1), "{wire}");
    assert_eq!(wire["contents"][0]["uri"], uri);
    Ok(wire["contents"][0].clone())
}

/// The JSON text of `uri`.
async fn read_json(fake: &Arc<FakeBackend>, uri: &str) -> Value {
    let content = read(fake, uri).await.expect("readable");
    assert_eq!(content["mimeType"], "application/json");
    serde_json::from_str(content["text"].as_str().expect("text")).expect("JSON text")
}

async fn refusal(fake: &Arc<FakeBackend>, uri: &str) -> ToolFailure {
    read(fake, uri).await.expect_err("refused")
}

#[test]
fn four_resources_and_the_screenshot_template_are_listed() {
    assert_eq!(uris(&list()), [VIEWS, FINDINGS, SCREENSHOTS, CONFIG]);
    for resource in list() {
        assert_eq!(resource.mime_type.as_deref(), Some("application/json"));
        assert!(resource.description.is_some_and(|text| text.len() > 20));
    }
    let templates = templates();
    assert_eq!(templates.len(), 1);
    assert_eq!(templates[0].uri_template, SCREENSHOT_TEMPLATE);
    assert_eq!(templates[0].mime_type.as_deref(), Some("image/png"));
}

#[tokio::test]
async fn views_answers_what_views_list_answers() {
    let fake = Arc::new(FakeBackend::default());
    let (error, tool) = answer_json(&server(&fake).views_list().await.expect("answer"));
    assert!(!error);
    assert_eq!(read_json(&fake, VIEWS).await, tool);
    assert_eq!(tool.as_array().map(Vec::len), Some(2));
}

#[tokio::test]
async fn config_leaves_out_every_protected_key() {
    let fake = Arc::new(FakeBackend::default());
    fake.set_latest(FakeLatest {
        config: Some(json!({
            "version": 1,
            "startUrl": "https://a.test/",
            "breakpoints": [{ "id": "mobile", "width": 375 }],
            "emulation": { "locale": "de-DE", "headers": { "X-Token": "t" } },
            "network": { "proxy": "http://proxy.test:8080" },
            "capture": { "dir": "C:/shots", "hideBanners": true },
        })),
        ..FakeLatest::default()
    });
    let config = read_json(&fake, CONFIG).await;
    for path in crate::protected_keys::paths() {
        let pointer = format!("/{}", path.replace('.', "/"));
        assert!(config.pointer(&pointer).is_none(), "{path} in {config}");
    }
    assert_eq!(config["startUrl"], "https://a.test/");
    assert_eq!(config["emulation"]["locale"], "de-DE");
    assert_eq!(config["capture"]["hideBanners"], true);
}

#[tokio::test]
async fn with_mcp_resources_off_none_is_listed_or_read() {
    let fake = Arc::new(FakeBackend::default());
    let server = server(&fake);
    assert_eq!(server.resource_list().len(), 4);
    assert_eq!(server.resource_templates().len(), 1);
    fake.set_options(McpOptions {
        resources: false,
        ..McpOptions::default()
    });
    assert!(server.resource_list().is_empty());
    assert!(server.resource_templates().is_empty());
    for uri in [VIEWS, CONFIG, "rbrowser://screenshots/latest/0"] {
        let refused = refusal(&fake, uri).await;
        assert_eq!(refused.code, DISABLED, "{uri}");
        assert_eq!(refused.settings_key.as_deref(), Some(SETTING));
    }
}

#[tokio::test]
async fn findings_are_the_last_run_masked() {
    let fake = Arc::new(FakeBackend::default());
    assert_eq!(refusal(&fake, FINDINGS).await.code, NO_FINDINGS);
    fake.set_latest(FakeLatest {
        findings: Some(json!({
            "run": { "runId": "run-1", "url": format!("https://a.test/?token={SECRET_MARK}") },
            "messages": {},
        })),
        ..FakeLatest::default()
    });
    let findings = read_json(&fake, FINDINGS).await;
    assert_eq!(findings["run"]["runId"], "run-1");
    assert_eq!(findings["run"]["url"], "***");
}

fn shots() -> Value {
    json!({
        "folder": "/site/.rbrowser/out/screenshots",
        "shots": [
            { "view": 1, "label": "mobile", "width": 375, "height": 812, "path": "/s/mobile.png" },
            { "view": 2, "label": "desktop", "width": 0, "height": 0, "error": "crashed" },
            { "view": 3, "label": "tablet", "width": 768, "height": 1024,
              "path": "/s/tablet.png", "pngBase64": "AAAA" },
        ],
        "overview": { "label": "overview", "width": 1200, "height": 900, "path": "/s/all.png" },
    })
}

#[test]
fn each_file_gets_its_uri_in_order_and_no_image_data() {
    let listed = with_uris(shots());
    assert_eq!(listed["shots"][0]["uri"], "rbrowser://screenshots/latest/0");
    assert!(listed["shots"][1].get("uri").is_none(), "no file, no uri");
    assert_eq!(listed["shots"][2]["uri"], "rbrowser://screenshots/latest/1");
    assert!(listed["shots"][2].get("pngBase64").is_none());
    assert_eq!(listed["overview"]["uri"], "rbrowser://screenshots/latest/2");
}

#[tokio::test]
async fn a_screenshot_is_a_blob_only_when_asked_for() {
    let fake = Arc::new(FakeBackend::default());
    assert_eq!(refusal(&fake, SCREENSHOTS).await.code, NO_SCREENSHOTS);
    let png = vec![0x89, b'P', b'N', b'G', 1, 2, 3];
    fake.set_latest(FakeLatest {
        screenshots: Some(shots()),
        pngs: vec![png.clone(), vec![4], vec![5]],
        ..FakeLatest::default()
    });
    let listed = read_json(&fake, SCREENSHOTS).await;
    assert!(!listed.to_string().contains("AAAA"), "{listed}");
    assert_eq!(listed["shots"][0]["path"], "/s/mobile.png");
    let content = read(&fake, "rbrowser://screenshots/latest/0")
        .await
        .expect("the PNG");
    assert_eq!(content["mimeType"], "image/png");
    let blob = content["blob"].as_str().expect("a blob");
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(blob)
        .expect("base64");
    assert_eq!(decoded, png);
    assert_eq!(
        refusal(&fake, "rbrowser://screenshots/latest/7").await.code,
        NO_SCREENSHOTS
    );
}

#[tokio::test]
async fn an_answer_over_mcp_max_answer_bytes_is_refused() {
    let fake = Arc::new(FakeBackend::default());
    fake.set_latest(FakeLatest {
        screenshots: Some(shots()),
        pngs: vec![vec![0; 6000]],
        findings: Some(json!({ "run": { "notes": "x".repeat(6000) } })),
        ..FakeLatest::default()
    });
    fake.set_options(McpOptions {
        max_answer_bytes: 4096,
        ..McpOptions::default()
    });
    for uri in [FINDINGS, "rbrowser://screenshots/latest/0"] {
        let refused = refusal(&fake, uri).await;
        assert_eq!(refused.code, TOO_LARGE, "{uri}");
        assert_eq!(refused.settings_key.as_deref(), Some("mcp.maxAnswerBytes"));
    }
    assert!(read(&fake, VIEWS).await.is_ok());
}

#[tokio::test]
async fn an_unknown_uri_is_not_found() {
    let fake = Arc::new(FakeBackend::default());
    for uri in [
        "rbrowser://nope",
        "rbrowser://views/",
        "rbrowser://screenshots/latest/",
        "rbrowser://screenshots/latest/-1",
        "rbrowser://screenshots/latest/1x",
        "rbrowser://screenshots/latest/99999",
        "rbrowser://screenshots/latest/../../config",
        "file:///etc/passwd",
        "RBROWSER://VIEWS",
    ] {
        assert_eq!(refusal(&fake, uri).await.code, NOT_FOUND, "{uri}");
    }
}
