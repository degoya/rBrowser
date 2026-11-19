//! The wizard's basics step for a project (RB-091-05): the layout it starts with and whether
//! input is synced, written into its `config.json` by the project's own keys - `layouts.default`
//! with a canvas preset, and the `sync` switches. The theme is the user setting `ui.theme`,
//! which the page writes itself.

use rb_ipc::SetupLayout;
use serde_json::{Map, Value, json};

/// The sync switches "sync off" turns off; on removes them, back to their defaults (on).
pub const SYNC_KEYS: [&str; 7] = [
    "navigation",
    "scroll",
    "click",
    "input",
    "keyboard",
    "hover",
    "forms",
];
/// The name of the preset a canvas choice adds when the project has no canvas preset.
pub const CANVAS_PRESET: &str = "canvas";

/// Applies the choices to the project config document `config` (an object).
pub fn apply(config: &mut Value, layout: Option<SetupLayout>, sync: Option<bool>) {
    let Some(config) = config.as_object_mut() else {
        return;
    };
    match layout {
        Some(SetupLayout::Canvas) => canvas(config),
        Some(SetupLayout::Grid) => grid(config),
        None => {}
    }
    if let Some(on) = sync {
        set_sync(config, on);
    }
}

fn object<'a>(parent: &'a mut Map<String, Value>, key: &str) -> &'a mut Map<String, Value> {
    let entry = parent
        .entry(key.to_owned())
        .or_insert_with(|| Value::Object(Map::new()));
    if !entry.is_object() {
        *entry = Value::Object(Map::new());
    }
    entry.as_object_mut().expect("made an object above")
}

fn mode_of(preset: &Value) -> &str {
    preset.get("mode").and_then(Value::as_str).unwrap_or("grid")
}

fn canvas(config: &mut Map<String, Value>) {
    let layouts = object(config, "layouts");
    let found = layouts
        .get("presets")
        .and_then(Value::as_object)
        .and_then(|presets| {
            presets
                .iter()
                .find(|(_, preset)| mode_of(preset) == "canvas")
                .map(|(name, _)| name.clone())
        });
    let name = match found {
        Some(name) => name,
        None => {
            object(layouts, "presets")
                .insert(CANVAS_PRESET.to_owned(), json!({ "mode": "canvas" }));
            CANVAS_PRESET.to_owned()
        }
    };
    layouts.insert("default".to_owned(), Value::String(name));
}

fn grid(config: &mut Map<String, Value>) {
    let Some(layouts) = config.get_mut("layouts").and_then(Value::as_object_mut) else {
        return;
    };
    let default_mode = layouts
        .get("default")
        .and_then(Value::as_str)
        .and_then(|name| layouts.get("presets").and_then(|presets| presets.get(name)))
        .map(|preset| mode_of(preset).to_owned());
    if default_mode.is_some_and(|mode| mode != "grid") {
        layouts.remove("default");
    }
}

fn set_sync(config: &mut Map<String, Value>, on: bool) {
    let sync = object(config, "sync");
    for key in SYNC_KEYS {
        if on {
            sync.remove(key);
        } else {
            sync.insert(key.to_owned(), Value::Bool(false));
        }
    }
    if on && sync.is_empty() {
        config.remove("sync");
    }
}

#[cfg(test)]
#[path = "basics_tests.rs"]
mod tests;
