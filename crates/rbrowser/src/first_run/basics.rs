//! The wizard's basics step for a project (RB-091-05): the layout it starts with and whether
//! input is synced, written into its `config.json` by the project's own keys - `layouts.default`
//! with a preset of the chosen mode, and the `sync` switches. The project then names its mode:
//! a later built-in default changes nothing (the canvas since RB-099-26, the grid before). The
//! theme is the user setting `ui.theme`, which the page writes itself.

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
/// The name of the preset a grid choice adds when the project has no grid preset (RB-099-26).
pub const GRID_PRESET: &str = "grid";

/// Applies the choices to the project config document `config` (an object).
pub fn apply(config: &mut Value, layout: Option<SetupLayout>, sync: Option<bool>) {
    let Some(config) = config.as_object_mut() else {
        return;
    };
    match layout {
        Some(SetupLayout::Canvas) => start_with(config, CANVAS_PRESET),
        Some(SetupLayout::Grid) => start_with(config, GRID_PRESET),
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

/// Whether `preset` is in `mode` and shows every view - no grid of its own, no hidden views -
/// so a project started in it shows the grid or canvas of every breakpoint. Its mode is read as
/// the config reads it: absent, the grid with a `layout` of its own, else the canvas (RB-099-26).
fn whole_in(preset: &Value, mode: &str) -> bool {
    let read = serde_json::from_value::<rb_config::LayoutPreset>(preset.clone()).ok();
    read.is_some_and(|read| {
        crate::layout::store::mode_name(read.mode) == mode
            && read.layout.is_none()
            && read.hidden.is_none()
    })
}

/// Starts the project in `mode` (`"canvas"` or `"grid"`): `layouts.default` names the first
/// preset of that mode over every view ([`whole_in`]), else a new one `{ "mode": <mode> }` named
/// after it (`-2`, `-3` ... when another preset has the name). A preset with its own few cells
/// (`phones`: two mobile views) would open the project with those alone.
fn start_with(config: &mut Map<String, Value>, mode: &str) {
    let layouts = object(config, "layouts");
    let presets = object(layouts, "presets");
    let found = presets
        .iter()
        .find(|(_, preset)| whole_in(preset, mode))
        .map(|(name, _)| name.clone());
    let name = found.unwrap_or_else(|| {
        let name = (1..)
            .map(|n| match n {
                1 => mode.to_owned(),
                n => format!("{mode}-{n}"),
            })
            .find(|name| !presets.contains_key(name))
            .expect("a free name among endless ones");
        presets.insert(name.clone(), json!({ "mode": mode }));
        name
    });
    layouts.insert("default".to_owned(), Value::String(name));
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
