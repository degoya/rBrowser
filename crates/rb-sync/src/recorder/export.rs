//! The boundary to the exporters (RB-050-01 keeps it stable; RB-050-02 adds the Playwright
//! `.spec.ts` and the rBrowser flow `.ts`): an [`Exporter`] turns a [`Recording`] into the text
//! of one file. [`JsonExporter`] is the recording's own format, the one the store writes.

use super::model::{RECORDING_VERSION, Recording};

/// Why a recording could not be exported or read.
#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    /// The text is not a recording of this format.
    #[error("not a recording: {0}")]
    Invalid(String),
    /// A recording of another format version.
    #[error("recording format version {0} is not supported (expected {RECORDING_VERSION})")]
    Version(u32),
    /// The exporter cannot express a step.
    #[error("step {step} cannot be exported: {reason}")]
    Unsupported {
        /// The step's id.
        step: u32,
        /// Why.
        reason: String,
    },
}

/// Turns a recording into the text of one file.
pub trait Exporter: Send + Sync {
    /// A stable id (`json`, `playwright`, `flow`).
    fn id(&self) -> &str;
    /// The file name's ending without the dot (`json`, `spec.ts`).
    fn extension(&self) -> &str;
    /// The file's text. A secret stays a placeholder; an exporter never resolves one.
    fn export(&self, recording: &Recording) -> Result<String, ExportError>;
}

/// The recording format itself.
#[derive(Clone, Copy, Debug, Default)]
pub struct JsonExporter;

impl Exporter for JsonExporter {
    fn id(&self) -> &str {
        "json"
    }

    fn extension(&self) -> &str {
        "json"
    }

    fn export(&self, recording: &Recording) -> Result<String, ExportError> {
        to_json(recording)
    }
}

/// `recording` as pretty JSON with a final newline.
pub fn to_json(recording: &Recording) -> Result<String, ExportError> {
    let mut text = serde_json::to_string_pretty(recording)
        .map_err(|error| ExportError::Invalid(error.to_string()))?;
    text.push('\n');
    Ok(text)
}

/// The recording in `text`; refuses another format version.
pub fn from_json(text: &str) -> Result<Recording, ExportError> {
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|error| ExportError::Invalid(error.to_string()))?;
    let version = value
        .get("version")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| ExportError::Invalid("no `version`".to_owned()))?;
    if version != u64::from(RECORDING_VERSION) {
        return Err(ExportError::Version(
            u32::try_from(version).unwrap_or(u32::MAX),
        ));
    }
    serde_json::from_value(value).map_err(|error| ExportError::Invalid(error.to_string()))
}

#[cfg(test)]
#[path = "export_tests.rs"]
mod tests;
