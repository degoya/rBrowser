//! One view against its baseline: the capture decoded, compared with the stored PNG
//! ([`crate::diff`]) and the metadata ([`crate::meta`]), the outcome as a [`Comparison`] - the
//! record the app answers over MCP and hands the `visual-regression` plugin as its arguments,
//! which turns it into findings ("view X differs by n %").

use serde::{Deserialize, Serialize};

use crate::baseline::Baseline;
use crate::diff::{DiffOptions, DiffOutcome, Size, diff};
use crate::meta::{BaselineMeta, MetaMismatch, compare_meta};
use crate::regions::Region;
use crate::{VisualError, decode_png, encode_png, png_size};

/// What a view's comparison found.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Status {
    /// No pixel differs beyond the threshold.
    #[default]
    Ok,
    /// Pixels differ: a regression.
    Differs,
    /// There is no baseline yet.
    New,
    /// The capture and the baseline differ in size.
    SizeMismatch,
    /// The view could not be captured or compared.
    Failed,
}

impl Status {
    /// Whether this counts as a regression (a failing compare).
    pub fn is_regression(self) -> bool {
        matches!(self, Self::Differs | Self::SizeMismatch)
    }
}

/// The view a comparison is of.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Subject {
    /// The view's id.
    pub view: u32,
    /// Its label.
    pub label: String,
    /// The baseline's name: the breakpoint id, or `<w>x<h>` for a free size.
    pub breakpoint: String,
    /// The page's baseline folder.
    pub slug: String,
    /// The page, without query, fragment and credentials.
    pub url: String,
}

/// One view against its baseline.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Comparison {
    /// The view.
    #[serde(flatten)]
    pub subject: Subject,
    /// The outcome.
    pub status: Status,
    /// The pixels that differ.
    pub different_pixels: u64,
    /// Every pixel compared.
    pub total_pixels: u64,
    /// The share of differing pixels, 0 to 1.
    pub ratio: f64,
    /// The differing areas, largest first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub regions: Vec<Region>,
    /// The baseline's size.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baseline_size: Option<Size>,
    /// The capture's size.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actual_size: Option<Size>,
    /// Where the setup differs from the baseline's: the comparison is unreliable.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub meta_mismatches: Vec<MetaMismatch>,
    /// The baseline has no metadata: the comparison is unreliable.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub meta_missing: bool,
    /// The baseline PNG.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baseline_path: Option<String>,
    /// The capture, saved beside the diff.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actual_path: Option<String>,
    /// The diff image.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diff_path: Option<String>,
    /// Why the view failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl Comparison {
    /// Whether the setup differs from the baseline's, so the result may be wrong.
    pub fn unreliable(&self) -> bool {
        self.meta_missing || !self.meta_mismatches.is_empty()
    }

    /// The share of differing pixels in percent, two decimals.
    pub fn percent(&self) -> f64 {
        (self.ratio * 10_000.0).round() / 100.0
    }

    /// A view that could not be compared.
    pub fn failed(subject: Subject, error: String) -> Self {
        Self {
            subject,
            status: Status::Failed,
            error: Some(error),
            ..Self::default()
        }
    }
}

/// A comparison and the diff image to save with it (none when nothing was compared).
#[derive(Clone, Debug, PartialEq)]
pub struct Compared {
    /// The outcome.
    pub comparison: Comparison,
    /// The diff image as PNG.
    pub diff_png: Option<Vec<u8>>,
}

/// Compares the capture `actual` (PNG) of `subject` with `baseline` (`None`: there is none
/// yet), taken now with `current`. The sizes come from the PNG headers; the images are decoded
/// only when they match, so a new page or a size mismatch never allocates their pixels.
pub fn compare(
    subject: Subject,
    baseline: Option<&Baseline>,
    actual: &[u8],
    current: &BaselineMeta,
    options: &DiffOptions,
) -> Result<Compared, VisualError> {
    let actual_size = header_size(actual)?;
    let mut comparison = Comparison {
        subject,
        actual_size: Some(actual_size),
        ..Comparison::default()
    };
    let Some(baseline) = baseline else {
        comparison.status = Status::New;
        return Ok(Compared {
            comparison,
            diff_png: None,
        });
    };
    comparison.baseline_path = Some(baseline.path.display().to_string());
    match &baseline.meta {
        Some(meta) => comparison.meta_mismatches = compare_meta(meta, current),
        None => comparison.meta_missing = true,
    }
    let baseline_size = header_size(&baseline.png)?;
    comparison.baseline_size = Some(baseline_size);
    if baseline_size != actual_size {
        // Nothing to compare: neither image is decoded.
        comparison.status = Status::SizeMismatch;
        return Ok(Compared {
            comparison,
            diff_png: None,
        });
    }
    let expected = decode_png(&baseline.png)?;
    let actual = decode_png(actual)?;
    let diff_png = match diff(&expected, &actual, options)? {
        DiffOutcome::SizeMismatch { .. } => {
            comparison.status = Status::SizeMismatch;
            None
        }
        DiffOutcome::Compared(found) => {
            comparison.status = if found.different == 0 {
                Status::Ok
            } else {
                Status::Differs
            };
            comparison.different_pixels = found.different;
            comparison.total_pixels = found.total;
            comparison.ratio = found.ratio();
            comparison.regions = found.regions.clone();
            (found.different > 0)
                .then(|| encode_png(&found.image))
                .transpose()?
        }
    };
    Ok(Compared {
        comparison,
        diff_png,
    })
}

/// The size in a PNG's header, without decoding it.
fn header_size(png: &[u8]) -> Result<Size, VisualError> {
    png_size(png)
        .map(|(width, height)| Size { width, height })
        .ok_or_else(|| VisualError::Png("not a PNG".to_owned()))
}

#[cfg(test)]
#[path = "compare_tests.rs"]
mod tests;
