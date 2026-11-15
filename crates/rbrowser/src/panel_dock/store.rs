//! The dock's settings kept (RB-080-57): `panels.dock`, `panels.dockSize.<side>` and
//! `panels.dockState` (RB-080-93) in the user settings in effect, in the settings file (off the
//! calling thread, every other key kept, RB-080-61's `panels.dockCollapsed` dropped once
//! `dockState` is written) and named to the shell pages with `settingsChanged`, as the DevTools
//! splitter keeps its size.

use rb_config::{PanelSettings, UserSettings};
use rb_ipc::CoreToShell;
use serde_json::{Value, json};

use super::PanelSide;
use crate::control::Core;

/// `panels.dockState`.
const STATE_KEY: &str = "panels.dockState";

/// The keys of `panels` that differ between `before` and `after`, with their new values.
#[must_use]
pub fn changed(before: &PanelSettings, after: &PanelSettings) -> Vec<(String, Value)> {
    let mut keys = Vec::new();
    if before.dock != after.dock {
        keys.push(("panels.dock".to_owned(), json!(after.dock)));
    }
    if before.dock_state != after.dock_state {
        keys.push((STATE_KEY.to_owned(), json!(after.dock_state)));
    }
    for side in PanelSide::ALL {
        let (old, new) = (
            side.stored_in(&before.dock_size),
            side.stored_in(&after.dock_size),
        );
        if old != new {
            keys.push((format!("panels.dockSize.{}", side.key()), json!(new)));
        }
    }
    keys
}

/// Makes `user` the settings in effect when its `panels` changed, writes the changed keys into
/// the settings file and tells the shell pages.
pub fn keep(core: &Core, user: UserSettings) {
    let keys = changed(&core.user_settings().panels, &user.panels);
    if keys.is_empty() {
        return;
    }
    core.replace_user_settings(user);
    if let Some(path) = core.user_settings_path().map(std::path::Path::to_path_buf) {
        let written = keys.clone();
        core.runtime().spawn_blocking(move || {
            let result = crate::settings_window::user_file::update(&path, |root| {
                for (key, value) in written {
                    if key == STATE_KEY {
                        drop_legacy(root);
                    }
                    set_path(root, &key, value);
                }
            });
            if let Err(error) = result {
                tracing::warn!(%error, "the plugin panels' dock was not saved");
            }
        });
    }
    core.send(&CoreToShell::SettingsChanged {
        keys: keys.into_iter().map(|(key, _)| key).collect(),
    });
}

/// Drops RB-080-61's `panels.dockCollapsed`: `panels.dockState` says it now (RB-080-93).
fn drop_legacy(root: &mut serde_json::Map<String, Value>) {
    if let Some(Value::Object(panels)) = root.get_mut("panels") {
        panels.remove("dockCollapsed");
    }
}

/// Sets the dotted `key` in `root`, making (or replacing) the objects on the way.
pub fn set_path(root: &mut serde_json::Map<String, Value>, key: &str, value: Value) {
    let mut node = root;
    let mut segments = key.split('.').peekable();
    while let Some(segment) = segments.next() {
        if segments.peek().is_none() {
            node.insert(segment.to_owned(), value);
            return;
        }
        let entry = node.entry(segment).or_insert_with(|| json!({}));
        if !entry.is_object() {
            *entry = json!({});
        }
        let Value::Object(next) = entry else {
            return;
        };
        node = next;
    }
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;
