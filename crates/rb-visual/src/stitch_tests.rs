use super::*;
use crate::raster::{Rgba, decode_png, encode_png};

/// A `width` x `rows` section whose row `n` is the colour `[first + n, 0, 0, 255]`.
fn section(width: u32, first: u8, rows: u8) -> Vec<u8> {
    let mut image = Rgba::new(width, u32::from(rows), [0, 0, 0, 255]).expect("small");
    for row in 0..rows {
        image.fill(0, u32::from(row), width, 1, [first + row, 0, 0, 255]);
    }
    encode_png(&image).expect("encoded")
}

/// The red channel of each row of `png`, top to bottom.
fn rows_of(png: &[u8]) -> Vec<u8> {
    let image = decode_png(png).expect("decodes");
    (0..image.height())
        .map(|row| image.get(0, row).expect("inside")[0])
        .collect()
}

#[test]
fn sections_are_stacked_top_to_bottom_in_their_order() {
    let first = section(3, 0, 4);
    let second = section(3, 4, 4);
    let last = section(3, 8, 2);
    let parts = [
        Section {
            png: &first,
            rows: 4,
        },
        Section {
            png: &second,
            rows: 4,
        },
        Section {
            png: &last,
            rows: 2,
        },
    ];
    let png = stitch(&parts, None).expect("stitched");
    assert_eq!(crate::png_size(&png), Some((3, 10)));
    assert_eq!(rows_of(&png), (0..10).collect::<Vec<u8>>());
    assert_eq!(truncation(&png), None);
}

#[test]
fn a_section_with_a_row_more_than_planned_is_cut_to_its_plan() {
    // A rounding renderer gave one row too many: the next section starts with it already.
    let first = section(2, 0, 5);
    let second = section(2, 4, 3);
    let parts = [
        Section {
            png: &first,
            rows: 4,
        },
        Section {
            png: &second,
            rows: 3,
        },
    ];
    let png = stitch(&parts, None).expect("stitched");
    assert_eq!(rows_of(&png), [0, 1, 2, 3, 4, 5, 6], "no doubled row");
}

#[test]
fn a_section_a_row_short_repeats_its_last_row_and_keeps_the_seam() {
    let first = section(2, 0, 3);
    let second = section(2, 4, 2);
    let parts = [
        Section {
            png: &first,
            rows: 4,
        },
        Section {
            png: &second,
            rows: 2,
        },
    ];
    let png = stitch(&parts, None).expect("stitched");
    assert_eq!(rows_of(&png), [0, 1, 2, 2, 4, 5]);
}

#[test]
fn a_section_far_shorter_than_planned_fails() {
    let first = section(2, 0, 3);
    let parts = [Section {
        png: &first,
        rows: 3 + MAX_PAD_ROWS + 1,
    }];
    let error = stitch(&parts, None).expect_err("too short");
    assert!(error.to_string().contains("rows"), "{error}");
}

#[test]
fn sections_of_another_width_fail() {
    let first = section(3, 0, 2);
    let second = section(4, 2, 2);
    let parts = [
        Section {
            png: &first,
            rows: 2,
        },
        Section {
            png: &second,
            rows: 2,
        },
    ];
    let error = stitch(&parts, None).expect_err("widths differ");
    assert!(error.to_string().contains("width"), "{error}");
}

#[test]
fn no_section_is_an_error() {
    assert!(stitch(&[], None).is_err());
}

#[test]
fn a_cut_image_says_how_tall_the_page_was() {
    let first = section(2, 0, 2);
    let parts = [Section {
        png: &first,
        rows: 2,
    }];
    let cut = Truncated {
        page_height: 52_000,
        captured_height: 43_690,
    };
    let png = stitch(&parts, Some(cut)).expect("stitched");
    assert_eq!(truncation(&png), Some(cut));
    assert_eq!(rows_of(&png), [0, 1], "the pixels are the same");
    assert_eq!(truncation(b"not a png"), None);
}

#[test]
fn rgb_and_grey_sections_become_rgba() {
    let mut rgb = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut rgb, 2, 1);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().expect("header");
        writer
            .write_image_data(&[9, 8, 7, 9, 8, 7])
            .expect("written");
        writer.finish().expect("finished");
    }
    let mut grey = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut grey, 2, 1);
        encoder.set_color(png::ColorType::Grayscale);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().expect("header");
        writer.write_image_data(&[5, 5]).expect("written");
        writer.finish().expect("finished");
    }
    let parts = [
        Section { png: &rgb, rows: 1 },
        Section {
            png: &grey,
            rows: 1,
        },
    ];
    let image = decode_png(&stitch(&parts, None).expect("stitched")).expect("decodes");
    assert_eq!(image.get(1, 0), Some([9, 8, 7, 255]));
    assert_eq!(image.get(1, 1), Some([5, 5, 5, 255]));
}

#[test]
fn the_top_of_a_tall_image_is_read_without_the_rest() {
    let png = section(2, 0, 9);
    let (top, height) = decode_top(&png, 4).expect("decoded");
    assert_eq!(height, 9);
    assert_eq!((top.width(), top.height()), (2, 4));
    assert_eq!(top.get(1, 3), Some([3, 0, 0, 255]));
    let (whole, height) = decode_top(&png, 100).expect("decoded");
    assert_eq!((whole.height(), height), (9, 9));
    assert!(decode_top(b"not a png", 4).is_err());
}
