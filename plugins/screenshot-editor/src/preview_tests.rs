use rb_plugin_api::Capability;
use rb_plugin_api::test_support::MemoryHost;
use tiny_skia::{IntSize, Pixmap};

use super::*;
use crate::rows::MemoryRows;
use crate::testing::png;

#[test]
fn an_image_within_a_canvas_is_shown_as_it_is_and_a_larger_one_scaled() {
    assert_eq!(scale(1920, 1080, 0), 1);
    assert_eq!(scale(16_384, 1_500, 0), 1);
    // The owner's long page: 2880 x 24576 (70.8 megapixels).
    assert_eq!(scale(2880, 24_576, 0), 2);
    // The capture's limits: 65535 rows, 256 megapixels.
    assert_eq!(scale(4096, 65_535, 0), 4);
    assert_eq!(scale(16_384, 16_384, 0), 4);
    for (width, height) in [(2880, 24_576), (4096, 65_535), (65_535, 4096)] {
        let factor = scale(width, height, 0);
        let (w, h) = (width.div_ceil(factor), height.div_ceil(factor));
        assert!(w <= PREVIEW_SIDE && h <= PREVIEW_SIDE, "{width} x {height}");
        assert!(
            u64::from(w) * u64::from(h) <= PREVIEW_PIXELS,
            "{width} x {height}"
        );
    }
    // A PNG over what the panel loads is scaled although its size would fit.
    assert_eq!(scale(4000, 4000, crate::cache::MAX_BYTES + 1), 2);
}

#[test]
fn each_preview_pixel_is_the_average_of_its_block() {
    // 5 x 3 in blocks of 2: the last column and row are blocks of their own size.
    let mut data = Vec::new();
    for y in 0..3u8 {
        for x in 0..5u8 {
            data.extend_from_slice(&[x * 50, y * 100, 7, 255]);
        }
    }
    let pixmap = Pixmap::from_vec(data, IntSize::from_wh(5, 3).expect("size")).expect("pixmap");
    let mut source = MemoryRows { pixmap, next: 0 };
    let mut out = Vec::new();
    downscale(&mut source, 2, &mut out).expect("preview");
    let preview = image::load_from_memory(&out).expect("png").into_rgba8();
    assert_eq!(preview.dimensions(), (3, 2));
    // Columns 0-1, rows 0-1: red (0 + 50) / 2 = 25, green (0 + 100) / 2 = 50.
    assert_eq!(preview.get_pixel(0, 0).0, [25, 50, 7, 255]);
    // Column 4 alone, rows 0-1.
    assert_eq!(preview.get_pixel(2, 0).0, [200, 50, 7, 255]);
    // Columns 2-3, row 2 alone.
    assert_eq!(preview.get_pixel(1, 1).0, [125, 200, 7, 255]);
    assert_eq!(out[25], 2, "an opaque source: RGB");
}

#[test]
fn the_preview_reads_the_image_in_ranges() {
    let bytes = png(9, 7, |x, y| [(x * 20) as u8, (y * 30) as u8, 0, 255]);
    let mut host = MemoryHost::new()
        .grant(Capability::ProjectRead)
        .with_file(".rbrowser/out/screenshots/a.png", &bytes);
    let preview = encode(&mut host, ".rbrowser/out/screenshots/a.png", 3).expect("preview");
    let shown = image::load_from_memory(&preview.png)
        .expect("png")
        .into_rgba8();
    // The fingerprint of the bytes it was made from: the whole file, read once in order.
    let whole = crate::cache::fnv(crate::cache::FNV_START, &bytes);
    assert_eq!(preview.hash, Some(format!("{whole:016x}")));
    assert_eq!(shown.dimensions(), (3, 3));
    assert!(!host.ranges.is_empty(), "read in ranges");
    let refused = encode(&mut MemoryHost::new(), ".rbrowser/out/screenshots/a.png", 3)
        .expect_err("no project-read");
    assert_eq!(
        refused.code(),
        Some(rb_plugin_api::codes::CAPABILITY_DENIED)
    );
}
