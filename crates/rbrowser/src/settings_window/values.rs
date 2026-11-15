//! What the settings window reads: every key's value and origin over the three files, and the
//! schema the window renders from (`getSettings { includeSchema }`).
//!
//! The origin is the file that holds the key - `config.local.json` before `config.json` before
//! the user settings - so the badge says where a value is stored, a trust-gated one included.
//! The value is the one in effect: the typed config and the user settings after loading, with
//! every default filled in; a key the trust gate strips shows what its file stores.

use std::collections::BTreeSet;
use std::path::PathBuf;

use rb_ipc::SettingValue;
use rb_settings::{Category, Origin, Scope, SettingDef, SettingsRegistry};
use serde_json::{Map, Value, json};

use super::docedit::lookup;

/// The version of the schema payload the window reads.
pub const SCHEMA_PAYLOAD_VERSION: u32 = 1;

/// The documents a value is resolved over.
#[derive(Clone, Debug, Default)]
pub struct Documents {
    /// The user settings in effect, serialized (every default filled in).
    pub user: Value,
    /// The user settings file as stored; `None` when there is none or it cannot be read.
    pub user_file: Option<Value>,
    /// The open project; `None` without one.
    pub project: Option<ProjectDocuments>,
}

/// The project's side of [`Documents`].
#[derive(Clone, Debug, Default)]
pub struct ProjectDocuments {
    /// The project root.
    pub root: PathBuf,
    /// The config in effect, serialized (every default filled in).
    pub effective: Value,
    /// `.rbrowser/config.json` as stored.
    pub project_file: Value,
    /// `.rbrowser/config.local.json` as stored; `None` when there is none.
    pub local_file: Option<Value>,
}

impl Documents {
    /// The file document of `scope`, as stored.
    pub fn file(&self, scope: Scope) -> Option<&Value> {
        match scope {
            Scope::User => self.user_file.as_ref(),
            Scope::Project => self.project.as_ref().map(|project| &project.project_file),
            Scope::Local => self
                .project
                .as_ref()
                .and_then(|project| project.local_file.as_ref()),
        }
    }

    /// The value of `def` in effect (its default when no document has it).
    pub fn effective(&self, def: &SettingDef) -> Value {
        let user_key = def.scope_allowed.contains(&Scope::User);
        let in_effect = if user_key {
            lookup(&self.user, &def.key)
        } else {
            self.project
                .as_ref()
                .and_then(|project| lookup(&project.effective, &def.key))
        };
        in_effect.cloned().unwrap_or_else(|| def.default_value())
    }
}

/// The value and origin of `def`.
pub fn resolve(def: &SettingDef, documents: &Documents) -> SettingValue {
    let stored = [Scope::Local, Scope::Project, Scope::User]
        .into_iter()
        .filter(|scope| def.scope_allowed.contains(scope))
        .find_map(|scope| {
            documents
                .file(scope)
                .and_then(|file| lookup(file, &def.key))
                .map(|value| (scope, value))
        });
    let effective = documents.effective(def);
    let (value, origin) = match stored {
        // The trust gate keeps a project value from applying: show what the file stores.
        Some((scope, value))
            if scope != Scope::User && rb_config::TRUST_GATED_KEYS.contains(&def.key.as_str()) =>
        {
            (value.clone(), Origin::from(scope))
        }
        Some((scope, _)) => (effective, Origin::from(scope)),
        // No user file to ask: a value other than the default was set by the user.
        None if documents.user_file.is_none()
            && def.scope_allowed.contains(&Scope::User)
            && effective != def.default_value() =>
        {
            (effective, Origin::User)
        }
        None => (effective, Origin::Default),
    };
    SettingValue {
        key: def.key.clone(),
        value,
        origin,
    }
}

/// Every key's value and origin, in the window's order.
pub fn values(registry: &SettingsRegistry, documents: &Documents) -> Vec<SettingValue> {
    ordered(registry)
        .into_iter()
        .map(|def| resolve(def, documents))
        .collect()
}

/// Every definition, by category, then section, then `x-order`.
pub fn ordered(registry: &SettingsRegistry) -> Vec<&SettingDef> {
    Category::ALL
        .into_iter()
        .flat_map(|category| registry.iter_category(category))
        .collect()
}

/// The schema the settings window renders from: `{ version, categories: [{ id, sections }],
/// defs: [SettingDef + plugin?], project: { root, hasLocal } | null }`. A def's `schema` is its
/// own JSON schema with its `x-*` annotations.
pub fn schema_payload(registry: &SettingsRegistry, documents: &Documents) -> Value {
    let categories: Vec<Value> = Category::ALL
        .into_iter()
        .map(|category| {
            let sections: Vec<&str> = category.sections().map(|section| section.id).collect();
            json!({ "id": category.id(), "sections": sections })
        })
        .collect();
    let defs: Vec<Value> = ordered(registry)
        .into_iter()
        .map(|def| {
            let mut value = serde_json::to_value(def).unwrap_or_else(|_| json!({}));
            if let (Some(plugin), Some(object)) =
                (registry.plugin_of(&def.key), value.as_object_mut())
            {
                object.insert("plugin".to_owned(), Value::String(plugin.to_owned()));
            }
            // A self-update key without the feature: shown locked with the hint (RB-090-02).
            if let (true, Some(object)) =
                (crate::updates::is_locked(&def.key), value.as_object_mut())
            {
                object.insert(
                    "lockedKey".to_owned(),
                    Value::String(crate::updates::INACTIVE_KEY.to_owned()),
                );
            }
            value
        })
        .collect();
    let project = documents.project.as_ref().map_or(Value::Null, |project| {
        let mut object = Map::new();
        object.insert(
            "root".to_owned(),
            Value::String(project.root.to_string_lossy().into_owned()),
        );
        object.insert(
            "hasLocal".to_owned(),
            Value::Bool(project.local_file.is_some()),
        );
        Value::Object(object)
    });
    json!({
        "version": SCHEMA_PAYLOAD_VERSION,
        "categories": categories,
        "defs": defs,
        "project": project,
        "texts": plugin_texts(registry),
    })
}

/// The texts of the plugins' keys - labels, descriptions, help, choices - and each plugin's
/// name, from the plugins' own catalogues: the window's catalogue is the core's only
/// (RB-080-82). Nothing else of a plugin's catalogue leaves for the window.
fn plugin_texts(registry: &SettingsRegistry) -> Map<String, Value> {
    let plugins: BTreeSet<&str> = registry
        .iter()
        .filter_map(|def| registry.plugin_of(&def.key))
        .collect();
    let mut texts = Map::new();
    for plugin in plugins {
        let settings = format!("plugin.{plugin}.settings.");
        let name = format!("plugin.{plugin}.name");
        for (key, text) in rb_i18n::plugin_texts(plugin) {
            if key.starts_with(&settings) || key == name {
                texts.insert(key, Value::String(text));
            }
        }
    }
    texts
}

#[cfg(test)]
#[path = "values_tests.rs"]
mod tests;
