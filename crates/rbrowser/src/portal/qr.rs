//! The pairing QR code (RB-080-05): the module matrix of the pairing URL, which the portal
//! panel draws as squares - no image, no markup from the core. The matrix encodes the token, so
//! it reaches the panel only, never a log or MCP; `Debug` shows its size alone.

use std::fmt;

use qrcode::{Color, EcLevel, QrCode};
use serde::Serialize;

/// A QR code's modules, row by row: `1` dark, `0` light.
#[derive(Clone, PartialEq, Eq, Serialize)]
pub struct QrMatrix {
    /// Modules per side.
    pub size: usize,
    /// `size` rows of `size` characters.
    pub rows: Vec<String>,
}

impl QrMatrix {
    /// The QR code of `text` at error correction level M.
    ///
    /// # Errors
    ///
    /// `text` does not fit a QR code.
    pub fn encode(text: &str) -> Result<Self, String> {
        let code = QrCode::with_error_correction_level(text.as_bytes(), EcLevel::M)
            .map_err(|error| error.to_string())?;
        let size = code.width();
        let colors = code.to_colors();
        let rows = colors
            .chunks(size.max(1))
            .map(|row| {
                row.iter()
                    .map(|color| if *color == Color::Dark { '1' } else { '0' })
                    .collect()
            })
            .collect();
        Ok(Self { size, rows })
    }
}

impl fmt::Debug for QrMatrix {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "QrMatrix({0}x{0})", self.size)
    }
}

#[cfg(test)]
#[path = "qr_tests.rs"]
mod tests;
