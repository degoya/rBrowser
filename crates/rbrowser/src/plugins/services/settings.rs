//! A plugin's own settings (Charge 6): the keys of its settings schema, `<plugin-id>.<path>`,
//! each resolved by the scopes its `x-scope` allows, strongest first - `config.local.json`, then
//! `config.json`, then the user settings - else the schema default (review finding L5 of
//! Charge 6). What `settings.get` answers the component and the panels window's `settings`
//! message the panel. Never another plugin's keys, never rBrowser's. The values of every plugin
//! are resolved once per change of the user settings or the project ([`SettingsCache`]), not at
//! each call.

use std::collections::BTreeMap;
use std::sync::{Arc, PoisonError, RwLock};

use rb_settings::Scope;
use serde_json::{Map, Value};

/// The documents a plugin's values resolve over.
#[derive(Clone, Debug, Default)]
pub struct Layers {
    /// The user settings in effect, as JSON.
    pub user: Value,
    /// `.rbrowser/config.json` as stored; `None` without a project.
    pub project: Option<Value>,
    /// `.rbrowser/config.local.json` as stored; `None` when there is none.
    pub local: Option<Value>,
}

impl Layers {
    fn of(&self, scope: Scope) -> Option<&Value> {
        match scope {
            Scope::User => Some(&self.user),
            Scope::Project => self.project.as_ref(),
            Scope::Local => self.local.as_ref(),
        }
    }
}

/// The scopes in resolution order, strongest first.
const ORDER: [Scope; 3] = [Scope::Local, Scope::Project, Scope::User];

/// Core keys a plugin's key took over, `(plugin key, old core path)`: in a document without the
/// plugin's key the old one counts, unless it is empty (its old default), so a project that set
/// it loses nothing. `checks.axe.tags` was a core key no code read; `a11y-axe.tags` is the one
/// the axe plugin reads (Charge 10).
const TAKEN_OVER: &[(&str, &str)] = &[("a11y-axe.tags", "checks.axe.tags")];

/// The value at the dotted `path` of `document`.
fn at<'a>(document: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('.')
        .try_fold(document, |node, segment| node.get(segment))
}

/// `key` in `document`, else the old core key it took over when that holds a choice.
fn value_in<'a>(document: &'a Value, key: &str) -> Option<&'a Value> {
    at(document, key).or_else(|| {
        let (_, old) = TAKEN_OVER.iter().find(|(new, _)| *new == key)?;
        at(document, old).filter(|value| {
            !value.is_null() && value.as_array().is_none_or(|items| !items.is_empty())
        })
    })
}

/// Every key of plugin `plugin_id`'s `schema` with its value in the strongest of `layers` its
/// scopes allow, else its default.
#[must_use]
pub fn plugin_values(plugin_id: &str, schema: &Value, layers: &Layers) -> Map<String, Value> {
    let Ok(defs) = rb_settings::plugin_defs_from_schema(plugin_id, schema) else {
        return Map::new();
    };
    defs.into_iter()
        .map(|def| {
            let value = ORDER
                .into_iter()
                .filter(|scope| def.scope_allowed.contains(scope))
                .filter_map(|scope| layers.of(scope))
                .find_map(|document| value_in(document, &def.key))
                .cloned()
                .unwrap_or_else(|| def.default_value());
            (def.key, value)
        })
        .collect()
}

/// `settings.get(key)` of plugin `plugin_id` over its resolved `values`: `key` is the path
/// below `<plugin-id>.`, or the whole key; the value as JSON text, `None` for a key its schema
/// does not declare.
#[must_use]
pub fn plugin_setting(plugin_id: &str, values: &Map<String, Value>, key: &str) -> Option<String> {
    let prefix = format!("{plugin_id}.");
    let full = if key.starts_with(&prefix) {
        key.to_owned()
    } else {
        format!("{prefix}{key}")
    };
    values.get(&full).map(Value::to_string)
}

/// The resolved values of every plugin, by plugin id.
pub type PluginValues = BTreeMap<String, Map<String, Value>>;

/// The values of every plugin, resolved on first use after a change and kept until the next.
/// A change during a resolution is not lost: values resolved before it are handed out once and
/// never kept.
#[derive(Debug, Default)]
pub struct SettingsCache {
    state: RwLock<CacheState>,
}

#[derive(Debug, Default)]
struct CacheState {
    /// Counts the changes.
    generation: u64,
    values: Option<Arc<PluginValues>>,
}

impl SettingsCache {
    /// The values held, or those `build` resolves now (then held).
    pub fn get_or_build(&self, build: impl FnOnce() -> PluginValues) -> Arc<PluginValues> {
        let generation = {
            let state = self.state.read().unwrap_or_else(PoisonError::into_inner);
            if let Some(values) = &state.values {
                return Arc::clone(values);
            }
            state.generation
        };
        let values = Arc::new(build());
        let mut state = self.state.write().unwrap_or_else(PoisonError::into_inner);
        if state.generation == generation {
            state.values = Some(Arc::clone(&values));
        }
        values
    }

    /// The user settings, the project or the schemas changed: the next use resolves again.
    pub fn invalidate(&self) {
        let mut state = self.state.write().unwrap_or_else(PoisonError::into_inner);
        state.generation = state.generation.wrapping_add(1);
        state.values = None;
    }
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
