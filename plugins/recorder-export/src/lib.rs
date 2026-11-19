//! rBrowser's first-party recorder export (RB-050-02): a recording of RB-050-01 as an rBrowser
//! flow and as a Playwright test with one project per breakpoint.
//!
//! The plugin is an `export-plugin` (`docs/plugins.md`) and needs no capability: the host reads
//! the recording, wraps it with the options and the breakpoints' emulation from the project
//! config ([`model::ExportRequest`]) and writes the answers to the exports folder. One call
//! answers one file, so the Playwright export is two calls - [`SPEC`] and [`PLAYWRIGHT_CONFIG`].
//!
//! | Format | File | Module |
//! | --- | --- | --- |
//! | [`FLOW`] | `<name>.ts` | [`flow`] |
//! | [`SPEC`] | `<name>.spec.ts` | [`spec`] |
//! | [`PLAYWRIGHT_CONFIG`] | `playwright.config.ts` | [`spec`] |
//! | [`PAUSES`] (the host's, not offered as an export) | `<name>.pauses.json` | [`pace`] |
//!
//! [`PAUSES`] answers the pause before each written step at the `recorded` pace
//! (`{"pauses":[{"step":1,"ms":0},...]}`): the host adds them to a played flow's timeout and the
//! Recorder panel shows them, so the rule lives here only (RB-090-84).
//!
//! A secret stays a secret: the recording holds only `{{secret:<name>}}` placeholders, and the
//! code reads `process.env.RB_SECRET_<NAME>`. The logic lives here and is tested natively
//! (`cargo test`); `src/guest.rs` only translates it into the WIT vocabulary on `wasm32`.

#[cfg(target_arch = "wasm32")]
mod guest;

pub mod flow;
pub mod js;
pub mod locator;
pub mod model;
pub mod pace;
pub mod spec;
pub mod steps;

use rb_plugin_api::export::{Exporter, MAX_EXPORT_BYTES, valid_file_name};
use rb_plugin_api::{ExportFormat, ExportOutput, Failure, FailureKind, PluginHost};

use model::{ParseError, parse_request};

/// The plugin's id: the prefix of its keys.
pub const PLUGIN_ID: &str = "recorder-export";
/// The rBrowser flow.
pub const FLOW: &str = "flow";
/// The Playwright test.
pub const SPEC: &str = "spec";
/// The Playwright config with one project per breakpoint.
pub const PLAYWRIGHT_CONFIG: &str = "playwright-config";
/// The pause before each written step at the `recorded` pace, as JSON - the host's.
pub const PAUSES: &str = "pauses";
/// The file name of [`PLAYWRIGHT_CONFIG`].
pub const CONFIG_FILE: &str = "playwright.config.ts";
/// The file stem when the recording's name is none a file may have.
const FALLBACK_STEM: &str = "recording";
/// The longest reason a failure carries.
const MAX_REASON: usize = 200;

/// The exporter.
#[derive(Clone, Copy, Debug, Default)]
pub struct RecorderExport;

impl Exporter for RecorderExport {
    fn formats(&self) -> Vec<ExportFormat> {
        [(FLOW, "ts"), (SPEC, "spec.ts"), (PLAYWRIGHT_CONFIG, "ts")]
            .into_iter()
            .map(|(id, extension)| ExportFormat {
                id: id.to_owned(),
                title: format!("plugin.{PLUGIN_ID}.formats.{id}"),
                extension: extension.to_owned(),
            })
            .collect()
    }

    fn render(
        &mut self,
        _host: &mut dyn PluginHost,
        format: &str,
        recording: &str,
    ) -> Result<ExportOutput, Failure> {
        render(format, recording)
    }
}

/// `input` - the host's request or a bare recording - in `format`.
///
/// # Errors
///
/// The format is unknown, the input is no recording of format version 1, a step cannot be
/// written, or the file would be larger than the host takes.
pub fn render(format: &str, input: &str) -> Result<ExportOutput, Failure> {
    if ![FLOW, SPEC, PLAYWRIGHT_CONFIG, PAUSES].contains(&format) {
        return Err(Failure::coded(
            FailureKind::NotSupported,
            "plugins.error.unknown_contribution",
        )
        .with_param("name", format));
    }
    let request = parse_request(input).map_err(|error| match error {
        ParseError::Invalid(reason) => {
            Failure::coded(FailureKind::BadData, &error_key("bad_recording"))
                .with_param("reason", cut(&reason))
        }
        ParseError::Version(version) => Failure::coded(FailureKind::BadData, &error_key("version"))
            .with_param("version", version),
    })?;
    // The files carry the export's name, the text the recording's (RB-090-69).
    let stem = request
        .options
        .name
        .as_deref()
        .filter(|name| fits_file(name))
        .unwrap_or_else(|| file_stem(&request.recording.name));
    let (file_name, text) = match format {
        FLOW => (format!("{stem}.ts"), flow::flow(&request)),
        SPEC => (format!("{stem}.spec.ts"), spec::spec(&request)),
        PAUSES => (format!("{stem}.pauses.json"), Ok(pauses_json(&request))),
        _ => (CONFIG_FILE.to_owned(), Ok(spec::config(&request))),
    };
    let text = text.map_err(|error| {
        Failure::coded(FailureKind::BadData, &error_key("step"))
            .with_param("step", error.step)
            .with_param("reason", cut(&error.reason))
    })?;
    if text.len() > MAX_EXPORT_BYTES {
        return Err(Failure::coded(FailureKind::Limit, &error_key("too_large")));
    }
    Ok(ExportOutput { file_name, text })
}

/// [`PAUSES`]: `{"pauses":[{"step":<id>,"ms":<pause>},...]}`, the written steps in order, at the
/// `recorded` pace whatever `options.pace` says, with `options.minPauseMs`.
fn pauses_json(request: &model::ExportRequest) -> String {
    let pauses: Vec<serde_json::Value> =
        steps::step_pauses(&request.recording.steps, request.options.min_pause_ms)
            .into_iter()
            .map(|(step, ms)| serde_json::json!({ "step": step, "ms": ms }))
            .collect();
    serde_json::json!({ "pauses": pauses }).to_string()
}

/// The recording's name when it [fits a file](fits_file), else `recording`.
fn file_stem(name: &str) -> &str {
    if fits_file(name) { name } else { FALLBACK_STEM }
}

/// Whether `name` is `[A-Za-z0-9_-]`, 1 to 64 characters (the recorder's rule), and a file stem.
fn fits_file(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        && valid_file_name(&format!("{name}.spec.ts"))
}

fn error_key(name: &str) -> String {
    format!("plugin.{PLUGIN_ID}.errors.{name}")
}

/// `reason` cut to [`MAX_REASON`] characters.
fn cut(reason: &str) -> String {
    reason.chars().take(MAX_REASON).collect()
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "autofill_tests.rs"]
mod autofill_tests;
