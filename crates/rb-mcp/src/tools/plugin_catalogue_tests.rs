use std::sync::Arc;

use rmcp::handler::server::wrapper::Parameters;
use serde_json::json;

use super::CatalogParams;
use crate::testing::{FakeBackend, server};

#[tokio::test]
async fn the_catalogue_is_read_through_its_command_with_named_arguments() {
    let fake = Arc::new(FakeBackend::default());
    let server = server(&fake);
    server
        .plugins_catalog_list(Parameters(CatalogParams::default()))
        .await
        .expect("answer");
    server
        .plugins_catalog_list(Parameters(CatalogParams {
            refresh: Some(true),
        }))
        .await
        .expect("answer");
    assert_eq!(
        fake.calls(),
        vec![
            ("plugins.catalog".to_owned(), json!({})),
            ("plugins.catalog".to_owned(), json!({ "refresh": true })),
        ]
    );
}

#[test]
fn the_tool_takes_no_install_or_repository_argument() {
    let refused = serde_json::from_value::<CatalogParams>(json!({
        "install": { "id": "seo" },
        "repository": "https://elsewhere.test/rbrowser-plugin-index.json",
    }));
    assert!(refused.is_err());
}
