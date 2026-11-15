//! `schema/findings.v1.json` is generated from `rb_plugin_types::FindingsFile` (RB-060-01); this
//! test fails when it is stale. Regenerate with
//! `RB_UPDATE_SCHEMA=1 cargo nextest run -p rb-plugin-host --test findings_schema`.

use std::path::Path;

use rb_plugin_types::{
    CheckTarget, CheckedView, ElementRef, FINDINGS_SCHEMA_FILE, FINDINGS_VERSION, FindingMessage,
    FindingsFile, FindingsSummary, MergedFinding, Severity, findings_schema_text,
};
use serde_json::Value;

#[test]
fn the_checked_in_findings_schema_is_current() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(FINDINGS_SCHEMA_FILE);
    let text = findings_schema_text();
    if std::env::var_os("RB_UPDATE_SCHEMA").is_some_and(|value| value == "1") {
        std::fs::write(&path, &text).expect("write the schema");
    }
    assert_eq!(
        std::fs::read_to_string(&path).ok().as_deref(),
        Some(text.as_str()),
        "{FINDINGS_SCHEMA_FILE} is stale; regenerate with \
         RB_UPDATE_SCHEMA=1 cargo nextest run -p rb-plugin-host --test findings_schema"
    );
}

/// The required properties of every object schema hold in `value`, walking the schema's
/// `properties`, `items` and `$defs` references - enough to catch a renamed or dropped field.
fn holds(schema: &Value, defs: &Value, value: &Value) {
    let schema = match schema.get("$ref").and_then(Value::as_str) {
        Some(reference) => {
            let name = reference
                .rsplit('/')
                .next()
                .expect("a reference names a definition");
            &defs[name]
        }
        None => schema,
    };
    if let (Some(required), Some(object)) = (schema.get("required"), value.as_object()) {
        for name in required.as_array().expect("required is a list") {
            let name = name.as_str().expect("a property name");
            assert!(object.contains_key(name), "{name} is required");
        }
    }
    if let (Some(properties), Some(object)) = (schema.get("properties"), value.as_object()) {
        for (name, inner) in object {
            let property = &properties[name];
            assert!(!property.is_null(), "{name} is not in the schema");
            holds(property, defs, inner);
        }
    }
    if let (Some(items), Some(list)) = (schema.get("items"), value.as_array()) {
        for item in list {
            holds(items, defs, item);
        }
    }
}

#[test]
fn a_findings_file_fits_its_schema() {
    let finding = MergedFinding {
        id: "4ef7e8f2df278863".to_owned(),
        rule_id: "seo.images".to_owned(),
        plugins: vec!["seo".to_owned(), "a11y-axe".to_owned()],
        severity: Severity::Error,
        message: FindingMessage {
            key: "plugin.seo.findings.alt".to_owned(),
            params: [("count".to_owned(), "2".to_owned())].into(),
        },
        views: vec![1, 3],
        breakpoints: vec!["mobile".to_owned(), "desktop".to_owned()],
        url: Some("https://site.test/".to_owned()),
        element: Some(ElementRef {
            selector: Some("img.hero".to_owned()),
            path: Some("html>body>img".to_owned()),
        }),
        help_url: Some("https://help.test/alt".to_owned()),
        data: Some(serde_json::json!({ "width": 40 })),
        shared_id: Some("img-alt-missing".to_owned()),
    };
    let file = FindingsFile {
        version: FINDINGS_VERSION,
        run_id: "run-0000000000001".to_owned(),
        started_at: 1,
        target: CheckTarget::View(2),
        url: None,
        plugins: vec!["seo".to_owned()],
        views: vec![CheckedView {
            id: 1,
            breakpoint: Some("mobile".to_owned()),
        }],
        summary: FindingsSummary::of(std::slice::from_ref(&finding)),
        findings: vec![finding],
        failures: Vec::new(),
        skipped: Vec::new(),
    };
    let schema: Value = serde_json::from_str(&findings_schema_text()).expect("json");
    let value = serde_json::to_value(&file).expect("serializes");
    holds(&schema, &schema["$defs"], &value);
}
