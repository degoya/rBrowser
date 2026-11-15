//! Which native dialog fills a path (`x-input`, RB-080-20): every string of the schemas that
//! names a file or a folder gets `"file"` or `"folder"`, the settings window shows Browse beside
//! it, and the lint (`scripts/check-settings.sh`) refuses a path key without it. The tables name
//! keys and nested fields alike: `extensions[].path` is the `path` of the list's items. A path
//! whose choices the core finds at runtime (`engines.firefox.path`: the installed Firefox
//! editions, RB-080-32) also gets `"x-options": "dynamic"`.

use rb_settings::{X_INPUT, X_OPTIONS};
use serde_json::{Map, Value};

/// The dialog a string is filled from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Input {
    /// One existing file (a program).
    File,
    /// One existing folder.
    Folder,
}

impl Input {
    /// The value of `x-input`.
    pub const fn id(self) -> &'static str {
        match self {
            Input::File => "file",
            Input::Folder => "folder",
        }
    }
}

/// The value of `x-options` on a path whose choices the core finds at runtime.
pub const DYNAMIC_OPTIONS: &str = "dynamic";

/// One path of the schema and its dialog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InputDef {
    /// The dotted path: a key (`engines.firefox.path`) or a field below one, `[]` for a
    /// list's items (`extensions[].path`).
    pub path: &'static str,
    pub input: Input,
    /// The core offers the choices at runtime (`"x-options": "dynamic"`).
    pub dynamic_options: bool,
}

const fn file(path: &'static str) -> InputDef {
    InputDef {
        path,
        input: Input::File,
        dynamic_options: false,
    }
}

const fn folder(path: &'static str) -> InputDef {
    InputDef {
        path,
        input: Input::Folder,
        dynamic_options: false,
    }
}

const fn file_found(path: &'static str) -> InputDef {
    InputDef {
        path,
        input: Input::File,
        dynamic_options: true,
    }
}

/// The paths of the project config.
pub const PROJECT_INPUTS: &[InputDef] = &[
    folder("capture.dir"),
    folder("automation.recorder.dir"),
    folder("automation.recorderExport.dir"),
    // A recommended extension's unpacked folder (RB-080-06).
    folder("extensions[].path"),
];

/// The paths of the user settings.
pub const USER_INPUTS: &[InputDef] = &[
    folder("automation.flows.dir"),
    file("engines.nodePath"),
    folder("engines.playwrightBrowsersPath"),
    // The installed Firefox editions, offered by the core (RB-080-32).
    file_found("engines.firefox.path"),
    folder("engines.firefox.profileDir"),
    folder("engines.chromium.extensions.unpacked[].path"),
    file("wsl.helperPath"),
    file("tools.ffmpegPath"),
    file("engines.lighthouseBrowserPath"),
];

/// Writes `x-input` (and `x-options: "dynamic"` where the core offers the choices) at every path
/// of `defs` in the root `schema`; the paths it cannot find come back as the error.
pub fn annotate_inputs(schema: &mut Value, defs: &[InputDef]) -> Result<(), Vec<&'static str>> {
    let mut missing = Vec::new();
    for def in defs {
        match node_mut(schema, def.path) {
            Some(map) => {
                map.insert(X_INPUT.to_owned(), Value::from(def.input.id()));
                if def.dynamic_options {
                    map.insert(X_OPTIONS.to_owned(), Value::from(DYNAMIC_OPTIONS));
                }
            }
            None => missing.push(def.path),
        }
    }
    if missing.is_empty() {
        Ok(())
    } else {
        Err(missing)
    }
}

/// The schema object at dotted `path` (`properties` at each level, `items` after `[]`).
fn node_mut<'a>(schema: &'a mut Value, path: &str) -> Option<&'a mut Map<String, Value>> {
    let mut node = schema;
    for segment in path.split('.') {
        let (name, items) = match segment.strip_suffix("[]") {
            Some(name) => (name, true),
            None => (segment, false),
        };
        node = node.get_mut("properties")?.get_mut(name)?;
        if items {
            node = node.get_mut("items")?;
        }
    }
    node.as_object_mut()
}
