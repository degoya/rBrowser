//! The edited image in bands of rows (RB-099-24), so an image of the capture's size - up to 256
//! megapixels, 1 GiB as RGBA - renders in a few MiB: rows are decoded as a band needs them,
//! drawn on ([`render::draw_all`]), cropped, handed to the encoder and dropped.
//!
//! Every shape changes only the pixels it covers, but a redaction reads around them: a mosaic
//! block's rows, the blur's radius more ([`redact::margin`]). So each band is drawn on the rows
//! it shows and, where a redaction reaches into it, the rows that redaction reads - and where an
//! earlier redaction reaches into those, its rows too, from the last annotation back to the
//! first ([`needed`]). A band then computes its own rows as the whole image does. The crop
//! chooses the rows and columns the bands show.

use rb_plugin_api::{Failure, FailureKind};
use tiny_skia::{IntSize, Pixmap};

use crate::PLUGIN_ID;
use crate::model::Annotation;
use crate::redact::{self, Region};
use crate::render;
use crate::rows::RowSource;

/// The bytes of output rows one band holds, about.
pub const BAND_BYTES: usize = 8 * 1024 * 1024;
/// The most bytes of decoded rows held at once, a band's margins included (and its copy as
/// many again): within the manifest's memory with the codecs' buffers.
pub const MAX_WINDOW_BYTES: usize = 128 * 1024 * 1024;

/// One band: the image rows it shows (`rows`, inside the crop) and the rows it draws on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Band {
    pub rows: (u32, u32),
    pub need: (u32, u32),
}

/// The bands of one render, top to bottom.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    pub width: u32,
    pub height: u32,
    /// What the output keeps: the crop, else the whole image.
    pub crop: Region,
    pub bands: Vec<Band>,
    /// For each band, the first row it or a later band needs: rows above it are dropped.
    keep: Vec<u32>,
}

/// The rows of one band for an image `width` wide: about [`BAND_BYTES`], a multiple of 8 (a
/// JPEG block row never spans two bands).
#[must_use]
pub fn rows_per_band(width: u32) -> u32 {
    let per = BAND_BYTES / (width.max(1) as usize * 4);
    (u32::try_from(per).unwrap_or(u32::MAX) / 8 * 8).max(8)
}

/// `plugin.screenshot-editor.errors.redactions_too_large`: the rows a band needs exceed
/// [`MAX_WINDOW_BYTES`] (many large blurs on top of each other in a very wide image).
fn window_too_large(rows: u32, width: u32) -> Failure {
    Failure::coded(
        FailureKind::Limit,
        &format!("plugin.{PLUGIN_ID}.errors.redactions_too_large"),
    )
    .with_param("rows", rows)
    .with_param("width", width)
}

/// The rows `top..bottom` need drawn to show as the whole image would, `annotations` applied
/// in order on a `width` x `height` image.
#[must_use]
pub fn needed(
    annotations: &[Annotation],
    width: u32,
    height: u32,
    top: u32,
    bottom: u32,
) -> (u32, u32) {
    let (mut top, mut bottom) = (top, bottom);
    for annotation in annotations.iter().rev() {
        let Annotation::Redact {
            x,
            y,
            w,
            h,
            mode,
            block,
        } = annotation
        else {
            continue;
        };
        let Some(region) = Region::clip(*x, *y, *w, *h, width, height) else {
            continue;
        };
        if region.top < bottom && top < region.bottom {
            let margin = redact::margin(*mode, *block);
            top = top.min(region.top.max(top.saturating_sub(margin)));
            bottom = bottom.max(region.bottom.min(bottom.saturating_add(margin)));
        }
    }
    (top, bottom)
}

/// The rows drawn beyond a band on each side so a clipped shape anti-aliases as on the whole
/// image.
pub const EDGE_ROWS: u32 = 8;

/// The bands of `annotations` on a `width` x `height` image, `band_rows` rows each.
pub fn plan(
    annotations: &[Annotation],
    width: u32,
    height: u32,
    band_rows: u32,
) -> Result<Plan, Failure> {
    let crop = render::crop_of(annotations, width, height)?.map_or(
        Region {
            left: 0,
            top: 0,
            right: width,
            bottom: height,
        },
        |(_, region)| region,
    );
    let band_rows = band_rows.max(1);
    let mut bands = Vec::new();
    let mut top = crop.top;
    while top < crop.bottom {
        let bottom = top.saturating_add(band_rows).min(crop.bottom);
        let need = needed(annotations, width, height, top, bottom);
        // Shapes are drawn on EDGE_ROWS more rows on each side than a band keeps: tiny-skia
        // anti-aliases a path clipped at the pixmap's edge differently from the whole image.
        let need = (
            need.0.min(top.saturating_sub(EDGE_ROWS)),
            need.1.max(bottom.saturating_add(EDGE_ROWS).min(height)),
        );
        bands.push(Band {
            rows: (top, bottom),
            need,
        });
        top = bottom;
    }
    let mut keep = vec![0; bands.len()];
    let mut lowest = height;
    for (index, band) in bands.iter().enumerate().rev() {
        lowest = lowest.min(band.need.0);
        keep[index] = lowest;
    }
    let stride = width as usize * 4;
    for (band, first) in bands.iter().zip(&keep) {
        let rows = band.need.1 - first;
        if rows as usize * stride > MAX_WINDOW_BYTES {
            return Err(window_too_large(rows, width));
        }
    }
    Ok(Plan {
        width,
        height,
        crop,
        bands,
        keep,
    })
}

/// The bands of a [`Plan`] rendered one after the other from a [`RowSource`].
pub struct Bands<'p, 'r> {
    source: &'r mut dyn RowSource,
    annotations: &'p [Annotation],
    plan: &'p Plan,
    next: usize,
    /// Decoded rows from `window_top` up to `read`.
    window: Vec<u8>,
    window_top: u32,
    read: u32,
    row: Vec<u8>,
    out: Vec<u8>,
}

impl<'p, 'r> Bands<'p, 'r> {
    #[must_use]
    pub fn new(
        source: &'r mut dyn RowSource,
        annotations: &'p [Annotation],
        plan: &'p Plan,
    ) -> Self {
        let stride = plan.width as usize * 4;
        Self {
            source,
            annotations,
            plan,
            next: 0,
            window: Vec::new(),
            window_top: 0,
            read: 0,
            row: vec![0; stride],
            out: Vec::new(),
        }
    }

    /// The output's width and height: the crop's.
    #[must_use]
    pub fn size(&self) -> (u32, u32) {
        let crop = self.plan.crop;
        (crop.right - crop.left, crop.bottom - crop.top)
    }

    /// Whether every output pixel is opaque: drawing never makes a pixel less opaque, so an
    /// opaque source gives an opaque result.
    #[must_use]
    pub fn opaque(&self) -> bool {
        self.source.opaque()
    }

    /// The next band's rows, premultiplied RGBA, the crop's columns, with how many there are;
    /// `None` after the last.
    pub fn next_band(&mut self) -> Result<Option<(&[u8], u32)>, Failure> {
        let Some(band) = self.plan.bands.get(self.next).copied() else {
            return Ok(None);
        };
        let keep = self.plan.keep[self.next];
        self.next += 1;
        let stride = self.plan.width as usize * 4;
        // Drop the rows no band from here on needs; skip unread rows above `keep`.
        if keep <= self.read {
            let dropped = keep.saturating_sub(self.window_top) as usize * stride;
            self.window.drain(..dropped.min(self.window.len()));
            self.window_top = self.window_top.max(keep);
        } else {
            self.window.clear();
            while self.read < keep {
                self.source.next_row(&mut self.row)?;
                self.read += 1;
            }
            self.window_top = keep;
        }
        while self.read < band.need.1 {
            self.source.next_row(&mut self.row)?;
            self.window.extend_from_slice(&self.row);
            self.read += 1;
        }
        let start = (band.need.0 - self.window_top) as usize * stride;
        let rows = band.need.1 - band.need.0;
        let data = self.window[start..start + rows as usize * stride].to_vec();
        let size = IntSize::from_wh(self.plan.width, rows).ok_or_else(crate::codec::not_png)?;
        let mut pixmap = Pixmap::from_vec(data, size).ok_or_else(crate::codec::not_png)?;
        render::draw_all(&mut pixmap, band.need.0, self.plan.height, self.annotations);
        let crop = self.plan.crop;
        let (left, right) = (crop.left as usize * 4, crop.right as usize * 4);
        self.out.clear();
        for y in band.rows.0..band.rows.1 {
            let at = (y - band.need.0) as usize * stride;
            self.out
                .extend_from_slice(&pixmap.data()[at + left..at + right]);
        }
        Ok(Some((&self.out, band.rows.1 - band.rows.0)))
    }
}

#[cfg(test)]
#[path = "band_tests.rs"]
pub(crate) mod tests;
