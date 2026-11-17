//! What a source of stylesheets hands the plugin: each sheet's URL, text and media list, within
//! limits that keep one call of the plugin bounded.

use serde::Serialize;

/// The most stylesheets read for one analysis.
pub const MAX_SHEETS: usize = 512;
/// The largest stylesheet read.
pub const MAX_SHEET_BYTES: usize = 8 * 1024 * 1024;
/// The most CSS handed to the plugin in one call (its memory limit is 256 MiB).
pub const MAX_TOTAL_BYTES: usize = 48 * 1024 * 1024;

/// One stylesheet, as the plugin's `detect` takes it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Sheet {
    /// Its URL; inline sheets the page URL with `#inline-<n>`.
    pub url: String,
    /// Its text.
    pub text: String,
    /// The media list of its `<link>` or `<style>`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media: Option<String>,
}

/// The stylesheets of the analysed pages.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Collected {
    pub sheets: Vec<Sheet>,
    /// The root font size the page reported (`em`/`rem` in its queries); 16 when unknown.
    pub root_font_size_px: Option<f64>,
    total_bytes: usize,
}

impl Collected {
    /// Adds `sheet` unless a limit is reached (it is dropped with a warning then).
    pub fn push(&mut self, sheet: Sheet) {
        if self.sheets.len() >= MAX_SHEETS
            || sheet.text.len() > MAX_SHEET_BYTES
            || self.total_bytes + sheet.text.len() > MAX_TOTAL_BYTES
        {
            tracing::warn!(
                bytes = sheet.text.len(),
                "a stylesheet over the analyser's limits was skipped"
            );
            return;
        }
        self.total_bytes += sheet.text.len();
        self.sheets.push(sheet);
    }

    /// Adds every sheet of `other`; its root font size counts when this has none.
    pub fn extend(&mut self, other: Collected) {
        self.root_font_size_px = self.root_font_size_px.or(other.root_font_size_px);
        for sheet in other.sheets {
            if !self
                .sheets
                .iter()
                .any(|known| known.url == sheet.url && known.text == sheet.text)
            {
                self.push(sheet);
            }
        }
    }
}
