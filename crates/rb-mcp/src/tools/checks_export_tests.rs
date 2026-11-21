use std::sync::Arc;

use rmcp::handler::server::wrapper::Parameters;
use serde_json::json;

use super::{ChecksExportParams, ExportFormat};
use crate::testing::{FakeBackend, server};

#[tokio::test]
async fn checks_export_runs_the_export_command_with_format_and_path() {
    let fake = Arc::new(FakeBackend::default());
    let params = ChecksExportParams {
        format: Some(ExportFormat::Junit),
        path: Some("ci/checks.xml".to_owned()),
    };
    server(&fake)
        .checks_export(Parameters(params))
        .await
        .expect("answer");
    assert_eq!(
        fake.calls(),
        vec![(
            "checks.export".to_owned(),
            json!({ "format": "junit", "path": "ci/checks.xml" })
        )]
    );
}

#[tokio::test]
async fn without_arguments_the_settings_and_the_export_folder_decide() {
    let fake = Arc::new(FakeBackend::default());
    server(&fake)
        .checks_export(Parameters(ChecksExportParams::default()))
        .await
        .expect("answer");
    assert_eq!(fake.calls(), vec![("checks.export".to_owned(), json!({}))]);
}

#[test]
fn unknown_fields_and_formats_are_refused() {
    assert!(serde_json::from_value::<ChecksExportParams>(json!({ "target": "all" })).is_err());
    assert!(serde_json::from_value::<ChecksExportParams>(json!({ "format": "pdf" })).is_err());
    let parsed: ChecksExportParams =
        serde_json::from_value(json!({ "format": "sarif" })).expect("parses");
    assert_eq!(parsed.format, Some(ExportFormat::Sarif));
}
