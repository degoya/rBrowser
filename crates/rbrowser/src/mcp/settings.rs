//! The settings over MCP on the host's side (RB-030-03): the document `settings_get` reads
//! ([`document`]: the settings window's categories, sections and keys in its order, with type,
//! value, origin, default, label, description and help text in English, secrets masked) and the audit of
//! every change ([`record`]: a log line with the key, the scope and the old and new value, both
//! masked, and - while `mcp.notifyOnSettingChange` is on - a notice in the shell that links to
//! the key).

use std::collections::BTreeMap;

use rb_ipc::{CoreToShell, Notification, NotificationLevel, SettingValue, ShellToCore};
use rb_mcp::SettingScope;
use rb_settings::{Category, SettingsRegistry};
use serde_json::{Value, json};

use super::backend::masked;
use crate::control::Core;

/// The i18n key of the notice that MCP changed a setting.
pub const CHANGED_KEY: &str = "mcp.notice.setting_changed";
/// The most characters of a value the audit line shows.
const AUDIT_CHARS: usize = 200;

/// The settings document over `registry` and the `values` the settings window reads, its texts
/// from `text` (an i18n key to English).
pub fn document(
    registry: &SettingsRegistry,
    values: &[SettingValue],
    text: &dyn Fn(&str) -> String,
) -> Value {
    let categories: Vec<Value> = Category::ALL
        .into_iter()
        .map(|category| {
            let sections: Vec<Value> = category
                .sections()
                .map(|section| json!({ "id": section.id, "title": text(&section.title_key()) }))
                .collect();
            json!({
                "id": category.id(),
                "title": text(&category.title_key()),
                "sections": sections,
            })
        })
        .collect();
    let keys: Vec<Value> = values
        .iter()
        .filter_map(|value| {
            let def = registry.get(&value.key)?;
            let protected = def.protected_from_mcp
                || rb_mcp::config_edit::write_refusal(&def.key, &Value::Null).is_some();
            let mut entry = json!({
                "key": def.key,
                "category": def.category.id(),
                "section": def.section,
                "type": def.schema.get("type").cloned().unwrap_or(Value::Null),
                "value": masked_setting(&def.key, &value.value),
                "origin": value.origin,
                "default": masked_setting(&def.key, &def.default_value()),
                "label": text(&def.label_key),
                "description": text(&def.description_key),
                "help": text(&def.help_key),
                "scopes": def.scope_allowed,
                "localOnly": def.local_only,
                "protected": protected,
            });
            if let (Some(options), Some(object)) = (def.schema.get("enum"), entry.as_object_mut()) {
                object.insert("enum".to_owned(), options.clone());
            }
            if let (Some(plugin), Some(object)) =
                (registry.plugin_of(&def.key), entry.as_object_mut())
            {
                object.insert("plugin".to_owned(), Value::String(plugin.to_owned()));
            }
            Some(entry)
        })
        .collect();
    masked(&json!({ "categories": categories, "keys": keys }))
}

/// The value of the setting `key` masked as if it sat under the key's last segment: a header
/// map (`emulation.headers`) masks every header, a secret-named key its whole value. The
/// document is masked again as a whole, which covers secrets inside any other value.
pub fn masked_setting(key: &str, value: &Value) -> Value {
    let name = key.rsplit('.').next().unwrap_or(key);
    let mut wrapped = serde_json::Map::new();
    wrapped.insert(name.to_owned(), value.clone());
    match masked(&Value::Object(wrapped)) {
        Value::Object(mut map) => map.remove(name).unwrap_or(Value::Null),
        _ => Value::Null,
    }
}

/// The values the settings window reads now, in its order.
pub fn values(core: &Core) -> Vec<SettingValue> {
    let asked = ShellToCore::GetSettings {
        include_schema: false,
    };
    match crate::settings_window::ipc(core, &asked) {
        Some(Ok(CoreToShell::Settings { values, .. })) => values,
        _ => Vec::new(),
    }
}

/// The settings document of `core`.
pub fn read(core: &Core) -> Value {
    document(core.settings_registry(), &values(core), &|key| {
        rb_i18n::t(key)
    })
}

/// The value of `key` now, for the audit (`null` when the key has none).
pub fn current(core: &Core, key: &str) -> Value {
    values(core)
        .into_iter()
        .find(|value| value.key == key)
        .map_or(Value::Null, |value| value.value)
}

/// `value` of the setting `key` as the audit line shows it: masked, compact, at most
/// [`AUDIT_CHARS`] characters.
pub fn audit_text(key: &str, value: &Value) -> String {
    let text = masked_setting(key, value).to_string();
    if text.chars().count() <= AUDIT_CHARS {
        return text;
    }
    let cut: String = text.chars().take(AUDIT_CHARS).collect();
    format!("{cut}...")
}

/// The notice that MCP changed `key`, linking to it.
pub fn notice(key: &str) -> CoreToShell {
    CoreToShell::Notification(Notification {
        level: NotificationLevel::Info,
        message_key: CHANGED_KEY.to_owned(),
        args: BTreeMap::from([("key".to_owned(), key.to_owned())]),
        settings_key: Some(key.to_owned()),
    })
}

/// Records that MCP changed `key` in `scope` from `old` to `new`: the log line always, the
/// notice while `mcp.notifyOnSettingChange` is on.
pub fn record(core: &Core, key: &str, scope: SettingScope, old: &Value, new: &Value) {
    tracing::info!(
        target: "rbrowser::mcp::audit",
        key,
        ?scope,
        old = %audit_text(key, old),
        new = %audit_text(key, new),
        "MCP changed a setting"
    );
    if core.user_settings().mcp.notify_on_setting_change {
        core.notify(notice(key));
    }
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
