use std::cell::RefCell;

use rb_plugin_api::test_support::MemoryHost;
use rb_plugin_api::{Capability, PluginHost};

use super::*;
use crate::stream::Shared;
use crate::testing::{header_only, png as rgba_png};

const SHOT: &str = ".rbrowser/out/screenshots/rows.png";

/// A PNG of `color` and `depth` with `data` as its raw samples, `palette` and `trns` when given.
fn encoded(
    width: u32,
    height: u32,
    color: ColorType,
    depth: BitDepth,
    data: &[u8],
    palette: Option<&[u8]>,
    trns: Option<&[u8]>,
) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, width, height);
        encoder.set_color(color);
        encoder.set_depth(depth);
        if let Some(palette) = palette {
            encoder.set_palette(palette.to_vec());
        }
        if let Some(trns) = trns {
            encoder.set_trns(trns.to_vec());
        }
        let mut writer = encoder.write_header().expect("header");
        writer.write_image_data(data).expect("data");
    }
    out
}

/// Every row of the PNG `bytes` through [`PngRows`], read in ranges.
fn rows_of(bytes: &[u8]) -> Result<(u32, u32, bool, Vec<u8>), Failure> {
    let mut host = MemoryHost::new()
        .grant(Capability::ProjectRead)
        .with_file(SHOT, bytes);
    let shared: Shared<'_> = RefCell::new(&mut host as &mut dyn PluginHost);
    let mut rows = PngRows::open(&shared, SHOT)?;
    let (width, height) = rows.size();
    let mut row = vec![0; width as usize * 4];
    let mut all = Vec::new();
    for _ in 0..height {
        rows.next_row(&mut row)?;
        all.extend_from_slice(&row);
    }
    Ok((width, height, rows.opaque(), all))
}

#[test]
fn every_colour_type_reads_as_the_whole_image_decodes() {
    let gray = encoded(
        3,
        2,
        ColorType::Grayscale,
        BitDepth::Eight,
        &[0, 100, 255, 7, 8, 9],
        None,
        None,
    );
    let gray_alpha = encoded(
        2,
        1,
        ColorType::GrayscaleAlpha,
        BitDepth::Eight,
        &[200, 128, 50, 0],
        None,
        None,
    );
    let rgb16 = encoded(
        1,
        1,
        ColorType::Rgb,
        BitDepth::Sixteen,
        &[0xff, 0xff, 0x10, 0x10, 0, 0],
        None,
        None,
    );
    let palette = encoded(
        2,
        1,
        ColorType::Indexed,
        BitDepth::Eight,
        &[0, 1],
        Some(&[255, 0, 0, 0, 0, 255]),
        Some(&[255, 64]),
    );
    let rgba = rgba_png(5, 3, |x, y| {
        [
            x as u8 * 40,
            y as u8 * 60,
            9,
            if x == 2 { 100 } else { 255 },
        ]
    });
    for (name, bytes, opaque) in [
        ("gray", gray, true),
        ("gray-alpha", gray_alpha, false),
        ("rgb16", rgb16, true),
        ("palette", palette, false),
        ("rgba", rgba, false),
    ] {
        let (width, height, seen, pixels) = rows_of(&bytes).expect(name);
        let whole = codec::decode(&bytes).expect(name);
        assert_eq!((width, height), (whole.width(), whole.height()), "{name}");
        assert_eq!(pixels, whole.data(), "{name}");
        assert_eq!(seen, opaque, "{name}");
    }
}

#[test]
fn an_image_over_one_range_reads_row_by_row() {
    // Noise does not compress: the PNG spans several ranges of `READ_BYTES`.
    let bytes = rgba_png(1600, 1000, crate::testing::noise);
    assert!(bytes.len() > crate::stream::READ_BYTES, "{}", bytes.len());
    let (width, height, opaque, pixels) = rows_of(&bytes).expect("rows");
    assert_eq!((width, height), (1600, 1000));
    assert!(!opaque, "RGBA by its format");
    assert_eq!(pixels, codec::decode(&bytes).expect("whole").data());
}

#[test]
fn the_header_is_held_to_the_limits_before_a_row_is_read() {
    let refused = rows_of(&header_only(70_000, 2)).err().expect("too wide");
    assert_eq!(
        refused.code(),
        Some("plugin.screenshot-editor.errors.image_too_large")
    );
    let broken = rows_of(b"not a png at all").err().expect("not a png");
    assert_eq!(
        broken.code(),
        Some("plugin.screenshot-editor.errors.not_png")
    );
    // A header without its rows: the first row fails, nothing is made up.
    let mut host = MemoryHost::new()
        .grant(Capability::ProjectRead)
        .with_file(SHOT, &header_only(4, 4));
    let shared: Shared<'_> = RefCell::new(&mut host as &mut dyn PluginHost);
    assert!(PngRows::open(&shared, SHOT).is_err());
}
