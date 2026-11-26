use std::sync::Arc;

use rmcp::handler::server::wrapper::Parameters;
use serde_json::json;

use super::{KeyParams, OpenParams, SetParams, SettingsParams};
use crate::backend::SettingScope;
use crate::testing::{FakeBackend, answer_json, server};

#[tokio::test]
async fn config_get_answers_all_or_one_key() {
    let fake = Arc::new(FakeBackend::default());
    let server = server(&fake);
    let (_, all) = answer_json(
        &server
            .config_get(Parameters(KeyParams::default()))
            .await
            .expect("answer"),
    );
    assert_eq!(all["version"], 1);
    let (error, one) = answer_json(
        &server
            .config_get(Parameters(KeyParams {
                key: Some("startUrl".to_owned()),
            }))
            .await
            .expect("answer"),
    );
    assert!(!error);
    assert_eq!(one, "https://a.test/");
    let (error, missing) = answer_json(
        &server
            .config_get(Parameters(KeyParams {
                key: Some("layout.nope".to_owned()),
            }))
            .await
            .expect("answer"),
    );
    assert!(error);
    assert_eq!(missing["code"], "settings.key_not_found");
}

#[tokio::test]
async fn settings_set_writes_and_settings_get_reads_it_back() {
    let fake = Arc::new(FakeBackend::default());
    let server = server(&fake);
    let (error, _) = answer_json(
        &server
            .settings_set(Parameters(SetParams {
                key: "grid.fitToWindow".to_owned(),
                value: json!(false),
                scope: SettingScope::User,
            }))
            .await
            .expect("answer"),
    );
    assert!(!error);
    let (_, one) = answer_json(
        &server
            .settings_get(Parameters(SettingsParams {
                key: Some("grid.fitToWindow".to_owned()),
                ..SettingsParams::default()
            }))
            .await
            .expect("answer"),
    );
    assert_eq!(one["value"], false);
    assert_eq!(one["origin"], "user");
    assert_eq!(one["deepLink"], "rbrowser://settings/grid.fitToWindow");
}

/// The answer of `settings_set` is the new value, its origin, the scope and the deep link.
#[tokio::test]
async fn settings_set_answers_the_new_value_and_where_it_is_stored() {
    let fake = Arc::new(FakeBackend::default());
    let (error, answer) = answer_json(
        &server(&fake)
            .settings_set(Parameters(SetParams {
                key: "grid.fitToWindow".to_owned(),
                value: json!(false),
                scope: SettingScope::User,
            }))
            .await
            .expect("answer"),
    );
    assert!(!error, "{answer}");
    assert_eq!(answer["key"], "grid.fitToWindow");
    assert_eq!(answer["value"], false);
    assert_eq!(answer["origin"], "user");
    assert_eq!(answer["scope"], "user");
    assert_eq!(answer["deepLink"], "rbrowser://settings/grid.fitToWindow");
}

/// Without arguments the structure: categories, sections, keys; filters narrow it.
#[tokio::test]
async fn settings_get_answers_the_structure_and_filters_it() {
    let fake = Arc::new(FakeBackend::default());
    let server = server(&fake);
    let (error, all) = answer_json(
        &server
            .settings_get(Parameters(SettingsParams::default()))
            .await
            .expect("answer"),
    );
    assert!(!error, "{all}");
    let categories: Vec<&str> = all["categories"]
        .as_array()
        .expect("categories")
        .iter()
        .filter_map(|category| category["id"].as_str())
        .collect();
    assert_eq!(categories, ["general", "layout", "security"]);
    assert_eq!(all["count"], 3);
    let (_, found) = answer_json(
        &server
            .settings_get(Parameters(SettingsParams {
                query: Some("evaluate".to_owned()),
                ..SettingsParams::default()
            }))
            .await
            .expect("answer"),
    );
    assert_eq!(found["count"], 1);
    assert_eq!(
        found["categories"][0]["sections"][0]["keys"][0]["key"],
        "mcp.allowEvaluate"
    );
    let (error, unknown) = answer_json(
        &server
            .settings_get(Parameters(SettingsParams {
                category: Some("nope".to_owned()),
                ..SettingsParams::default()
            }))
            .await
            .expect("answer"),
    );
    assert!(error);
    assert_eq!(unknown["code"], "settings.category_not_found");
}

#[tokio::test]
async fn a_protected_key_is_refused_with_its_setting() {
    let fake = Arc::new(FakeBackend::default());
    let (error, answer) = answer_json(
        &server(&fake)
            .settings_set(Parameters(SetParams {
                key: "mcp.allowEvaluate".to_owned(),
                value: json!(true),
                scope: SettingScope::User,
            }))
            .await
            .expect("answer"),
    );
    assert!(error);
    assert_eq!(answer["code"], "settings.rejected.protected");
    assert_eq!(answer["settingsKey"], "mcp.allowEvaluate");
    assert!(fake.written().is_empty());
}

#[tokio::test]
async fn project_info_and_open_reach_the_backend() {
    let fake = Arc::new(FakeBackend::default());
    let server = server(&fake);
    let (_, info) = answer_json(
        &server
            .project_info(crate::tools::no_args())
            .await
            .expect("answer"),
    );
    assert_eq!(info["trusted"], false);
    server
        .project_open(Parameters(OpenParams {
            path: Some("/tmp/other".to_owned()),
            index: None,
        }))
        .await
        .expect("answer");
    assert_eq!(
        fake.calls(),
        vec![(
            "project.open_recent".to_owned(),
            json!({ "path": "/tmp/other" })
        )]
    );
}
