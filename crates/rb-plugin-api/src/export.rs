//! The native side of the `export-plugin` world: `interface exporter` (`export` is a WIT keyword).
//!
//! An export plugin turns a recording (the recorder's JSON, RB-050-02) into code - Playwright
//! first. It answers text; the host writes it where the person chooses.

use crate::{ExportFormat, ExportOutput, Failure, PluginHost};

/// The most bytes one rendered file may have.
pub const MAX_EXPORT_BYTES: usize = 8 * 1024 * 1024;

/// The logic of an export plugin. A store lives for one call.
pub trait Exporter {
    /// The formats it writes.
    fn formats(&self) -> Vec<ExportFormat>;

    /// Renders `recording` (JSON) in `format`.
    ///
    /// # Errors
    ///
    /// The format is unknown or the recording does not fit it.
    fn render(
        &mut self,
        host: &mut dyn PluginHost,
        format: &str,
        recording: &str,
    ) -> Result<ExportOutput, Failure>;
}

/// Whether `name` is a file name an export may answer: no folder, no `..`, nothing Windows would
/// change - the path rule of [`crate::paths`] with exactly one segment.
#[must_use]
pub fn valid_file_name(name: &str) -> bool {
    crate::paths::relative_segments(name).is_ok_and(|segments| segments.len() == 1)
}

#[cfg(test)]
#[path = "export_tests.rs"]
mod tests;
