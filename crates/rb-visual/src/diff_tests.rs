use std::path::PathBuf;

use super::*;
use crate::{decode_png, encode_png};

const WHITE: [u8; 4] = [255, 255, 255, 255];
const BLACK: [u8; 4] = [0, 0, 0, 255];

/// A white 32 x 16 image with a black bar over the columns `left..right`, rows 4 to 11.
fn bar(left: u32, right: u32) -> Rgba {
    let mut image = Rgba::new(32, 16, WHITE).expect("image");
    image.fill(left, 4, right - left, 8, BLACK);
    image
}

/// A white 32 x 16 image, its right half black; with `smooth` the first black column grey, as a
/// renderer smoothing the edge paints it.
fn edge(smooth: bool) -> Rgba {
    let mut image = Rgba::new(32, 16, WHITE).expect("image");
    image.fill(16, 0, 16, 16, BLACK);
    if smooth {
        image.fill(16, 0, 1, 16, [128, 128, 128, 255]);
    }
    image
}

fn compared(outcome: DiffOutcome) -> PixelDiff {
    match outcome {
        DiffOutcome::Compared(diff) => diff,
        DiffOutcome::SizeMismatch { .. } => panic!("the sizes match"),
    }
}

fn red_pixels(image: &Rgba) -> usize {
    image
        .pixels()
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|pixel| **pixel == DIFF_COLOR)
        .count()
}

/// `image` against `tests/golden/<name>.png`; `RB_UPDATE_GOLDEN=1` writes the file anew after an
/// intended change of the diff image.
fn assert_golden(image: &Rgba, name: &str) {
    let golden = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden")
        .join(format!("{name}.png"));
    if std::env::var_os("RB_UPDATE_GOLDEN").is_some() {
        std::fs::write(&golden, encode_png(image).expect("encoded")).expect("golden written");
    }
    let expected = decode_png(&std::fs::read(&golden).expect("golden file")).expect("golden png");
    assert_eq!(
        *image,
        expected,
        "the diff image differs from {}",
        golden.display()
    );
}

#[test]
fn identical_images_have_no_difference_and_a_grey_diff_image() {
    let image = bar(8, 20);
    let diff = compared(diff(&image, &image, &DiffOptions::default()).expect("diff"));
    assert_eq!((diff.different, diff.antialiased, diff.total), (0, 0, 512));
    assert!(diff.regions.is_empty());
    assert_eq!(diff.ratio(), 0.0);
    // White stays white, black fades to a light grey.
    assert_eq!(diff.image.get(0, 0), Some(WHITE));
    assert_eq!(diff.image.get(10, 6), Some([230, 230, 230, 255]));
}

#[test]
fn a_one_pixel_shift_is_a_difference() {
    let diff = compared(diff(&bar(8, 20), &bar(9, 21), &DiffOptions::default()).expect("diff"));
    // The column the bar left and the column it reached, eight rows each.
    assert_eq!(diff.different, 16);
    assert_eq!(diff.antialiased, 0);
    assert_eq!(red_pixels(&diff.image), 16);
    assert_eq!(
        diff.regions,
        [Region {
            x: 8,
            y: 4,
            width: 13,
            height: 8,
            pixels: 16,
        }]
    );
    assert!((diff.ratio() - 16.0 / 512.0).abs() < 1e-12);
    assert_golden(&diff.image, "diff-shifted");
}

#[test]
fn a_smoothed_edge_is_anti_aliasing_not_a_difference() {
    let diff = compared(diff(&edge(false), &edge(true), &DiffOptions::default()).expect("diff"));
    assert_eq!(diff.different, 0);
    assert_eq!(diff.antialiased, 16);
    assert!(diff.regions.is_empty());
    assert_golden(&diff.image, "diff-antialiased");
}

#[test]
fn counting_anti_aliasing_makes_the_smoothed_edge_differ() {
    let options = DiffOptions {
        include_aa: true,
        ..DiffOptions::default()
    };
    let diff = compared(diff(&edge(false), &edge(true), &options).expect("diff"));
    assert_eq!((diff.different, diff.antialiased), (16, 0));
}

#[test]
fn a_changed_button_colour_is_boxed_exactly() {
    let mut before = Rgba::new(120, 80, WHITE).expect("image");
    before.fill(30, 40, 50, 20, [0, 90, 200, 255]);
    let mut after = before.clone();
    after.fill(30, 40, 50, 20, [200, 30, 30, 255]);
    let diff = compared(diff(&before, &after, &DiffOptions::default()).expect("diff"));
    assert_eq!(diff.different, 1000);
    assert_eq!(
        diff.regions,
        [Region {
            x: 30,
            y: 40,
            width: 50,
            height: 20,
            pixels: 1000,
        }]
    );
}

#[test]
fn the_threshold_decides_what_a_small_colour_change_is() {
    let before = Rgba::new(4, 4, [200, 200, 200, 255]).expect("image");
    let after = Rgba::new(4, 4, [190, 190, 190, 255]).expect("image");
    let strict = DiffOptions {
        threshold: 0.0,
        ..DiffOptions::default()
    };
    let loose = DiffOptions::default();
    assert_eq!(
        compared(diff(&before, &after, &strict).expect("diff")).different,
        16
    );
    assert_eq!(
        compared(diff(&before, &after, &loose).expect("diff")).different,
        0
    );
}

#[test]
fn a_threshold_out_of_range_is_the_default() {
    for threshold in [-0.5, 1.5, f64::NAN] {
        let options = DiffOptions {
            threshold,
            ..DiffOptions::default()
        };
        assert_eq!(options.threshold(), DEFAULT_THRESHOLD);
    }
    let options = DiffOptions {
        threshold: 0.3,
        ..DiffOptions::default()
    };
    assert_eq!(options.threshold(), 0.3);
}

#[test]
fn different_sizes_are_a_size_mismatch_not_a_failure() {
    let small = Rgba::new(10, 20, WHITE).expect("image");
    let large = Rgba::new(10, 25, WHITE).expect("image");
    assert_eq!(
        diff(&small, &large, &DiffOptions::default()).expect("diff"),
        DiffOutcome::SizeMismatch {
            baseline: Size {
                width: 10,
                height: 20,
            },
            actual: Size {
                width: 10,
                height: 25,
            },
        }
    );
}

#[test]
fn transparent_pixels_compare_as_on_white() {
    let clear = Rgba::new(4, 4, [0, 0, 0, 0]).expect("image");
    let white = Rgba::new(4, 4, WHITE).expect("image");
    let diff = compared(diff(&clear, &white, &DiffOptions::default()).expect("diff"));
    assert_eq!(diff.different, 0);
}
