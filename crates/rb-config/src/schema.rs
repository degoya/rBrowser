//! Generating the JSON schemas from the types: draft 2020-12, every subschema inlined (each key
//! sits where it is used, so the settings window and the annotations need no `$ref` resolution),
//! annotated by [`crate::annotate`] (the settings keys, `docs/settings.md`).
//!
//! The checked-in files are [`schema_files`]; `tests/schema_files.rs` fails when they are stale
//! and rewrites them with `RB_UPDATE_SCHEMA=1`.

use schemars::generate::SchemaSettings;
use schemars::{JsonSchema, Schema};
use serde_json::Value;

use crate::annotate::{OrderCounters, SchemaKind, annotate};
use crate::jsonio::to_json_string;
use crate::{ProjectConfig, UserSettings};

/// The project config schema, relative to the repository root.
pub const PROJECT_SCHEMA_FILE: &str = "schema/config.v1.json";
/// The user settings schema, relative to the repository root.
pub const USER_SETTINGS_SCHEMA_FILE: &str = "schema/settings.v1.json";

/// Both annotated schemas: (project config, user settings). They are built together because
/// `x-order` counts across both.
pub fn schemas() -> (Schema, Schema) {
    let mut counters = OrderCounters::new();
    let project = annotated::<ProjectConfig>(SchemaKind::Project, &mut counters);
    let user = annotated::<UserSettings>(SchemaKind::User, &mut counters);
    (project, user)
}

fn annotated<T: JsonSchema>(kind: SchemaKind, counters: &mut OrderCounters) -> Schema {
    let schema = SchemaSettings::draft2020_12()
        .with(|settings| settings.inline_subschemas = true)
        .into_generator()
        .into_root_schema_for::<T>();
    let mut value = Value::from(schema);
    if let Err(missing) = annotate(&mut value, kind, counters) {
        // The key tables name fields of the types; `annotate_tests` holds them to each other.
        panic!("settings keys without a field in the schema: {missing:?}");
    }
    Schema::try_from(value).expect("an annotated object schema is still a schema")
}

/// The checked-in schema files: (path relative to the repository root, canonical text).
pub fn schema_files() -> Vec<(&'static str, String)> {
    let (project, user) = schemas();
    vec![
        (PROJECT_SCHEMA_FILE, to_json_string(project.as_value())),
        (USER_SETTINGS_SCHEMA_FILE, to_json_string(user.as_value())),
    ]
}

#[cfg(test)]
#[path = "schema_tests.rs"]
mod tests;
