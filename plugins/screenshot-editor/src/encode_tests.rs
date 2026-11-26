//! The band encoders against the whole-image ones (RB-099-24).

use serde_json::json;

use super::*;
use crate::band::tests::{annotations, pattern};
use crate::band::{Bands, plan};
use crate::codec;
use crate::render;
use crate::rows::MemoryRows;

fn list() -> Vec<crate::model::Annotation> {
    annotations(json!([
        { "type": "redact", "x": 3, "y": 5, "w": 30, "h": 41, "mode": "blur", "block": 8 },
        { "type": "redact", "x": 10, "y": 30, "w": 30, "h": 30, "mode": "pixelate", "block": 13 },
        { "type": "crop", "x": 1, "y": 2, "w": 45, "h": 60 }
    ]))
}

/// Encodes `list` on `image` in bands of `band_rows` with `encode`.
fn encoded(
    image: &tiny_skia::Pixmap,
    band_rows: u32,
    encode: impl FnOnce(&mut Bands<'_, '_>, &mut Vec<u8>) -> Result<(), Failure>,
) -> Vec<u8> {
    let list = list();
    let plan = plan(&list, image.width(), image.height(), band_rows).expect("plan");
    let mut source = MemoryRows {
        pixmap: image.clone(),
        next: 0,
    };
    let mut bands = Bands::new(&mut source, &list, &plan);
    let mut out = Vec::new();
    encode(&mut bands, &mut out).expect("encoded");
    out
}

#[test]
fn a_png_in_bands_holds_the_whole_images_pixels() {
    let image = pattern(48, 64);
    let whole = render::render(image.clone(), &list()).expect("whole");
    let expected = image::load_from_memory(&codec::encode_png(&whole).expect("png"))
        .expect("decode")
        .into_rgba8();
    for band_rows in [8, 24] {
        let png = encoded(&image, band_rows, |bands, out| png(bands, out));
        let decoded = image::load_from_memory(&png).expect("decode").into_rgba8();
        assert_eq!(decoded.dimensions(), (45, 60));
        assert_eq!(decoded.as_raw(), expected.as_raw(), "bands of {band_rows}");
        // An opaque source: RGB (colour type 2 in the header), no text chunk, no time.
        assert_eq!(png[25], 2);
        let types = crate::testing::chunk_types(&png);
        assert!(
            types
                .iter()
                .all(|kind| ["IHDR", "IDAT", "IEND"].contains(&kind.as_str()))
        );
    }
}

#[test]
fn a_jpeg_in_bands_is_the_whole_images_jpeg_byte_for_byte() {
    let image = pattern(48, 64);
    let whole = render::render(image.clone(), &list()).expect("whole");
    let expected = codec::encode_jpeg(&whole, 85).expect("jpeg");
    for band_rows in [8, 16, 64] {
        let jpeg = encoded(&image, band_rows, |bands, out| jpeg(bands, 85, out));
        assert_eq!(jpeg, expected, "bands of {band_rows}");
    }
}

#[test]
fn a_transparent_source_keeps_its_alpha_and_lies_over_white_in_a_jpeg() {
    let mut image = pattern(16, 16);
    for pixel in image.pixels_mut().iter_mut().take(16) {
        *pixel = tiny_skia::PremultipliedColorU8::from_rgba(0, 0, 0, 0).expect("clear");
    }
    let png = encoded(&image, 8, |bands, out| png(bands, out));
    assert_eq!(png[25], 6, "RGBA");
    let mut rows = Vec::new();
    straight(&[0, 0, 0, 0, 10, 20, 30, 255], true, &mut rows);
    assert_eq!(rows, [0, 0, 0, 10, 20, 30]);
    let mut white = Vec::new();
    over_white(&[0, 0, 0, 0, 10, 20, 30, 255], &mut white);
    assert_eq!(white, [255, 255, 255, 10, 20, 30]);
}
