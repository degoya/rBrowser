//! Screenshot editor (RB-080-01): annotates, redacts and crops a screenshot of `.rbrowser/out/`
//! and writes the result beside its annotations, never over the original.
//!
//! **Where the work happens.** The panel (`panel/`, a dialog) draws the preview on a canvas and
//! keeps the editing state - tools, undo and redo, zoom, the clipboard. This component renders
//! the file: it reads the original, checks the annotations ([`model`]), draws them
//! ([`render`], [`text`]), renders redactions into the pixels ([`redact`]), crops, encodes
//! without metadata ([`codec`]) and writes `<name>.edited.png` and `<name>.annotations.json`
//! below `.rbrowser/out/screenshot-editor/` ([`paths`]). The same tool serves MCP, so an agent
//! annotates without any UI.
//!
//! Contributions (`manifest.toml`):
//!
//! - the commands `open_last` (`Ctrl+Shift+Space C E`, opens the editor at the newest screenshot)
//!   and `copy` (`Ctrl+Shift+Space C K`, has the open editor put its image on the clipboard);
//! - the MCP tools `shot_annotate`, `shot_load` and `shot_list` ([`tools`]), which the panel uses
//!   as well;
//! - the panel `editor`.
//!
//! The logic is tested natively against `rb_plugin_api::test_support::MemoryHost`; `guest.rs`
//! translates it into the WIT vocabulary and exists on `wasm32` alone.

#[cfg(target_arch = "wasm32")]
mod guest;

pub mod cache;
pub mod codec;
pub mod files;
pub mod model;
pub mod paths;
pub mod redact;
pub mod render;
pub mod settings;
pub mod text;
pub mod tools;

#[cfg(test)]
mod testing;

use rb_plugin_api::feature::Feature;
use rb_plugin_api::{Failure, FailureKind, PluginHost};
use serde_json::{Value, json};

/// The plugin's id, as in `manifest.toml`.
pub const PLUGIN_ID: &str = "screenshot-editor";
/// The editor panel's id.
pub const PANEL: &str = "editor";

/// `plugin.screenshot-editor.errors.bad_args` naming what does not fit.
#[must_use]
pub fn bad_args(reason: &str) -> Failure {
    Failure::coded(
        FailureKind::BadData,
        &format!("plugin.{PLUGIN_ID}.errors.bad_args"),
    )
    .with_param("reason", reason.chars().take(64).collect::<String>())
}

fn unknown(name: &str) -> Failure {
    Failure::coded(FailureKind::NotFound, "plugins.error.unknown_contribution")
        .with_param("name", name.chars().take(64).collect::<String>())
}

/// Opens the editor panel and posts `message` to it (an editor already open acts at once; one
/// that opens now loads the newest screenshot by itself).
fn open_panel(message: Value, extra: Value) -> String {
    let mut answer = match extra {
        Value::Object(map) => Value::Object(map),
        _ => json!({}),
    };
    answer["$open"] = json!({ "panel": PANEL });
    answer["$post"] = json!([{ "target": "panel", "message": message }]);
    answer.to_string()
}

/// `open_last`: the editor at the newest screenshot (none: the editor with its file list).
fn open_last(host: &mut dyn PluginHost) -> Result<String, Failure> {
    let image = files::last(host)?;
    Ok(open_panel(
        json!({ "open": image }),
        json!({ "image": image }),
    ))
}

/// The plugin's `feature` export.
#[derive(Clone, Copy, Debug, Default)]
pub struct ScreenshotEditor;

impl Feature for ScreenshotEditor {
    fn run_command(
        &mut self,
        host: &mut dyn PluginHost,
        command: &str,
        _args: &str,
    ) -> Result<String, Failure> {
        match command {
            "open_last" => open_last(host),
            // The clipboard is the panel's: only a document the person uses may write it.
            "copy" => Ok(open_panel(json!({ "copy": true }), json!({}))),
            other => Err(unknown(other)),
        }
    }

    fn call_tool(
        &mut self,
        host: &mut dyn PluginHost,
        tool: &str,
        args: &str,
    ) -> Result<String, Failure> {
        match tool {
            "annotate" => tools::annotate(host, args),
            "load" => tools::load(host, args),
            "list" => tools::list(host, args),
            other => Err(unknown(other)),
        }
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
