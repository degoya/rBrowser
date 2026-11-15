//! The title of a command for the status bar (RB-080-58): a failed command is named the way the
//! palette names it, not by its id. The shell reads a plugin command's title message from here
//! too (RB-080-82).

use rb_commands::{CommandId, CommandRegistry};

/// The title of command `id` in the UI language: a plugin command's from its own plugin's
/// catalogue, a core command's from the core's; the id itself when the command or its text is
/// unknown.
#[must_use]
pub fn command_title(registry: &CommandRegistry, id: &str) -> String {
    let Some(def) = CommandId::new(id)
        .ok()
        .and_then(|id| registry.get(&id).cloned())
    else {
        return id.to_owned();
    };
    let key = def.title_key.as_str();
    let text = if id.starts_with("plugin.") {
        plugin_title_message(id, key).map(|text| rb_i18n::format_message(&text, &[]))
    } else {
        Some(rb_i18n::t(key)).filter(|text| text != key)
    };
    text.unwrap_or_else(|| id.to_owned())
}

/// The title message (unformatted) of plugin command `id` titled `title_key`, from its own
/// plugin's catalogue only: a command `plugin.<id, `-` as `_`>.<command>` reads below
/// `plugin.<id>.`. `None` for a core command, another plugin's key or an unknown text. The
/// shell gets these beside the command list, since its catalogue is the core's (RB-080-82).
#[must_use]
pub fn plugin_title_message(id: &str, title_key: &str) -> Option<String> {
    let (segment, _) = id.strip_prefix("plugin.")?.split_once('.')?;
    let (plugin, _) = title_key.strip_prefix("plugin.")?.split_once('.')?;
    if plugin.replace('-', "_") != segment {
        return None;
    }
    rb_i18n::plugin_text(plugin, title_key)
}

#[cfg(test)]
#[path = "title_tests.rs"]
mod tests;
