//! The hosted frontend's screencast (RB-080-21): `inspector.html` shows the inspected page once
//! more beside the panels, Chrome's `ScreencastApp`, on by default. While it shows, the frontend
//! makes it the overlay's highlighter: inspect mode never reaches the view, the picks land in
//! the scaled picture instead (and hit "something else" there). rBrowser keeps it off unless
//! `devtools.screencast` says otherwise.
//!
//! The frontend keeps its settings in the dock's `localStorage` (the stub `InspectorFrontendHost`
//! of a frontend without an embedder, read once at start). The script here writes the toggle's
//! setting before the frontend's own scripts run; it needs `Page.enable` on the dock's session,
//! or Blink's page agent never injects it.

/// The setting of the screencast toggle: `screencast-enabled` since the kebab-case settings
/// (Chrome 118), `screencastEnabled` before.
pub const SETTING_KEYS: [&str; 2] = ["screencast-enabled", "screencastEnabled"];

/// The script that writes the screencast's setting for the frontend about to load: `enabled`
/// as the frontend stores a boolean (`JSON.stringify`). Nothing may throw in it.
pub fn preference_script(enabled: bool) -> String {
    let keys = SETTING_KEYS
        .iter()
        .map(|key| format!("'{key}'"))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "try {{ for (const key of [{keys}]) {{ localStorage.setItem(key, '{enabled}'); }} }} catch (error) {{}}"
    )
}

#[cfg(test)]
#[path = "screencast_tests.rs"]
mod tests;
