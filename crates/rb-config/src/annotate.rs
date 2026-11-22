//! The settings annotations of the generated schemas (`docs/settings.md`, "Schema annotations"
//! and "Placement of the 0.1 keys").
//!
//! A **key** is a property the settings window edits as one field; it gets `x-category`,
//! `x-section`, `x-order` and `x-scope`, a local-only key also `x-local-only`, a key MCP may not
//! touch `x-protected` (names: `rb-settings`), a key that applies only at the next start
//! `x-restart` ([`RESTART_KEYS`], RB-094-23). A root property that is no key is a **group**
//! (`sync`, `network`): it stays unannotated and each of its properties is a key
//! (`sync.scroll`). The root's `$schema` and `version` are file structure, neither. The tables
//! (`project_keys.rs`, `user_keys.rs`) are the keys in display order; `x-order` numbers them per
//! section in that order, counted across the project and the user schema together (a key may
//! fix its number), and `annotate_tests` holds it unique per section.

use std::collections::BTreeMap;

use rb_settings::{
    Category, Scope, X_CATEGORY, X_LOCAL_ONLY, X_ORDER, X_PROTECTED, X_SCOPE, X_SECTION,
};
use serde_json::{Map, Value};

use crate::LOCAL_ONLY_KEYS;

mod inputs;
mod project_keys;
mod restart;
mod user_keys;

pub use inputs::{InputDef, PROJECT_INPUTS, USER_INPUTS, annotate_inputs};
pub use project_keys::PROJECT_KEYS;
pub use restart::{RESTART_KEYS, X_RESTART};
pub use user_keys::USER_KEYS;

/// One settings key and where the window shows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyDef {
    /// The dotted key: a root property or `<group>.<property>`.
    pub key: &'static str,
    /// `x-category`.
    pub category: Category,
    /// `x-section`.
    pub section: &'static str,
    /// `x-protected`: refused for every MCP caller.
    pub protected: bool,
    /// A fixed `x-order` (agreed with another crate's definition); the next free number of the
    /// section when `None`.
    pub order: Option<u32>,
    /// A project key the user settings may set too (`x-scope` project, local and user; the
    /// project wins, `crate::shared_settings`).
    pub shared: bool,
}

impl KeyDef {
    /// The same key, `x-protected`.
    const fn protect(self) -> KeyDef {
        KeyDef {
            protected: true,
            ..self
        }
    }

    /// The same key at the fixed `x-order` `order`.
    const fn at(self, order: u32) -> KeyDef {
        KeyDef {
            order: Some(order),
            ..self
        }
    }
}

const fn key(key: &'static str, category: Category, section: &'static str) -> KeyDef {
    KeyDef {
        key,
        category,
        section,
        protected: false,
        order: None,
        shared: false,
    }
}

/// A project key the user settings may set as well.
const fn shared(key: &'static str, category: Category, section: &'static str) -> KeyDef {
    KeyDef {
        key,
        category,
        section,
        protected: false,
        order: None,
        shared: true,
    }
}

/// A key that chooses a program to run or widens what an agent can do.
const fn protected(key: &'static str, category: Category, section: &'static str) -> KeyDef {
    KeyDef {
        key,
        category,
        section,
        protected: true,
        order: None,
        shared: false,
    }
}

/// The root properties that are file structure, neither key nor group.
#[cfg(test)]
pub const STRUCTURAL_KEYS: &[&str] = &["$schema", "version"];

/// Which file a schema describes; decides `x-scope` and the key table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SchemaKind {
    /// `config.json` / `config.local.json`.
    Project,
    /// The user settings.
    User,
}

impl SchemaKind {
    /// The keys of this schema.
    pub fn keys(self) -> &'static [KeyDef] {
        match self {
            SchemaKind::Project => PROJECT_KEYS,
            SchemaKind::User => USER_KEYS,
        }
    }

    /// The paths of this schema a native dialog fills (`x-input`, RB-080-20).
    pub fn inputs(self) -> &'static [InputDef] {
        match self {
            SchemaKind::Project => PROJECT_INPUTS,
            SchemaKind::User => USER_INPUTS,
        }
    }
}

/// Counts `x-order` per (category, section) across the schemas annotated with it.
pub type OrderCounters = BTreeMap<(Category, &'static str), u32>;

/// Annotates every key of `kind`'s table in the root `schema`, then every path of its input
/// table; the keys and paths it cannot find (a table entry without its field) come back as the
/// error.
pub fn annotate(
    schema: &mut Value,
    kind: SchemaKind,
    counters: &mut OrderCounters,
) -> Result<(), Vec<&'static str>> {
    let mut missing = Vec::new();
    for def in kind.keys() {
        let Some(map) = property_mut(schema, def.key) else {
            missing.push(def.key);
            continue;
        };
        let order = match def.order {
            Some(fixed) => fixed,
            None => {
                let next = counters.entry((def.category, def.section)).or_insert(0);
                *next += 1;
                *next
            }
        };
        let local_only = kind == SchemaKind::Project && LOCAL_ONLY_KEYS.contains(&def.key);
        let scopes: &[Scope] = match kind {
            SchemaKind::User => &[Scope::User],
            SchemaKind::Project if local_only => &[Scope::Local],
            SchemaKind::Project if def.shared => &[Scope::Project, Scope::Local, Scope::User],
            SchemaKind::Project => &[Scope::Project, Scope::Local],
        };
        map.insert(X_CATEGORY.to_owned(), Value::from(def.category.id()));
        map.insert(X_SECTION.to_owned(), Value::from(def.section));
        map.insert(X_ORDER.to_owned(), Value::from(order));
        map.insert(
            X_SCOPE.to_owned(),
            serde_json::to_value(scopes).expect("scopes serialize as strings"),
        );
        if local_only {
            map.insert(X_LOCAL_ONLY.to_owned(), Value::Bool(true));
        }
        if def.protected {
            map.insert(X_PROTECTED.to_owned(), Value::Bool(true));
        }
        if RESTART_KEYS.contains(&def.key) {
            map.insert(X_RESTART.to_owned(), Value::Bool(true));
        }
    }
    if let Err(paths) = annotate_inputs(schema, kind.inputs()) {
        missing.extend(paths);
    }
    if missing.is_empty() {
        Ok(())
    } else {
        Err(missing)
    }
}

/// The schema object of the property at dotted `key` (`properties` at each level).
fn property_mut<'a>(schema: &'a mut Value, key: &str) -> Option<&'a mut Map<String, Value>> {
    let mut node = schema;
    for segment in key.split('.') {
        node = node.get_mut("properties")?.get_mut(segment)?;
    }
    if node.as_bool() == Some(true) {
        *node = Value::Object(Map::new());
    }
    node.as_object_mut()
}

#[cfg(test)]
#[path = "annotate_tests.rs"]
mod tests;
