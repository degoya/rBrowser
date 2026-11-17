//! `plugins.resume` (RB-090-59): a plugin paused after repeated failures runs again. With
//! `{"id": "<plugin>"}` - the Resume button of a paused row in Settings > Plugins - that plugin
//! alone; without one - the palette, the shortcut - every paused plugin, with a notice that
//! names them. Each plugin has its own health ([`rb_plugin_host::PluginHealths`]), so a resume
//! never touches another plugin. A person's action: MCP has no tool for it (protected).

use rb_commands::CommandError;
use rb_ipc::{Notification, NotificationLevel};
use rb_plugin_host::PluginHealths;
use serde_json::{Value, json};

/// The notice after a resume from the palette; placeholder `{plugins}`.
pub const RESUMED_KEY: &str = "plugins.notice.resumed";
/// The notice when no plugin was paused.
pub const NONE_PAUSED_KEY: &str = "plugins.notice.none_paused";

/// Resumes plugin `args.id`, or every paused plugin without an id; answers
/// `{"resumed": [ids]}`, the plugins that were paused and run again.
///
/// # Errors
///
/// `id` is there and no text.
pub fn resume(healths: &PluginHealths, args: &Value) -> Result<Value, CommandError> {
    let resumed = match args.get("id") {
        None | Some(Value::Null) => healths.resume_all(),
        Some(Value::String(id)) => {
            if healths.suspended().iter().any(|paused| paused == id) && healths.resume(id) {
                vec![id.clone()]
            } else {
                Vec::new()
            }
        }
        Some(_) => {
            return Err(CommandError::InvalidArgs(
                "`id` names a plugin by its id".to_owned(),
            ));
        }
    };
    for id in &resumed {
        tracing::info!(plugin_id = id.as_str(), "a paused plugin was resumed");
    }
    Ok(json!({ "resumed": resumed }))
}

/// What the person sees after a resume without an id: the plugins resumed, or that none was
/// paused.
#[must_use]
pub fn notice(resumed: &[String]) -> Notification {
    let (message_key, args) = if resumed.is_empty() {
        (NONE_PAUSED_KEY, Default::default())
    } else {
        (
            RESUMED_KEY,
            [("plugins".to_owned(), resumed.join(", "))].into(),
        )
    };
    Notification {
        level: NotificationLevel::Info,
        message_key: message_key.to_owned(),
        args,
        settings_key: None,
    }
}

#[cfg(test)]
#[path = "resume_tests.rs"]
mod tests;
