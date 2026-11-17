//! Reads the setting definitions out of a JSON schema's `x-*` annotations - the config and
//! user-settings schemas of `rb-config` and the plugins' schema fragments.
//!
//! The walk, the same as the lint's (`scripts/lib/settings_schema.py`): every property of the
//! root object is a key. A property that carries any of `x-category`, `x-section` or `x-order`
//! is a setting and must carry all three plus `x-scope`; its value, an object or an array
//! included, is edited as one. A property without them that resolves (through `$ref`, a
//! single-element `allOf`, or an `anyOf`/`oneOf` with one non-null branch) to an object with
//! `properties` is a group: the walk goes into it and its keys are `<group>.<property>`. Anything
//! else is a key without annotations, an issue. The root's [`STRUCTURAL_KEYS`] are file
//! structure, not settings.

use serde_json::{Map, Value};

use crate::registry::invalid;
use crate::{
    Category, Scope, SettingDef, SettingsError, SettingsRegistry, X_CATEGORY, X_DESCRIPTION_KEY,
    X_HELP_KEY, X_LABEL_KEY, X_LOCAL_ONLY, X_ORDER, X_PROTECTED, X_SCOPE, X_SECTION, section,
};

/// The properties of a schema's root that describe the file, not a setting: skipped by the
/// walk and the lint.
pub const STRUCTURAL_KEYS: &[&str] = &["$schema", "version"];

/// How deep `$ref`s and groups are followed; a recursive schema ends here with an issue.
const MAX_DEPTH: usize = 32;

/// A key of a schema whose annotations are missing or wrong.
#[derive(Debug, PartialEq)]
pub struct SchemaIssue {
    /// The dotted key.
    pub key: String,
    /// What is wrong.
    pub error: SettingsError,
}

/// The definitions of a core schema (the project config or the user settings), in schema order.
/// Every issue is returned at once - one per key - so a lint names them all. Labels default to
/// `settings.key.<key>.label` / `.description` / `.help`.
pub fn defs_from_schema(schema: &Value) -> Result<Vec<SettingDef>, Vec<SchemaIssue>> {
    walk_schema(schema, &Naming::Core)
}

/// The definitions of plugin `plugin_id`'s schema fragment: keys `<plugin_id>.<property>`,
/// labels `plugin.<plugin_id>.settings.<property>.label` / `.description` / `.help` (the plugin's own
/// i18n namespace). An `x-category` that is not one of the fixed categories is an
/// [`SettingsError::UnknownCategory`]: a plugin cannot add a top-level category.
pub fn plugin_defs_from_schema(
    plugin_id: &str,
    schema: &Value,
) -> Result<Vec<SettingDef>, Vec<SchemaIssue>> {
    walk_schema(schema, &Naming::Plugin(plugin_id))
}

impl SettingsRegistry {
    /// Reads a core schema with [`defs_from_schema`] and registers every definition; returns
    /// how many, or every issue of the reading or the registering.
    pub fn register_schema(&mut self, schema: &Value) -> Result<usize, Vec<SchemaIssue>> {
        let defs = defs_from_schema(schema)?;
        self.register_all(defs, None)
    }

    /// Reads plugin `plugin_id`'s fragment with [`plugin_defs_from_schema`] and registers every
    /// definition as the plugin's.
    pub fn register_plugin_schema(
        &mut self,
        plugin_id: &str,
        schema: &Value,
    ) -> Result<usize, Vec<SchemaIssue>> {
        let defs = plugin_defs_from_schema(plugin_id, schema)?;
        self.register_all(defs, Some(plugin_id))
    }

    fn register_all(
        &mut self,
        defs: Vec<SettingDef>,
        plugin: Option<&str>,
    ) -> Result<usize, Vec<SchemaIssue>> {
        let count = defs.len();
        let mut issues = Vec::new();
        for def in defs {
            let key = def.key.clone();
            let result = match plugin {
                Some(plugin) => self.register_plugin(plugin, def),
                None => self.register(def),
            };
            if let Err(error) = result {
                issues.push(SchemaIssue { key, error });
            }
        }
        if issues.is_empty() {
            Ok(count)
        } else {
            Err(issues)
        }
    }
}

enum Naming<'a> {
    Core,
    Plugin(&'a str),
}

impl Naming<'_> {
    fn key(&self, path: &str) -> String {
        match self {
            Naming::Core => path.to_owned(),
            Naming::Plugin(id) => format!("{id}.{path}"),
        }
    }

    fn i18n(&self, path: &str, what: &str) -> String {
        match self {
            Naming::Core => format!("settings.key.{path}.{what}"),
            Naming::Plugin(id) => format!("plugin.{id}.settings.{path}.{what}"),
        }
    }
}

fn walk_schema(schema: &Value, naming: &Naming) -> Result<Vec<SettingDef>, Vec<SchemaIssue>> {
    let mut defs = Vec::new();
    let mut issues = Vec::new();
    match properties(resolve(schema, schema, 0)) {
        Some(props) => {
            for (name, prop) in props {
                if STRUCTURAL_KEYS.contains(&name.as_str()) {
                    continue;
                }
                walk(schema, name, prop, naming, 0, &mut defs, &mut issues);
            }
        }
        None => issues.push(SchemaIssue {
            key: String::new(),
            error: invalid("", "the schema has no properties".to_owned()),
        }),
    }
    if issues.is_empty() {
        Ok(defs)
    } else {
        Err(issues)
    }
}

fn walk(
    root: &Value,
    path: &str,
    prop: &Value,
    naming: &Naming,
    depth: usize,
    defs: &mut Vec<SettingDef>,
    issues: &mut Vec<SchemaIssue>,
) {
    let key = naming.key(path);
    let annotated = [X_CATEGORY, X_SECTION, X_ORDER]
        .iter()
        .any(|name| prop.get(name).is_some());
    if annotated {
        match read_def(root, &key, path, prop, naming) {
            Ok(def) => defs.push(def),
            Err(error) => issues.push(SchemaIssue { key, error }),
        }
        return;
    }
    let group = if depth < MAX_DEPTH {
        properties(resolve(root, prop, 0))
    } else {
        None
    };
    match group {
        Some(props) => {
            for (name, child) in props {
                let child_path = format!("{path}.{name}");
                walk(root, &child_path, child, naming, depth + 1, defs, issues);
            }
        }
        None => issues.push(SchemaIssue {
            error: invalid(&key, format!("missing {X_CATEGORY}")),
            key,
        }),
    }
}

/// The `properties` of an object schema.
fn properties(node: &Value) -> Option<&Map<String, Value>> {
    node.get("properties").and_then(Value::as_object)
}

/// `node` with its `$ref`, single `allOf` or one non-null `anyOf`/`oneOf` branch followed.
fn resolve<'a>(root: &'a Value, node: &'a Value, depth: usize) -> &'a Value {
    if depth >= MAX_DEPTH {
        return node;
    }
    if let Some(target) = node
        .get("$ref")
        .and_then(Value::as_str)
        .and_then(|reference| reference.strip_prefix('#'))
        .and_then(|pointer| root.pointer(pointer))
    {
        return resolve(root, target, depth + 1);
    }
    if let Some([only]) = node
        .get("allOf")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
    {
        return resolve(root, only, depth + 1);
    }
    for combinator in ["anyOf", "oneOf"] {
        if let Some(branches) = node.get(combinator).and_then(Value::as_array) {
            let mut real = branches
                .iter()
                .filter(|branch| branch.get("type").and_then(Value::as_str) != Some("null"));
            if let (Some(only), None) = (real.next(), real.next()) {
                return resolve(root, only, depth + 1);
            }
        }
    }
    node
}

fn read_def(
    root: &Value,
    key: &str,
    path: &str,
    prop: &Value,
    naming: &Naming,
) -> Result<SettingDef, SettingsError> {
    let text = |name: &str| -> Result<&str, SettingsError> {
        match prop.get(name) {
            None => Err(invalid(key, format!("missing {name}"))),
            Some(value) => value
                .as_str()
                .ok_or_else(|| invalid(key, format!("{name} is not a string"))),
        }
    };
    let category_id = text(X_CATEGORY)?;
    let category = Category::from_id(category_id)
        .ok_or_else(|| SettingsError::UnknownCategory(category_id.to_owned()))?;
    let section_id = text(X_SECTION)?;
    if section(category, section_id).is_none() {
        return Err(SettingsError::UnknownSection {
            category: category_id.to_owned(),
            section: section_id.to_owned(),
        });
    }
    let order = match prop.get(X_ORDER) {
        None => return Err(invalid(key, format!("missing {X_ORDER}"))),
        Some(value) => value
            .as_u64()
            .and_then(|order| u32::try_from(order).ok())
            .ok_or_else(|| invalid(key, format!("{X_ORDER} is not a whole number >= 0")))?,
    };
    let scope_allowed = match prop.get(X_SCOPE) {
        None => return Err(invalid(key, format!("missing {X_SCOPE}"))),
        Some(value) => serde_json::from_value::<Vec<Scope>>(value.clone()).map_err(|_| {
            invalid(
                key,
                format!("{X_SCOPE} is not a list of \"user\", \"project\", \"local\""),
            )
        })?,
    };
    let flag = |name: &str| -> Result<bool, SettingsError> {
        match prop.get(name) {
            None => Ok(false),
            Some(value) => value
                .as_bool()
                .ok_or_else(|| invalid(key, format!("{name} is not true or false"))),
        }
    };
    let label = |name: &str, what: &str| -> Result<String, SettingsError> {
        match prop.get(name) {
            None => Ok(naming.i18n(path, what)),
            Some(_) => text(name).map(str::to_owned),
        }
    };
    let def = SettingDef {
        key: key.to_owned(),
        category,
        section: section_id.to_owned(),
        order,
        scope_allowed,
        local_only: flag(X_LOCAL_ONLY)?,
        protected_from_mcp: flag(X_PROTECTED)?,
        label_key: label(X_LABEL_KEY, "label")?,
        description_key: label(X_DESCRIPTION_KEY, "description")?,
        help_key: label(X_HELP_KEY, "help")?,
        schema: own_schema(root, prop),
    };
    crate::registry::check_def(&def)?;
    Ok(def)
}

/// The key's schema with a `$ref` replaced by its target: the target's fields, then the key's
/// own (annotations, default, description) over them.
fn own_schema(root: &Value, prop: &Value) -> Value {
    let Some(own) = prop.as_object() else {
        return prop.clone();
    };
    if !own.contains_key("$ref") {
        return prop.clone();
    }
    let mut merged = resolve(root, prop, 0)
        .as_object()
        .cloned()
        .unwrap_or_default();
    for (name, value) in own {
        if name != "$ref" {
            merged.insert(name.clone(), value.clone());
        }
    }
    Value::Object(merged)
}

#[cfg(test)]
#[path = "annotate_tests.rs"]
mod tests;
