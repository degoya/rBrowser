//! Secret settings (RB-094-26, RB-094-27): a key annotated `x-secret: true`
//! ([`crate::X_SECRET`], `api-client.secrets`) holds credentials. It is no setting of the
//! settings window - not listed, not searched, not in its JSON view - and stays in the registry,
//! so its plugin writes it and MCP reads it masked ([`mask_secret`]; the names inside the value
//! stay). Its `x-panel` ([`crate::X_PANEL`]) names where its values are edited
//! ([`SettingDef::editor_panel`]): a link to the key opens that panel.

use serde_json::Value;

use crate::{SettingDef, X_PANEL, X_SECRET};

/// What a secret's value shows instead: rb-diag's redaction placeholder.
pub const SECRET_MASK: &str = "***";

impl SettingDef {
    /// Whether the key's values are secrets (`x-secret: true` in its schema).
    pub fn is_secret(&self) -> bool {
        self.schema.get(X_SECRET).and_then(Value::as_bool) == Some(true)
    }

    /// The panel a secret key's values are edited in (its `x-panel`): `None` for a key that is
    /// not secret, names none, or names one by an id that is not `[a-z][a-z0-9-]*` (at most 40).
    pub fn editor_panel(&self) -> Option<EditorPanel<'_>> {
        if !self.is_secret() {
            return None;
        }
        let named = self.schema.get(X_PANEL)?;
        let panel = named
            .get("panel")
            .and_then(Value::as_str)
            .filter(|id| is_id(id))?;
        let tab = match named.get("tab") {
            None => None,
            Some(tab) => Some(tab.as_str().filter(|id| is_id(id))?),
        };
        Some(EditorPanel { panel, tab })
    }
}

/// Where a secret key's values are edited: a panel of the key's plugin and, when named, its tab
/// (RB-094-27: `api-client.secrets` in the API client's Variables tab).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EditorPanel<'a> {
    /// The panel id, as the plugin's manifest names it.
    pub panel: &'a str,
    /// The tab the panel shows the values in.
    pub tab: Option<&'a str>,
}

/// `[a-z][a-z0-9-]*`, at most 40 characters: a panel id as a manifest checks it.
fn is_id(id: &str) -> bool {
    (1..=40).contains(&id.len())
        && id.starts_with(|c: char| c.is_ascii_lowercase())
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// `value` with every scalar replaced by [`SECRET_MASK`]: the object keys and the shape stay,
/// `null` stays `null`.
pub fn mask_secret(value: &Value) -> Value {
    match value {
        Value::Null => Value::Null,
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(name, item)| (name.clone(), mask_secret(item)))
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.iter().map(mask_secret).collect()),
        Value::Bool(_) | Value::Number(_) | Value::String(_) => {
            Value::String(SECRET_MASK.to_owned())
        }
    }
}

#[cfg(test)]
#[path = "secret_tests.rs"]
mod tests;
