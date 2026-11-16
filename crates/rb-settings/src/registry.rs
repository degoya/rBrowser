//! The settings registry: every known setting, checked on the way in.
//!
//! A definition is accepted only with a well-formed key, a section its category has, at least
//! one scope (exactly Local for a local-only key), well-formed i18n keys and an `x-order` that no
//! other key of the same source repeats in its section. The source is the core or one plugin:
//! plugins number their own keys, so two plugins never collide over an order. A plugin's keys
//! start with `<plugin id>.`; its category is always one of the fixed [`Category`] values.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::{Category, SECTIONS, SettingDef, SettingsError, section};

/// The set of every known setting, core and plugins, by key.
#[derive(Debug, Default)]
pub struct SettingsRegistry {
    defs: BTreeMap<String, SettingDef>,
    /// The plugin that contributed a key; core keys have no entry.
    plugins: BTreeMap<String, String>,
}

impl SettingsRegistry {
    /// An empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a core definition after checking its key, section, scopes, i18n keys and order.
    pub fn register(&mut self, def: SettingDef) -> Result<(), SettingsError> {
        self.insert(def, None)
    }

    /// Adds a definition of plugin `plugin_id`: the same checks as [`Self::register`], and the
    /// key must start with `<plugin_id>.`.
    pub fn register_plugin(
        &mut self,
        plugin_id: &str,
        def: SettingDef,
    ) -> Result<(), SettingsError> {
        if !is_key_segment(plugin_id) {
            return Err(invalid(
                &def.key,
                format!("malformed plugin id `{plugin_id}`"),
            ));
        }
        if !def.key.starts_with(&format!("{plugin_id}.")) {
            return Err(invalid(
                &def.key,
                format!("a key of plugin `{plugin_id}` starts with `{plugin_id}.`"),
            ));
        }
        self.insert(def, Some(plugin_id))
    }

    fn insert(&mut self, def: SettingDef, plugin: Option<&str>) -> Result<(), SettingsError> {
        check_def(&def)?;
        if self.defs.contains_key(&def.key) {
            return Err(SettingsError::Duplicate(def.key));
        }
        let clash = self.defs.values().find(|other| {
            other.category == def.category
                && other.section == def.section
                && other.order == def.order
                && self.plugin_of(&other.key) == plugin
        });
        if let Some(other) = clash {
            return Err(invalid(
                &def.key,
                format!(
                    "x-order {} repeats `{}` in section {}/{}",
                    def.order,
                    other.key,
                    def.category.id(),
                    def.section
                ),
            ));
        }
        if let Some(plugin) = plugin {
            self.plugins.insert(def.key.clone(), plugin.to_owned());
        }
        self.defs.insert(def.key.clone(), def);
        Ok(())
    }

    /// The definition of `key`.
    pub fn get(&self, key: &str) -> Option<&SettingDef> {
        self.defs.get(key)
    }

    /// Every definition, by key.
    pub fn iter(&self) -> impl Iterator<Item = &SettingDef> {
        self.defs.values()
    }

    /// The definitions of one category in display order: by section (the order of
    /// [`SECTIONS`]), then `x-order`, then key - core and plugin keys mixed.
    pub fn iter_category(&self, category: Category) -> impl Iterator<Item = &SettingDef> {
        let mut defs: Vec<&SettingDef> = self
            .defs
            .values()
            .filter(|def| def.category == category)
            .collect();
        defs.sort_by_key(|def| (section_position(def), def.order, def.key.as_str()));
        defs.into_iter()
    }

    /// The definitions of plugin `plugin_id`, in key order: its own page under Plugins.
    pub fn iter_plugin<'a>(&'a self, plugin_id: &'a str) -> impl Iterator<Item = &'a SettingDef> {
        self.defs
            .values()
            .filter(move |def| self.plugin_of(&def.key) == Some(plugin_id))
    }

    /// The plugin that contributed `key`, or `None` for a core key.
    pub fn plugin_of(&self, key: &str) -> Option<&str> {
        self.plugins.get(key).map(String::as_str)
    }

    /// The definition of `key` when an MCP caller may read or write it: refuses an unknown key
    /// and an `x-protected` one (trust, plugins, MCP itself) for every MCP caller.
    pub fn check_mcp_access(&self, key: &str) -> Result<&SettingDef, SettingsError> {
        let def = self
            .get(key)
            .ok_or_else(|| SettingsError::UnknownKey(key.to_owned()))?;
        if def.protected_from_mcp {
            return Err(SettingsError::Protected(key.to_owned()));
        }
        Ok(def)
    }
}

impl SettingDef {
    /// The schema's `default`, or `null` when the schema names none.
    pub fn default_value(&self) -> Value {
        self.schema.get("default").cloned().unwrap_or(Value::Null)
    }
}

fn section_position(def: &SettingDef) -> usize {
    SECTIONS
        .iter()
        .position(|s| s.category == def.category && s.id == def.section)
        .unwrap_or(usize::MAX)
}

/// The rules every definition obeys, core or plugin.
pub(crate) fn check_def(def: &SettingDef) -> Result<(), SettingsError> {
    if !is_setting_key(&def.key) {
        return Err(invalid(&def.key, "malformed key".to_owned()));
    }
    if section(def.category, &def.section).is_none() {
        return Err(SettingsError::UnknownSection {
            category: def.category.id().to_owned(),
            section: def.section.clone(),
        });
    }
    if def.scope_allowed.is_empty() {
        return Err(invalid(&def.key, "x-scope names no scope".to_owned()));
    }
    let mut scopes = def.scope_allowed.clone();
    scopes.sort_unstable();
    scopes.dedup();
    if scopes.len() != def.scope_allowed.len() {
        return Err(invalid(&def.key, "x-scope repeats a scope".to_owned()));
    }
    if def.local_only && def.scope_allowed != [crate::Scope::Local] {
        return Err(invalid(
            &def.key,
            "a local-only key has x-scope [\"local\"]".to_owned(),
        ));
    }
    for (what, key) in [
        ("x-label-key", &def.label_key),
        ("x-description-key", &def.description_key),
        ("x-help-key", &def.help_key),
    ] {
        if !is_i18n_key(key) {
            return Err(invalid(&def.key, format!("{what} `{key}` is malformed")));
        }
    }
    Ok(())
}

pub(crate) fn invalid(key: &str, reason: String) -> SettingsError {
    SettingsError::Invalid {
        key: key.to_owned(),
        reason,
    }
}

/// A dotted settings key: segments of ASCII letters, digits, `_` and `-`, each starting with a
/// letter (`sync.scroll`, `network.ignoreCertificateErrors`).
pub(crate) fn is_setting_key(key: &str) -> bool {
    !key.is_empty() && key.split('.').all(is_key_segment)
}

fn is_key_segment(segment: &str) -> bool {
    segment.starts_with(|c: char| c.is_ascii_alphabetic())
        && segment
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// The i18n key rule of `rb_i18n::is_well_formed_key` (this crate depends on no other rBrowser
/// crate): a lower-case area, then at least one segment of ASCII letters, digits, `_` and `-`
/// starting with a lower-case letter or a digit.
fn is_i18n_key(key: &str) -> bool {
    let mut segments = key.split('.');
    let area_ok = segments.next().is_some_and(|area| {
        area.starts_with(|c: char| c.is_ascii_lowercase())
            && area
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
    });
    let mut rest = segments.peekable();
    let has_rest = rest.peek().is_some();
    area_ok
        && has_rest
        && rest.all(|segment| {
            segment.starts_with(|c: char| c.is_ascii_lowercase() || c.is_ascii_digit())
                && segment
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        })
}

#[cfg(test)]
#[path = "registry_tests.rs"]
mod tests;
