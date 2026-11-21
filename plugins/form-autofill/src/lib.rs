//! rBrowser's first-party form autofill (RB-070-11, `docs/plugins.md`): fills the forms of every
//! view with plausible valid values or deliberately invalid ones, from one seed, so every view
//! gets the same data - matched to each view's locale (German data for `de-*`, American English
//! otherwise). It never submits and never touches a file input.
//!
//! The page script `src/page.js` reads the fields and sets the values through `evaluate` in the
//! plugin's isolated world; the kind of each field, the values and the invalid strategies are
//! native Rust over [`PluginHost`], tested against a fake host; `src/guest.rs` translates it for
//! the component. "Take this form as a preset" (RB-090-70) reads a form's values the same way
//! (`src/capture.rs`) and stores what the panel or MCP saves in the plugin's own file
//! (`src/store.rs`); the panel renames and deletes one (`src/preset_edit.rs`, RB-090-71). A fill
//! names the fields it set to the recorder in `host` posts (`src/record.rs`, RB-090-78).
//! Grants: `views`, `evaluate`, `project-read` (`.rbrowser/` only) and `out-write`
//! (`.rbrowser/out/form-autofill/` only).

use rb_plugin_api::feature::Feature;
use rb_plugin_api::{Failure, FailureKind, PluginHost, codes};

pub mod args;
pub mod bounds;
pub mod capture;
pub mod classify;
pub mod field;
pub mod fill;
pub mod invalid;
pub mod passes;
pub mod pattern;
pub mod persona;
pub mod plan;
pub mod preset_edit;
pub mod record;
pub mod rng;
pub mod secret;
pub mod settings;
pub mod store;
pub mod value;

#[cfg(test)]
mod fake;

#[cfg(target_arch = "wasm32")]
mod guest;

use args::{Args, Mode};
use capture::CaptureArgs;
use preset_edit::NameArgs;
use store::SaveArgs;

/// The plugin's id.
pub const PLUGIN_ID: &str = "form-autofill";

/// The i18n key `plugin.form-autofill.<name>`.
#[must_use]
pub fn key(name: &str) -> String {
    format!("plugin.{PLUGIN_ID}.{name}")
}

/// The plugin: no state between calls (a store lives for one call).
#[derive(Clone, Copy, Debug, Default)]
pub struct FormAutofill;

fn unknown(name: &str) -> Failure {
    Failure::coded(FailureKind::NotFound, codes::UNKNOWN_CONTRIBUTION).with_param("name", name)
}

fn run(host: &mut dyn PluginHost, args: &str, mode: Option<Mode>) -> Result<String, Failure> {
    let args = Args::parse(args)?;
    let answer = fill::fill(host, &args, mode)?;
    Ok(fill::answer_with_post(answer).to_string())
}

impl Feature for FormAutofill {
    fn run_command(
        &mut self,
        host: &mut dyn PluginHost,
        command: &str,
        args: &str,
    ) -> Result<String, Failure> {
        match command {
            "fill_valid" => run(host, args, Some(Mode::Valid)),
            "fill_invalid" => run(host, args, Some(Mode::Invalid)),
            "capture" => Ok(capture::command(host, &CaptureArgs::parse(args)?)?.to_string()),
            _ => Err(unknown(command)),
        }
    }

    fn call_tool(
        &mut self,
        host: &mut dyn PluginHost,
        tool: &str,
        args: &str,
    ) -> Result<String, Failure> {
        match tool {
            "fill" => run(host, args, None),
            "capture" => Ok(capture::capture(host, &CaptureArgs::parse(args)?)?.to_string()),
            "save" => Ok(store::save(host, &SaveArgs::parse(args)?)?.to_string()),
            "presets" => Ok(store::list(host)?.to_string()),
            "rename" => Ok(preset_edit::rename(host, &NameArgs::parse(args)?)?.to_string()),
            "delete" => Ok(preset_edit::delete(host, &NameArgs::parse(args)?)?.to_string()),
            _ => Err(unknown(tool)),
        }
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
