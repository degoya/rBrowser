//! The shell pages learn when the panels offered change (RB-080-97): the panels page asks for
//! the list (`pluginPanel` `list`) as it loads - since the icon rail (RB-080-93) at the start,
//! while the plugins still load and the list is refused with `plugins.error.not_available`. The
//! window's plugin runtime, once started, and every change of what the panels offered depend on
//! since - the hidden panels (`plugins.panelsHidden`) and the project plugins' trust; a window
//! loads its plugins once - send the `pluginPanel` event [`changed_message`] to every shell page;
//! a panels page asks for the list again. What was told last lives beside the window's runtime
//! (`plugins::registry`).

use std::collections::BTreeSet;

use rb_config::UserSettings;
use serde_json::{Value, json};

/// The `pluginPanel` event that asks a panels page for the list again.
#[must_use]
pub fn changed_message() -> Value {
    json!({ "panelsChanged": true })
}

/// What a window's panels offered depend on beside its loaded plugins.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Inputs {
    hidden: BTreeSet<String>,
    project_trusted: bool,
}

impl Inputs {
    /// The hidden panels of `user` and the project plugins' trust in effect.
    #[must_use]
    pub fn of(user: &UserSettings, project_trusted: bool) -> Self {
        Self {
            hidden: user
                .plugins
                .panels_hidden
                .iter()
                .map(|id| id.0.clone())
                .collect(),
            project_trusted,
        }
    }
}

/// Keeps `now` in `last`; whether it is news - the first, or another than the last.
pub fn take(last: &mut Option<Inputs>, now: Inputs) -> bool {
    if last.as_ref() == Some(&now) {
        return false;
    }
    *last = Some(now);
    true
}

#[cfg(test)]
#[path = "panel_list_tests.rs"]
mod tests;
