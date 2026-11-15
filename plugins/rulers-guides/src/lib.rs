//! Rulers and guides (RB-060-05): rulers at the edges of every view, guides dragged out of them,
//! a column overlay per breakpoint and a baseline grid - the same in every view.
//!
//! **Where the work happens.** The page script (`page/rulers.js`) draws in each view, in the
//! plugin's isolated world, inside a closed shadow root that takes no pointer events but on the
//! rulers; it measures the page and asks this component for the drawing. The component holds the
//! model: the guides, the column setups and their presets, the baseline grid, what is shown, the
//! two layers (the shared settings, the person's overrides in `.rbrowser/out/rulers-guides/`) and
//! the arithmetic - guide units, snapping, the columns of a breakpoint. Every change is posted
//! to every view's page script and to the panel (`$post`), so a guide dragged in one view appears
//! in all of them.
//!
//! Contributions (`manifest.toml`):
//!
//! - commands `toggle` (rulers, `Ctrl+Shift+Space O R`), `grid` (baseline grid, `O G`),
//!   `columns` (column overlay, `O C`), each with `{"on": bool}` to set instead of toggle;
//! - the MCP tool `rulers_set` ([`set`]), covering the commands;
//! - the page script's requests ([`page`]); the panel `guides` (a dialog) uses `rulers_set`.
//!
//! The logic is tested natively against `rb_plugin_api::test_support::MemoryHost`; `guest.rs`
//! translates it into the WIT vocabulary and exists on `wasm32` alone.

#[cfg(target_arch = "wasm32")]
mod guest;

pub mod columns;
pub mod guides;
pub mod layout;
pub mod model;
pub mod page;
pub mod set;
pub mod store;

use rb_plugin_api::feature::Feature;
use rb_plugin_api::{Failure, FailureKind, PluginHost};
use serde::Deserialize;
use serde_json::{Value, json};

use model::{Overrides, Shared, effective};

/// The plugin's id, as in `manifest.toml`.
pub const PLUGIN_ID: &str = "rulers-guides";
/// The tool a page script's request reaches `call-tool` with (`rb_plugin_host::PAGE_MESSAGE_TOOL`).
pub const PAGE_TOOL: &str = "@page";
/// The MCP tool `rulers_set`.
pub const SET_TOOL: &str = "set";

/// `plugin.rulers-guides.errors.bad_args` naming what does not fit.
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

/// After a change: writes the overrides (a failure is logged - without a project the views keep
/// the change for the session) and answers `extra` with `state` and `saved`, posting the state
/// and the overrides to every view's page script and the state to the panel.
///
/// # Errors
///
/// None today; the signature leaves room for the answer's encoding.
pub fn changed(
    host: &mut dyn PluginHost,
    shared: &Shared,
    overrides: &Overrides,
    extra: Value,
) -> Result<String, Failure> {
    let saved = match store::save(host, overrides) {
        Ok(()) => true,
        Err(failure) => {
            let code = failure.code.unwrap_or_default();
            host.log("warn", &format!("state.json not written: {code}"));
            false
        }
    };
    let state = effective(shared, overrides);
    let mut answer = match extra {
        Value::Object(map) => Value::Object(map),
        _ => json!({}),
    };
    answer["state"] = json!(state);
    answer["saved"] = json!(saved);
    answer["$post"] = json!([
        { "target": "all", "message": { "state": state, "overrides": overrides } },
        { "target": "panel", "message": { "state": state } },
    ]);
    Ok(answer.to_string())
}

/// A command's arguments: `{"on": bool}` sets instead of toggling; other fields are ignored.
#[derive(Deserialize, Default)]
#[serde(default)]
struct ToggleArgs {
    on: Option<bool>,
}

/// `toggle`, `grid`, `columns`: switches the rulers, the baseline grid or the column overlay.
fn toggle(host: &mut dyn PluginHost, command: &str, args: &str) -> Result<String, Failure> {
    let args: ToggleArgs = if args.trim().is_empty() {
        ToggleArgs::default()
    } else {
        serde_json::from_str(args).map_err(|_| bad_args("arguments"))?
    };
    let mut overrides = store::overrides(host, None);
    let shared = store::shared(host);
    let now = effective(&shared, &overrides).visible;
    let (field, current) = match command {
        "toggle" => (&mut overrides.visible.rulers, now.rulers),
        "grid" => (&mut overrides.visible.baseline, now.baseline),
        _ => (&mut overrides.visible.columns, now.columns),
    };
    *field = Some(args.on.unwrap_or(!current));
    changed(host, &shared, &overrides, json!({}))
}

/// The plugin's `feature` export.
#[derive(Clone, Copy, Debug, Default)]
pub struct RulersGuides;

impl Feature for RulersGuides {
    fn run_command(
        &mut self,
        host: &mut dyn PluginHost,
        command: &str,
        args: &str,
    ) -> Result<String, Failure> {
        match command {
            "toggle" | "grid" | "columns" => toggle(host, command, args),
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
            PAGE_TOOL => page::handle(host, args),
            other => Err(unknown(other)),
        }
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
