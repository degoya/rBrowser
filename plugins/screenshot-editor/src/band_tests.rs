//! Band rendering against the whole image (RB-099-24): every tool, redactions across band edges
//! and stacked on each other, the crop - small images in bands of a few rows.

use serde_json::json;
use tiny_skia::{IntSize, Pixmap};

use super::*;
use crate::model::validate;
use crate::rows::MemoryRows;

/// An opaque image with fine detail, so a mosaic and a blur change it everywhere.
pub fn pattern(width: u32, height: u32) -> Pixmap {
    let mut data = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for x in 0..width {
            let dark = (x + 2 * y) % 3 == 0 || (x * y) % 7 == 1;
            let shade = if dark { 20 } else { 230 };
            data.extend_from_slice(&[shade, ((x * 5) % 256) as u8, ((y * 3) % 256) as u8, 255]);
        }
    }
    Pixmap::from_vec(data, IntSize::from_wh(width, height).expect("size")).expect("pixmap")
}

pub fn annotations(value: serde_json::Value) -> Vec<Annotation> {
    let mut list: Vec<Annotation> = serde_json::from_value(value).expect("annotations");
    validate(&mut list).expect("valid");
    list
}

/// `image` with `list` rendered in bands of `band_rows`: the output's size and pixels.
pub fn banded(image: &Pixmap, list: &[Annotation], band_rows: u32) -> (u32, u32, Vec<u8>) {
    let plan = plan(list, image.width(), image.height(), band_rows).expect("plan");
    let mut source = MemoryRows {
        pixmap: image.clone(),
        next: 0,
    };
    let mut bands = Bands::new(&mut source, list, &plan);
    let (width, height) = bands.size();
    let mut out = Vec::new();
    let mut rows = 0;
    while let Some((data, count)) = bands.next_band().expect("band") {
        out.extend_from_slice(data);
        rows += count;
    }
    assert_eq!(rows, height);
    (width, height, out)
}

/// The largest difference of one channel between `a` and `b`.
fn largest_difference(a: &[u8], b: &[u8]) -> u8 {
    assert_eq!(a.len(), b.len());
    a.iter()
        .zip(b)
        .map(|(x, y)| x.abs_diff(*y))
        .max()
        .unwrap_or(0)
}

/// Band and whole image alike up to anti-aliasing: tiny-skia covers a stroke's or a fill's
/// edge pixels a little differently when the path is clipped at a band's edge, so a handful of
/// edge pixels (at most one in 200) may differ by up to 32 levels; every other channel by 3.
pub(crate) fn assert_close(a: &[u8], b: &[u8], what: &str) {
    assert_eq!(a.len(), b.len(), "{what}");
    let worst = largest_difference(a, b);
    assert!(worst <= 32, "{what}: a channel {worst} off");
    let off = a
        .chunks(4)
        .zip(b.chunks(4))
        .filter(|(x, y)| x.iter().zip(y.iter()).any(|(p, q)| p.abs_diff(*q) > 3))
        .count();
    assert!(off * 200 <= a.len() / 4, "{what}: {off} pixels off");
}

/// Redactions only, across band edges and on top of each other: integer arithmetic, so a band
/// computes exactly the whole image's pixels.
fn redactions() -> Vec<Annotation> {
    annotations(json!([
        { "type": "redact", "x": 3, "y": 5, "w": 30, "h": 41, "mode": "blur", "block": 8 },
        { "type": "redact", "x": 10, "y": 30, "w": 30, "h": 30, "mode": "pixelate", "block": 13 },
        { "type": "redact", "x": 0, "y": 20, "w": 48, "h": 9, "mode": "blur", "block": 12 },
        { "type": "redact", "x": 20, "y": 50, "w": 9, "h": 9, "mode": "black" },
        { "type": "redact", "x": 0, "y": 10, "w": 20, "h": 14, "mode": "pixelate", "block": 8 }
    ]))
}

#[test]
fn redactions_in_bands_are_the_whole_image_exactly() {
    let image = pattern(48, 64);
    let list = redactions();
    let whole = render::render(image.clone(), &list).expect("whole");
    for band_rows in [1, 5, 8, 13, 64] {
        let (width, height, data) = banded(&image, &list, band_rows);
        assert_eq!((width, height), (48, 64));
        assert_eq!(data, whole.data(), "bands of {band_rows}");
    }
}

#[test]
fn every_tool_in_bands_is_the_whole_image() {
    // Shapes and text crossing band edges: a shape clipped at a band's edge, and a glyph placed
    // nearer zero, may round their anti-aliasing a step otherwise.
    let image = pattern(48, 64);
    let list = annotations(json!([
        { "type": "rect", "x": 2.5, "y": 6.5, "w": 30, "h": 20, "color": "#ff0000", "width": 3 },
        { "type": "rect", "x": 30, "y": 3, "w": 10, "h": 50, "color": "#000000", "width": 1, "fill": true },
        { "type": "arrow", "x1": 1, "y1": 1, "x2": 40.3, "y2": 60.7, "color": "#00ff00", "width": 3 },
        { "type": "line", "x1": 0, "y1": 31.5, "x2": 48, "y2": 33, "color": "#0000ff", "width": 2 },
        { "type": "freehand", "points": [[1, 1], [5, 16], [9, 4], [20, 40]], "color": "#123456", "width": 4 },
        { "type": "text", "x": 4, "y": 12, "text": "Bug here", "color": "#000000", "size": 14 },
        { "type": "marker", "x": 24, "y": 24.5, "number": 7, "color": "#e53935", "size": 16 },
        { "type": "highlight", "x": 0, "y": 6.3, "w": 40, "h": 13.4, "color": "#ffeb3b" },
        { "type": "redact", "x": 10, "y": 14, "w": 16, "h": 20, "mode": "blur", "block": 8 },
        { "type": "redact", "x": 4, "y": 40, "w": 30, "h": 17, "mode": "pixelate", "block": 8 },
        { "type": "crop", "x": 2, "y": 3, "w": 41, "h": 57 }
    ]));
    let whole = render::render(image.clone(), &list).expect("whole");
    for band_rows in [3, 8, 16] {
        let (width, height, data) = banded(&image, &list, band_rows);
        assert_eq!((width, height), (whole.width(), whole.height()));
        assert_close(&data, whole.data(), &format!("bands of {band_rows}"));
    }
}

#[test]
fn the_crop_chooses_the_rows_and_columns() {
    let image = pattern(40, 40);
    let list = annotations(json!([
        { "type": "redact", "x": 0, "y": 0, "w": 40, "h": 40, "mode": "pixelate", "block": 8 },
        { "type": "crop", "x": 5, "y": 9, "w": 20, "h": 13 }
    ]));
    let whole = render::render(image.clone(), &list).expect("whole");
    let (width, height, data) = banded(&image, &list, 8);
    assert_eq!((width, height), (20, 13));
    assert_eq!(data, whole.data());
    let plan = plan(&list, 40, 40, 8).expect("plan");
    assert_eq!(
        plan.bands.iter().map(|band| band.rows).collect::<Vec<_>>(),
        [(9, 17), (17, 22)]
    );
}

#[test]
fn a_band_needs_the_rows_its_redactions_and_the_earlier_ones_read() {
    // A blur of block 8 reads 7 + 4 rows around, inside its region; the pixelation before it 12
    // more around those.
    let list = annotations(json!([
        { "type": "redact", "x": 0, "y": 0, "w": 10, "h": 100, "mode": "pixelate", "block": 13 },
        { "type": "redact", "x": 0, "y": 40, "w": 10, "h": 20, "mode": "blur", "block": 8 },
        { "type": "redact", "x": 0, "y": 90, "w": 10, "h": 5, "mode": "black" }
    ]));
    assert_eq!(needed(&list, 10, 100, 44, 48), (28, 71));
    // Outside every region but the first: its own block rows.
    assert_eq!(needed(&list, 10, 100, 70, 72), (58, 84));
    // A black bar reads nothing around it.
    let black = annotations(json!([
        { "type": "redact", "x": 0, "y": 0, "w": 10, "h": 100, "mode": "black" }
    ]));
    assert_eq!(needed(&black, 10, 100, 44, 48), (44, 48));
}

#[test]
fn a_band_over_the_window_limit_names_it() {
    // Blurs of the largest block stacked over a very wide image.
    let blur = json!({ "type": "redact", "x": 0, "y": 0, "w": 65535, "h": 4096, "mode": "blur", "block": 128 });
    let list = annotations(json!([blur, blur, blur, blur]));
    let refused = plan(&list, 65_535, 4096, 8).expect_err("too many rows at once");
    assert_eq!(
        refused.code(),
        Some("plugin.screenshot-editor.errors.redactions_too_large")
    );
    // One of them fits.
    let one = annotations(json!([blur]));
    assert!(plan(&one, 65_535, 4096, rows_per_band(65_535)).is_ok());
}

#[test]
fn a_band_holds_about_eight_mebibytes_in_whole_block_rows() {
    assert_eq!(rows_per_band(2880), 728);
    assert_eq!(rows_per_band(65_535), 32);
    assert_eq!(rows_per_band(1_000_000), 8);
    for width in [1, 100, 2880, 65_535] {
        assert_eq!(rows_per_band(width) % 8, 0);
    }
}
