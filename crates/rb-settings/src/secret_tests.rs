use serde_json::json;

use super::*;
use crate::Category;
use crate::test_support::def_with_schema;

#[test]
fn a_key_is_secret_only_with_the_annotation_true() {
    let secret = def_with_schema(
        "p.vault",
        Category::Network,
        "api-client",
        1,
        json!({ "type": "object", "x-secret": true }),
    );
    assert!(secret.is_secret());
    let open = def_with_schema(
        "p.vault",
        Category::Network,
        "api-client",
        1,
        json!({ "type": "object", "x-secret": "yes" }),
    );
    assert!(!open.is_secret());
    let plain = def_with_schema("p.vault", Category::Network, "api-client", 1, json!({}));
    assert!(!plain.is_secret());
}

/// The names stay, every value goes: an environment's secret names are no secret, their values
/// are.
#[test]
fn the_mask_keeps_the_names_and_hides_every_value() {
    let value = json!({ "staging": { "TOKEN": "s3cr3t", "PORT": 8080 }, "list": ["a", true] });
    assert_eq!(
        mask_secret(&value),
        json!({ "staging": { "TOKEN": "***", "PORT": "***" }, "list": ["***", "***"] })
    );
    assert_eq!(mask_secret(&json!(null)), json!(null));
    assert_eq!(mask_secret(&json!({})), json!({}));
}

/// RB-094-27: a secret key is no setting of the window; its `x-panel` names the panel where its
/// values are edited, and the tab - the API client's Variables.
#[test]
fn a_secret_key_names_the_panel_that_edits_it() {
    let panel = json!({ "panel": "api-client", "tab": "variables" });
    let secret = def_with_schema(
        "p.vault",
        Category::Network,
        "api-client",
        1,
        json!({ "type": "object", "x-secret": true, "x-panel": panel }),
    );
    assert_eq!(
        secret.editor_panel(),
        Some(EditorPanel {
            panel: "api-client",
            tab: Some("variables")
        })
    );
    let no_tab = def_with_schema(
        "p.vault",
        Category::Network,
        "api-client",
        1,
        json!({ "x-secret": true, "x-panel": { "panel": "vault" } }),
    );
    assert_eq!(
        no_tab.editor_panel(),
        Some(EditorPanel {
            panel: "vault",
            tab: None
        })
    );
    // Only a secret key names one, and only a well-formed id.
    for schema in [
        json!({ "x-panel": { "panel": "api-client" } }),
        json!({ "x-secret": true }),
        json!({ "x-secret": true, "x-panel": "api-client" }),
        json!({ "x-secret": true, "x-panel": { "panel": "Api Client" } }),
        json!({ "x-secret": true, "x-panel": { "panel": "api-client", "tab": "../x" } }),
        json!({ "x-secret": true, "x-panel": { "panel": "" } }),
    ] {
        let def = def_with_schema(
            "p.vault",
            Category::Network,
            "api-client",
            1,
            schema.clone(),
        );
        assert_eq!(def.editor_panel(), None, "{schema}");
    }
}
