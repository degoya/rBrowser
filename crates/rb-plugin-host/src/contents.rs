//! The checks of a package's text members: the locale files and the settings schema.
//!
//! Locales (`docs/i18n.md`, "Plugin locales"): the catalogue's nested shape, every text a string,
//! every key below `plugin.<id>.` and well formed - so a plugin can never override a core key or
//! another plugin's. `rb_i18n::register_plugin_locale` checks the same when the host hands the
//! file over ([`crate::LocaleSink`]); this check refuses the package before it is installed.
//!
//! Settings schema (`docs/settings.md`, "Rules for plugins"): a JSON schema object whose every
//! key carries `x-category` of an existing category, `x-section` of that category, an integer
//! `x-order` and its `x-scope` (the rules of RB-010-02's settings lint). **A plugin cannot create
//! a top-level category**: an unknown `x-category` refuses the package
//! (`plugins.error.settings_category`). The definitions themselves are RB-010-02's
//! `plugin_defs_from_schema` (keys `<plugin-id>.<property>`), fed through [`crate::SettingsSink`].

use anyhow::{Context, Result, bail};
use rb_plugin_types::codes;
use rb_settings::{Category, X_CATEGORY, X_ORDER, X_SCOPE, X_SECTION, section};
use serde_json::Value;

use crate::refusal::refuse;

/// The most keys one locale file or settings schema may hold.
const MAX_KEYS: usize = 4_096;
/// How deep a settings schema may nest objects.
const MAX_DEPTH: usize = 8;

/// Checks a locale file of plugin `plugin_id`; answers how many texts it holds.
///
/// # Errors
///
/// No JSON object, a non-string text, a key outside `plugin.<plugin_id>.` or malformed.
pub fn check_locale(plugin_id: &str, bytes: &[u8]) -> Result<usize> {
    let value: Value = serde_json::from_slice(bytes).context("a locale file is no JSON")?;
    let mut keys = Vec::new();
    flatten(&value, String::new(), &mut keys)?;
    let prefix = format!("plugin.{plugin_id}.");
    for key in &keys {
        if !key.starts_with(&prefix) || !rb_i18n::is_well_formed_key(key) {
            bail!("locale key `{key}` must be a well-formed key below `{prefix}`");
        }
    }
    if keys.len() > MAX_KEYS {
        bail!("a locale file holds more than {MAX_KEYS} texts");
    }
    Ok(keys.len())
}

fn flatten(value: &Value, path: String, keys: &mut Vec<String>) -> Result<()> {
    match value {
        Value::Object(map) => {
            for (segment, child) in map {
                let child_path = if path.is_empty() {
                    segment.clone()
                } else {
                    format!("{path}.{segment}")
                };
                flatten(child, child_path, keys)?;
                if keys.len() > MAX_KEYS {
                    bail!("a locale file holds more than {MAX_KEYS} texts");
                }
            }
            Ok(())
        }
        Value::String(text) if !text.is_empty() && !path.is_empty() => {
            keys.push(path);
            Ok(())
        }
        _ => bail!("locale key `{path}` is no text"),
    }
}

/// Checks the settings schema of a plugin and answers it parsed, for
/// [`crate::SettingsSink::register_plugin_schema`].
///
/// # Errors
///
/// No JSON schema object; a key without its annotations (`plugins.error.settings_annotation`), of
/// an unknown category (`plugins.error.settings_category`) or section
/// (`plugins.error.settings_section`).
pub fn check_settings_schema(bytes: &[u8]) -> Result<Value> {
    let schema: Value = serde_json::from_slice(bytes).context("settings.schema.json is no JSON")?;
    let properties = schema
        .get("properties")
        .and_then(Value::as_object)
        .context("settings.schema.json needs `properties`")?;
    let mut count = 0;
    for (key, property) in properties {
        check_property(key, property, 0, &mut count)?;
    }
    Ok(schema)
}

fn check_property(key: &str, property: &Value, depth: usize, count: &mut usize) -> Result<()> {
    *count += 1;
    if depth > MAX_DEPTH || *count > MAX_KEYS {
        bail!("settings.schema.json nests or holds too much");
    }
    // An object with its own properties is a group; its leaves carry the annotations.
    if let Some(children) = property.get("properties").and_then(Value::as_object) {
        for (child, value) in children {
            check_property(&format!("{key}.{child}"), value, depth + 1, count)?;
        }
        return Ok(());
    }
    let annotation = |name: &str| {
        property.get(name).ok_or_else(|| {
            refuse(
                codes::SETTINGS_ANNOTATION,
                format!("settings key `{key}` has no `{name}`"),
            )
        })
    };
    let category_id = annotation(X_CATEGORY)?.as_str().unwrap_or_default();
    let Some(category) = Category::from_id(category_id) else {
        return Err(refuse(
            codes::SETTINGS_CATEGORY,
            format!(
                "settings key `{key}` names the category `{category_id}`, which rBrowser does not \
                 have; a plugin cannot add a top-level category"
            ),
        ));
    };
    let section_id = annotation(X_SECTION)?.as_str().unwrap_or_default();
    if section(category, section_id).is_none() {
        return Err(refuse(
            codes::SETTINGS_SECTION,
            format!(
                "settings key `{key}` names the section `{section_id}`, which `{category_id}` does not have"
            ),
        ));
    }
    if !annotation(X_ORDER)?.is_u64() {
        return Err(refuse(
            codes::SETTINGS_ANNOTATION,
            format!("settings key `{key}` needs an integer `{X_ORDER}`"),
        ));
    }
    if !annotation(X_SCOPE)?.is_array() {
        return Err(refuse(
            codes::SETTINGS_ANNOTATION,
            format!("settings key `{key}` needs `{X_SCOPE}`, a list of scopes"),
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "contents_tests.rs"]
mod tests;
