//! What the fake's resources read (RB-095-01): a config of its own, the last check run, the last
//! screenshots and their PNGs.

use serde_json::Value;

use crate::backend::ToolFailure;

/// The fake's reads for `rbrowser://config`, `.../findings/latest` and `.../screenshots/latest`.
#[derive(Clone, Debug, Default)]
pub struct FakeLatest {
    /// What `config()` answers; the fake's small config when `None`.
    pub config: Option<Value>,
    /// What `latest_findings()` answers.
    pub findings: Option<Value>,
    /// What `latest_screenshots()` answers.
    pub screenshots: Option<Value>,
    /// The PNG of each file of `screenshots`, in its order.
    pub pngs: Vec<Vec<u8>>,
}

impl FakeLatest {
    /// The PNG `index` as the app hands it out: refused over `max_bytes`, unknown past the end.
    pub(crate) fn png(&self, index: usize, max_bytes: usize) -> Result<Vec<u8>, ToolFailure> {
        let png = self.pngs.get(index).ok_or_else(|| {
            ToolFailure::new("screenshots_none", format!("no screenshot {index}"))
        })?;
        if png.len() > max_bytes {
            return Err(
                ToolFailure::new("answer_too_large", "the image is too large")
                    .with_setting("mcp.maxAnswerBytes"),
            );
        }
        Ok(png.clone())
    }
}
