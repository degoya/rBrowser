//! The plugins granted `network-rules` (RB-070-02): every command and tool call gets the host's
//! `$network` state (this run's session, workspace trust, the plugin's hits and rejected rules,
//! the cache switch, the views and whether each is intercepted) - a `$network` the caller sent is
//! overwritten - and after each such call, at the start and whenever trust or the settings
//! change, the host asks the plugin for its rules (`@rules`, [`rb_plugin_host::NETWORK_RULES_TOOL`])
//! and applies the answer ([`crate::network_rules`]). No WASM runs per request.

use serde_json::{Value, json};

use super::runtime::Component;
use crate::network_rules::STATE_KEY;
use crate::network_rules::state::Rules;

/// `args` with `state` under [`STATE_KEY`], over anything the caller put there; `null` becomes
/// an object, other non-objects stay as they are.
#[must_use]
pub fn with_state(args: &Value, state: Value) -> Value {
    match args {
        Value::Object(map) => {
            let mut map = map.clone();
            map.insert(STATE_KEY.to_owned(), state);
            Value::Object(map)
        }
        Value::Null => json!({ STATE_KEY: state }),
        other => other.clone(),
    }
}

/// Asks `plugin`'s component for its rules and applies them, all as project rules for a
/// `project` plugin. A failed request keeps the rules that apply (trust is enforced by `rules`
/// either way); an answer that does not read drops them.
pub async fn sync(component: &dyn Component, plugin: &str, rules: &Rules, project: bool) {
    let args = json!({ STATE_KEY: rules.state(plugin) }).to_string();
    let answer = match component.network_rules(&args).await {
        Ok(answer) => answer,
        Err(failure) => {
            tracing::warn!(
                plugin_id = plugin,
                code = failure.code().unwrap_or_default(),
                "a plugin's network rules were not answered"
            );
            return;
        }
    };
    let document = serde_json::from_str::<Value>(&answer).unwrap_or(Value::Null);
    let applied = if project {
        rules.apply_from_project(plugin, &document)
    } else {
        rules.apply(plugin, &document)
    };
    match applied {
        Ok(rejected) if !rejected.is_empty() => {
            tracing::info!(
                plugin_id = plugin,
                rejected = rejected.len(),
                "network rules rejected"
            );
        }
        Ok(_) => {}
        Err(reason) => {
            tracing::warn!(plugin_id = plugin, %reason, "a plugin's network rules do not read");
        }
    }
}

#[cfg(test)]
#[path = "network_tests.rs"]
mod tests;
