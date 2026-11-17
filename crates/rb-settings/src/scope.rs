//! Scope resolution: the effective value of a key and where it comes from, over the three
//! documents `rb-config` loads - the user settings, `config.json` and `config.local.json`.
//!
//! The order, strongest first: Local, Project, User, the schema default. Only the scopes a key
//! allows (`x-scope`) are read: a local-only key set in `config.json` is ignored here as in
//! `rb-config`, and a user value never reaches a project key. A write is checked against the
//! scopes and, lightly, against the key's schema (`type`, `enum`, `minimum`, `maximum`); the full
//! validation of the file is `rb-config`'s. The store holds the documents in memory: whoever
//! owns the files writes [`LayeredStore::layer`] back after a [`SettingsStore::set`] and hands a
//! reloaded file in with [`LayeredStore::replace_layer`].

use std::sync::{Arc, PoisonError, RwLock};

use serde_json::{Map, Value};

use crate::registry::invalid;
use crate::{
    Origin, ResolvedValue, Scope, SettingDef, SettingsError, SettingsRegistry, SettingsStore,
};

/// The scopes in resolution order, strongest first.
const ORDER: [Scope; 3] = [Scope::Local, Scope::Project, Scope::User];

/// A [`SettingsStore`] over the registry and the three documents.
#[derive(Debug)]
pub struct LayeredStore {
    registry: Arc<SettingsRegistry>,
    layers: RwLock<[Value; 3]>,
}

impl LayeredStore {
    /// A store whose three documents are empty objects.
    pub fn new(registry: Arc<SettingsRegistry>) -> Self {
        Self {
            registry,
            layers: RwLock::new([empty(), empty(), empty()]),
        }
    }

    /// The store with `document` as the content of `scope`.
    pub fn with_layer(self, scope: Scope, document: Value) -> Self {
        self.replace_layer(scope, document);
        self
    }

    /// Replaces the document of `scope`, after its file changed on disk.
    pub fn replace_layer(&self, scope: Scope, document: Value) {
        let mut layers = self.layers.write().unwrap_or_else(PoisonError::into_inner);
        layers[index(scope)] = document;
    }

    /// A copy of the document of `scope`, to write to its file.
    pub fn layer(&self, scope: Scope) -> Value {
        let layers = self.layers.read().unwrap_or_else(PoisonError::into_inner);
        layers[index(scope)].clone()
    }

    /// The registry the store resolves against.
    pub fn registry(&self) -> &SettingsRegistry {
        &self.registry
    }

    fn def(&self, key: &str) -> Result<&SettingDef, SettingsError> {
        self.registry
            .get(key)
            .ok_or_else(|| SettingsError::UnknownKey(key.to_owned()))
    }
}

impl SettingsStore for LayeredStore {
    fn resolve(&self, key: &str) -> Result<ResolvedValue, SettingsError> {
        let def = self.def(key)?;
        let layers = self.layers.read().unwrap_or_else(PoisonError::into_inner);
        for scope in ORDER {
            if !def.scope_allowed.contains(&scope) {
                continue;
            }
            if let Some(value) = lookup(&layers[index(scope)], key) {
                return Ok(ResolvedValue {
                    value: value.clone(),
                    origin: Origin::from(scope),
                });
            }
        }
        Ok(ResolvedValue {
            value: def.default_value(),
            origin: Origin::Default,
        })
    }

    fn set(&self, key: &str, value: Value, scope: Scope) -> Result<(), SettingsError> {
        let def = self.def(key)?;
        if !def.scope_allowed.contains(&scope) || (def.local_only && scope != Scope::Local) {
            return Err(SettingsError::ScopeNotAllowed {
                key: key.to_owned(),
                scope,
            });
        }
        if !value.is_null() {
            check_value(&def.schema, &value).map_err(|reason| invalid(key, reason))?;
        }
        let mut layers = self.layers.write().unwrap_or_else(PoisonError::into_inner);
        let document = &mut layers[index(scope)];
        if value.is_null() {
            remove(document, key);
            Ok(())
        } else {
            insert(document, key, value).map_err(|reason| invalid(key, reason))
        }
    }
}

fn empty() -> Value {
    Value::Object(Map::new())
}

fn index(scope: Scope) -> usize {
    match scope {
        Scope::User => 0,
        Scope::Project => 1,
        Scope::Local => 2,
    }
}

fn lookup<'a>(document: &'a Value, key: &str) -> Option<&'a Value> {
    key.split('.')
        .try_fold(document, |node, segment| node.as_object()?.get(segment))
}

fn insert(document: &mut Value, key: &str, value: Value) -> Result<(), String> {
    let segments: Vec<&str> = key.split('.').collect();
    let Some((last, parents)) = segments.split_last() else {
        return Err("empty key".to_owned());
    };
    let mut node = document;
    for segment in parents {
        let object = node
            .as_object_mut()
            .ok_or_else(|| format!("`{segment}` sits in a value that is not an object"))?;
        node = object.entry(*segment).or_insert_with(empty);
    }
    let object = node
        .as_object_mut()
        .ok_or_else(|| format!("the parent of `{last}` is not an object"))?;
    object.insert((*last).to_owned(), value);
    Ok(())
}

fn remove(document: &mut Value, key: &str) {
    let segments: Vec<&str> = key.split('.').collect();
    let Some((last, parents)) = segments.split_last() else {
        return;
    };
    let mut node = document;
    for segment in parents {
        match node
            .as_object_mut()
            .and_then(|object| object.get_mut(*segment))
        {
            Some(child) => node = child,
            None => return,
        }
    }
    if let Some(object) = node.as_object_mut() {
        object.remove(*last);
    }
}

impl SettingDef {
    /// The light check of `value` against the key's schema (`type`, `enum`, `minimum`,
    /// `maximum`): what a write must pass, and what `rb-config` holds a loaded file to.
    pub fn check_value(&self, value: &Value) -> Result<(), String> {
        check_value(&self.schema, value)
    }
}

/// The light check of a written value: `type`, `enum`, `minimum` and `maximum` where the schema
/// names them.
fn check_value(schema: &Value, value: &Value) -> Result<(), String> {
    let types: Vec<&str> = match schema.get("type") {
        Some(Value::String(one)) => vec![one.as_str()],
        Some(Value::Array(many)) => many.iter().filter_map(Value::as_str).collect(),
        _ => Vec::new(),
    };
    if !types.is_empty() && !types.iter().any(|kind| has_type(value, kind)) {
        return Err(format!("expected {}", types.join(" or ")));
    }
    if let Some(allowed) = schema.get("enum").and_then(Value::as_array)
        && !allowed.contains(value)
    {
        return Err("not one of the allowed values".to_owned());
    }
    if let Some(number) = value.as_f64() {
        if let Some(minimum) = schema.get("minimum").and_then(Value::as_f64)
            && number < minimum
        {
            return Err(format!("below the minimum {minimum}"));
        }
        if let Some(maximum) = schema.get("maximum").and_then(Value::as_f64)
            && number > maximum
        {
            return Err(format!("above the maximum {maximum}"));
        }
    }
    Ok(())
}

fn has_type(value: &Value, kind: &str) -> bool {
    match kind {
        "null" => value.is_null(),
        "boolean" => value.is_boolean(),
        "integer" => value.is_i64() || value.is_u64(),
        "number" => value.is_number(),
        "string" => value.is_string(),
        "array" => value.is_array(),
        "object" => value.is_object(),
        _ => true,
    }
}

#[cfg(test)]
#[path = "scope_tests.rs"]
mod tests;
