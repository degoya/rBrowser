//! Helpers the unit tests of this crate share.

use serde_json::{Value, json};

use crate::{Category, Scope, SettingDef};

/// A valid definition of `key` in `category` > `section` at `order`, writable to Project and
/// Local, with the default label keys.
pub(crate) fn def(key: &str, category: Category, section: &str, order: u32) -> SettingDef {
    SettingDef {
        key: key.into(),
        category,
        section: section.into(),
        order,
        scope_allowed: vec![Scope::Project, Scope::Local],
        local_only: false,
        protected_from_mcp: false,
        label_key: format!("settings.key.{key}.label"),
        description_key: format!("settings.key.{key}.description"),
        help_key: format!("settings.key.{key}.help"),
        schema: json!({ "type": "boolean", "default": true }),
    }
}

/// [`def`] with its own schema.
pub(crate) fn def_with_schema(
    key: &str,
    category: Category,
    section: &str,
    order: u32,
    schema: Value,
) -> SettingDef {
    SettingDef {
        schema,
        ..def(key, category, section, order)
    }
}

/// The `x-*` annotations of a key, as a schema fragment to extend.
pub(crate) fn annotated(category: &str, section: &str, order: u32, scope: &[&str]) -> Value {
    json!({
        "x-category": category,
        "x-section": section,
        "x-order": order,
        "x-scope": scope,
    })
}
