//! Image overlay (RB-070-09): each view's design image over the page - opacity, blend modes,
//! invert, offset, a split view - with one set of settings for every view.
//!
//! **Where the work happens.** The page script (`page/image-overlay.js`) draws in each view, in
//! the plugin's isolated world, inside closed shadow roots of `rbrowser-image-overlay` elements
//! (the sync agent ignores them) that take no pointer events but on the split handle. It asks this
//! component which design its view shows and fetches the file in chunks of [`CHUNK_BYTES`]
//! (base64 in the answer, at most 64 KiB) - no URL, no markup: the page script decodes the bytes
//! into an `ImageBitmap` and draws it on a canvas. The component holds the rest: which design a
//! view shows ([`select`]), the checks of path and file ([`path`], [`image`]), the state with its
//! two layers ([`state`], [`store`]). Every change is posted to every view and the panel.
//!
//! Contributions (`manifest.toml`):
//!
//! - the command `toggle` (`Ctrl+Shift+Space O D`), with `{"on": bool}` to set;
//! - the MCP tool `image_overlay_set` ([`set`]), covering it, which the panel uses as well;
//! - the MCP tool `imageoverlay_designs` ([`designs`]): the designs with their sizes, and a
//!   design's bytes in chunks for the panel's previews;
//! - the command `add_design` (`Ctrl+Shift+Space O N`, RB-080-74): asks the application to open
//!   the native file dialog and copy the chosen image into `.rbrowser/designs/` (a `host` post -
//!   neither a panel nor this component can open a dialog or write into the project); the
//!   application then chooses it through `set`. No MCP tool: a dialog is a person's;
//! - the page script's requests ([`page`]); the panel `designs` (a dialog).
//!
//! The logic is tested natively against `rb_plugin_api::test_support::MemoryHost`; `guest.rs`
//! translates it into the WIT vocabulary and exists on `wasm32` alone.

#[cfg(target_arch = "wasm32")]
mod guest;

pub mod cache;
pub mod designs;
pub mod image;
pub mod page;
pub mod path;
pub mod select;
pub mod set;
pub mod state;
pub mod store;
pub mod thumbs;

use rb_plugin_api::feature::Feature;
use rb_plugin_api::{Failure, FailureKind, PluginHost};
use serde::Deserialize;
use serde_json::{Value, json};

use state::{Overrides, Shared, State, effective, reload};

/// The plugin's id, as in `manifest.toml`.
pub const PLUGIN_ID: &str = "image-overlay";
/// The tool a page script's request reaches `call-tool` with (`rb_plugin_host::PAGE_MESSAGE_TOOL`).
pub const PAGE_TOOL: &str = "@page";
/// The MCP tool `image_overlay_set`.
pub const SET_TOOL: &str = "set";
/// The MCP tool `imageoverlay_designs`.
pub const DESIGNS_TOOL: &str = "designs";
/// The application's action that adds a design (`crates/rbrowser/src/plugins/host_actions.rs`).
pub const ADD_DESIGN_ACTION: &str = "add_design";
/// The bytes of a design one chunk carries: as base64 in its answer it stays below the 64 KiB a
/// page script's answer may have.
pub const CHUNK_BYTES: usize = 47 * 1024;

/// `plugin.image-overlay.errors.bad_args` naming what does not fit.
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

/// After a change from `before`: writes the overrides (a failure is logged - without a project
/// the views keep the change for the session) and answers `extra` with `state` and `saved`,
/// posting the state, the overrides and whether to load the image again to every view's page
/// script, and the state to the panel.
pub fn changed(
    host: &mut dyn PluginHost,
    before: &State,
    shared: &Shared,
    overrides: &Overrides,
    extra: Value,
) -> String {
    let saved = match store::save(host, overrides) {
        Ok(()) => true,
        Err(failure) => {
            let code = failure.code.unwrap_or_default();
            host.log("warn", &format!("state.json not written: {code}"));
            false
        }
    };
    let state = effective(shared, overrides);
    let reload = reload(before, &state);
    let mut answer = match extra {
        Value::Object(map) => Value::Object(map),
        _ => json!({}),
    };
    answer["state"] = json!(state);
    answer["saved"] = json!(saved);
    answer["$post"] = json!([
        { "target": "all", "message": { "state": state, "overrides": overrides, "reload": reload } },
        { "target": "panel", "message": { "state": state, "reload": reload } },
    ]);
    answer.to_string()
}

/// The command's arguments: `{"on": bool}` sets instead of toggling; other fields are ignored.
#[derive(Deserialize, Default)]
#[serde(default)]
struct ToggleArgs {
    on: Option<bool>,
}

/// `toggle`: shows or hides the design in every view.
fn toggle(host: &mut dyn PluginHost, args: &str) -> Result<String, Failure> {
    let args: ToggleArgs = if args.trim().is_empty() {
        ToggleArgs::default()
    } else {
        serde_json::from_str(args).map_err(|_| bad_args("arguments"))?
    };
    let mut overrides = store::overrides(host, None);
    let shared = store::shared(host);
    let before = effective(&shared, &overrides);
    overrides.enabled = Some(args.on.unwrap_or(!before.enabled));
    Ok(changed(host, &before, &shared, &overrides, json!({})))
}

/// `add_design`: the application opens the file dialog (a `host` post); the status bar says so
/// until the outcome replaces it.
fn add_design() -> String {
    json!({
        "started": true,
        "$post": [
            {
                "target": "status",
                "message": { "key": format!("plugin.{PLUGIN_ID}.status.choose_design"), "level": "info" },
            },
            { "target": "host", "message": { "action": ADD_DESIGN_ACTION } },
        ],
    })
    .to_string()
}

/// The plugin's `feature` export.
#[derive(Clone, Copy, Debug, Default)]
pub struct ImageOverlay;

impl Feature for ImageOverlay {
    fn run_command(
        &mut self,
        host: &mut dyn PluginHost,
        command: &str,
        args: &str,
    ) -> Result<String, Failure> {
        match command {
            "toggle" => toggle(host, args),
            "add_design" => Ok(add_design()),
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
            SET_TOOL => set::run(host, args),
            DESIGNS_TOOL => designs::run(host, args),
            PAGE_TOOL => page::handle(host, args),
            other => Err(unknown(other)),
        }
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
