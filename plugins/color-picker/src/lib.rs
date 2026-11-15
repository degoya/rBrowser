//! The colour picker (RB-070-10, `docs/plugins.md`): the rendered pixel under the cursor, the
//! WCAG contrast of two colours, the page's palette and the characters per line of its text - in
//! every view.
//!
//! - **Pipette** (command `pick`, `Ctrl+Shift+Space D I`; MCP `color_sample`, [`sample`]): the
//!   page script `page/picker.js` shows a crosshair and an 11 x 11 loupe, drawn from one tile
//!   where the pointer rests in its view ([`sample::tile`], RB-080-56: never a screenshot per
//!   move), cut here from the view's image as its window shows it ([`shot`], RB-080-64: no clip,
//!   which made the view flicker) where its size or the page script's markers place the viewport
//!   in it ([`calibrate`], RB-080-77, RB-090-19), decoded here ([`pixels`]), DPR-aware. Only
//!   Chromium views get the pipette. A click takes the colour, Shift+click takes it as the
//!   background of the contrast; arrows and Enter do the same by keyboard, Esc ends. Start and
//!   end reach the panel (`picking`, `picked`, `stopped`) and the status bar ([`notice`],
//!   RB-080-64), from the panel's button and the shortcut alike; `pick` with `{"stop": true}`
//!   ends it (the panel's second click).
//! - **Contrast** (MCP `color_contrast`, [`contrast`]): WCAG 2.x ratio, AA and AAA for normal and
//!   large text.
//! - **Palette** (MCP `color_palette`, [`palette`]): the computed `color`, `background-color` and
//!   `border-color` of every rendered element (`src/palette.js` through `evaluate`), grouped and
//!   sorted by use; a colour's uses can be marked in the views.
//! - **Text measure** (command `measure_text`, `Ctrl+Shift+Space D M`; MCP `color_measure_text`,
//!   [`measure`]): characters per line from `Range.getClientRects()` (`src/measure.js`), rated
//!   against the optimal 45-75 and marked above 80 (WCAG 1.4.8).
//! - **Panel** `picker` (command `panel`, `Ctrl+Shift+Space P I`): the colours in every format
//!   ([`convert`]) to copy, the contrast, the palette and the measurements per view.
//!
//! Chromium views only for now (`evaluate` and CDP); a Firefox or WebKit view answers
//! `plugin.color-picker.errors.unsupported_engine`. The logic is plain JSON against the native
//! [`PluginHost`]; `guest.rs` translates it into the contract's vocabulary on `wasm32` alone.

#[cfg(target_arch = "wasm32")]
mod guest;

pub mod calibrate;
pub mod color;
pub mod contrast;
pub mod convert;
pub mod measure;
pub mod page;
pub mod palette;
pub mod pixels;
pub mod sample;
pub mod settings;
pub mod shot;
pub mod targets;

#[cfg(test)]
mod testing;

use rb_plugin_api::feature::Feature;
use rb_plugin_api::{Failure, FailureKind, PluginHost, codes};
use serde_json::{Value, json};

use crate::settings::Settings;

/// The plugin's id: the prefix of its keys.
pub const PLUGIN_ID: &str = "color-picker";
/// The panel the command `panel` shows.
pub const PANEL_ID: &str = "picker";
/// The tool a page script's request reaches `call-tool` with.
pub const PAGE_TOOL: &str = "@page";

/// The i18n key `plugin.color-picker.<name>`.
#[must_use]
pub fn key(name: &str) -> String {
    format!("plugin.{PLUGIN_ID}.{name}")
}

/// `plugins.error.invalid` naming the argument that does not hold.
#[must_use]
pub fn invalid(reason: &str) -> Failure {
    let reason: String = reason.chars().take(64).collect();
    Failure::coded(FailureKind::BadData, codes::INVALID).with_param("reason", reason)
}

/// A browser answer that does not read (`plugins.error.invalid`, `reason`).
#[must_use]
pub fn bad_data(reason: &str) -> Failure {
    invalid(reason)
}

/// The plugin's own failure `plugin.color-picker.errors.<name>`.
#[must_use]
pub fn failure(kind: FailureKind, name: &str) -> Failure {
    Failure::coded(kind, &key(&format!("errors.{name}")))
}

fn unknown(name: &str) -> Failure {
    let name: String = name.chars().take(64).collect();
    Failure::coded(FailureKind::NotFound, codes::UNKNOWN_CONTRIBUTION).with_param("name", name)
}

/// `args` as a JSON object (`{}` for an empty text).
///
/// # Errors
///
/// No JSON object.
pub fn parse(args: &str) -> Result<Value, Failure> {
    if args.trim().is_empty() {
        return Ok(json!({}));
    }
    match serde_json::from_str::<Value>(args) {
        Ok(value) if value.is_object() => Ok(value),
        _ => Err(invalid("arguments")),
    }
}

/// A post for the status bar: the plugin's text `notice.<name>` with `params`, at `level`
/// (`info`, `success`).
#[must_use]
pub fn notice(name: &str, level: &str, params: &Value) -> Value {
    json!({
        "target": "status",
        "message": { "key": key(&format!("notice.{name}")), "level": level, "params": params },
    })
}

/// The pipette ends in every view; the panel and the status bar say so.
#[must_use]
pub fn stopped() -> Value {
    json!({ "$post": [
        { "target": "all", "message": { "type": "stop" } },
        { "target": "panel", "message": { "type": "stopped" } },
        notice("stopped", "info", &json!({})),
    ] })
}

/// `pick`: the page script of every Chromium view shows the pipette, with the settings it draws
/// by - a Firefox or WebKit view has no window image to read (RB-080-77); `active` names the
/// active view, the only one that shows the loupe before the pointer comes - one read at its
/// centre, never one in every view (RB-080-56). The panel shows the pipette as on and the status
/// bar says how it ends, however it was started (RB-080-64). `{"stop": true}` ends it instead
/// ([`stopped`]).
#[must_use]
pub fn pick(host: &mut dyn PluginHost, args: &Value) -> Value {
    if args["stop"].as_bool() == Some(true) {
        return stopped();
    }
    let config = Settings::read(host).to_json();
    let active = host.active_view().ok().flatten();
    let message = json!({ "type": "pick", "config": config, "active": active });
    let mut posts: Vec<Value> = match host.views() {
        Ok(views) => views
            .iter()
            .filter(|view| targets::supported(view))
            .map(|view| json!({ "target": view.id, "message": message }))
            .collect(),
        // The grid unknown: every view's page script, as before.
        Err(_) => vec![json!({ "target": "all", "message": message })],
    };
    posts.push(json!({ "target": "panel", "message": { "type": "picking" } }));
    posts.push(notice("picking", "info", &json!({})));
    json!({ "$post": posts })
}

/// `panel`: the panel opens.
#[must_use]
pub fn panel() -> Value {
    json!({ "$open": { "panel": PANEL_ID } })
}

/// The feature plugin.
#[derive(Clone, Copy, Debug, Default)]
pub struct ColorPicker;

impl Feature for ColorPicker {
    fn run_command(
        &mut self,
        host: &mut dyn PluginHost,
        command: &str,
        args: &str,
    ) -> Result<String, Failure> {
        let args = parse(args)?;
        let answer = match command {
            "pick" => pick(host, &args),
            "measure_text" => measure::command(host, &args)?,
            "panel" => panel(),
            other => return Err(unknown(other)),
        };
        Ok(answer.to_string())
    }

    fn call_tool(
        &mut self,
        host: &mut dyn PluginHost,
        tool: &str,
        args: &str,
    ) -> Result<String, Failure> {
        let args = parse(args)?;
        let answer = match tool {
            "sample" => sample::tool(host, &args)?,
            "contrast" => contrast::tool(&args)?,
            "palette" => palette::tool(host, &args)?,
            "measure_text" => measure::tool(host, &args)?,
            PAGE_TOOL => page::handle(host, &args)?,
            other => return Err(unknown(other)),
        };
        Ok(answer.to_string())
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
