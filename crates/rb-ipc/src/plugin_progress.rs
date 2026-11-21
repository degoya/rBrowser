//! A plugin's progress in the activity bar (RB-093-01): what `host.report-progress` of a running
//! plugin call said last - `done` of `total` steps - with the plugin's name and the command that
//! cancels it, when its manifest has one (`[[commands]] id = "cancel"`). The snapshot lists one
//! per plugin with a call running that reported progress; the entry goes when the plugin's last
//! running call ends.

use serde::{Deserialize, Serialize};

/// One plugin's progress.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginProgress {
    /// The plugin id (`width-sweep`).
    pub plugin: String,
    /// Its display name, as its locales give it (`Width sweep`).
    pub name: String,
    /// Steps done, never past `total`.
    pub done: u32,
    /// Steps in all; 0 when the plugin does not know yet.
    pub total: u32,
    /// The registry command that cancels the call (`plugin.width_sweep.cancel`); none when the
    /// plugin has no `cancel` command.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cancel: Option<String>,
}

#[cfg(test)]
#[path = "plugin_progress_tests.rs"]
mod tests;
