use tiny_skia::{IntSize, Pixmap};

use super::*;
use crate::model::RedactMode;

/// An opaque pixmap with `pixel(x, y)` (RGB).
fn pixmap(width: u32, height: u32, pixel: impl Fn(u32, u32) -> [u8; 3]) -> Pixmap {
    let mut data = Vec::new();
    for y in 0..height {
        for x in 0..width {
            let [r, g, b] = pixel(x, y);
            data.extend_from_slice(&[r, g, b, 255]);
        }
    }
    Pixmap::from_vec(data, IntSize::from_wh(width, height).expect("size")).expect("pixmap")
}

fn get(pixmap: &Pixmap, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * pixmap.width() + x) * 4) as usize;
    let d = pixmap.data();
    [d[i], d[i + 1], d[i + 2], d[i + 3]]
}

/// Black-and-white "text": one pixel wide strokes the mosaic must not keep.
fn secret(x: u32, y: u32) -> [u8; 3] {
    if (x + 2 * y) % 3 == 0 || (x * y) % 7 == 1 {
        [0, 0, 0]
    } else {
        [255, 255, 255]
    }
}

#[test]
fn a_region_is_clipped_to_the_image_and_rounded_outward() {
    assert_eq!(
        Region::clip(-5.0, 2.5, 20.0, 3.2, 10, 10),
        Some(Region {
            left: 0,
            top: 2,
            right: 10,
            bottom: 6
        })
    );
    assert_eq!(Region::clip(12.0, 0.0, 5.0, 5.0, 10, 10), None);
    assert_eq!(Region::clip(2.0, 2.0, 0.0, 5.0, 10, 10), None);
}

#[test]
fn the_mosaic_is_the_expected_block_averages_and_the_original_pattern_is_gone() {
    let (width, height, block) = (40, 30, 8);
    let original = pixmap(width, height, secret);
    let mut redacted = original.clone();
    let region = Region {
        left: 4,
        top: 3,
        right: 36,
        bottom: 27,
    };
    pixelate(&mut redacted, region, block, 0);

    // The expected mosaic, computed here on its own.
    let mut top = region.top;
    while top < region.bottom {
        let bottom = (top + block).min(region.bottom);
        let mut left = region.left;
        while left < region.right {
            let right = (left + block).min(region.right);
            let (mut sum, mut count) = (0u32, 0u32);
            for y in top..bottom {
                for x in left..right {
                    sum += u32::from(secret(x, y)[0]);
                    count += 1;
                }
            }
            let expected = u8::try_from((sum + count / 2) / count).expect("average");
            for y in top..bottom {
                for x in left..right {
                    assert_eq!(
                        get(&redacted, x, y),
                        [expected, expected, expected, 255],
                        "{x},{y}"
                    );
                }
            }
            left = right;
        }
        top = bottom;
    }
    // Outside the region nothing changed.
    assert_eq!(get(&redacted, 0, 0), get(&original, 0, 0));
    assert_eq!(get(&redacted, 39, 29), get(&original, 39, 29));
    // No row of the original region occurs in the redacted region.
    let row = |p: &Pixmap, y: u32| -> Vec<u8> {
        let from = ((y * width + region.left) * 4) as usize;
        let to = ((y * width + region.right) * 4) as usize;
        p.data()[from..to].to_vec()
    };
    let redacted_rows: Vec<Vec<u8>> = (region.top..region.bottom)
        .map(|y| row(&redacted, y))
        .collect();
    for y in region.top..region.bottom {
        assert!(!redacted_rows.contains(&row(&original, y)), "row {y}");
    }
    // Pure black and white, which the strokes are made of, are gone from the region.
    for y in region.top..region.bottom {
        for x in region.left..region.right {
            let [r, ..] = get(&redacted, x, y);
            assert!(r != 0 && r != 255, "{x},{y}");
        }
    }
}

#[test]
fn the_smoothed_mode_depends_on_the_block_averages_only() {
    let region = Region {
        left: 0,
        top: 0,
        right: 24,
        bottom: 24,
    };
    let mut first = pixmap(24, 24, secret);
    // Another image with the same block averages: each block's pixels swapped around.
    let mut second = pixmap(24, 24, |x, y| {
        let (bx, by) = (x / 8 * 8, y / 8 * 8);
        secret(bx + 7 - (x - bx), by + 7 - (y - by))
    });
    blur(&mut first, region, 8, 0);
    blur(&mut second, region, 8, 0);
    assert_eq!(first.data(), second.data());
    // And it is not just the mosaic.
    let mut mosaic = pixmap(24, 24, secret);
    pixelate(&mut mosaic, region, 8, 0);
    assert_ne!(first.data(), mosaic.data());
}

#[test]
fn a_black_bar_is_opaque_black() {
    let mut image = pixmap(10, 10, secret);
    black(
        &mut image,
        Region {
            left: 2,
            top: 2,
            right: 5,
            bottom: 4,
        },
        0,
    );
    for y in 2..4 {
        for x in 2..5 {
            assert_eq!(get(&image, x, y), [0, 0, 0, 255]);
        }
    }
    assert_eq!(get(&image, 5, 4), {
        let [r, g, b] = secret(5, 4);
        [r, g, b, 255]
    });
}

/// The rows `top..bottom` of `image` as a pixmap of their own: a band.
fn band(image: &Pixmap, top: u32, bottom: u32) -> Pixmap {
    let stride = (image.width() * 4) as usize;
    let data = image.data()[top as usize * stride..bottom as usize * stride].to_vec();
    let size = tiny_skia::IntSize::from_wh(image.width(), bottom - top).expect("size");
    Pixmap::from_vec(data, size).expect("band")
}

#[test]
fn a_band_with_its_margin_redacts_its_rows_as_the_whole_image_does() {
    // RB-099-24: a region crossing the band's edges, blocks not starting at the band's top.
    let region = Region {
        left: 3,
        top: 5,
        right: 37,
        bottom: 58,
    };
    for (mode, block) in [
        (RedactMode::Pixelate, 8),
        (RedactMode::Pixelate, 13),
        (RedactMode::Blur, 8),
        (RedactMode::Blur, 12),
        (RedactMode::Black, 8),
    ] {
        let original = pixmap(40, 64, secret);
        let mut whole = original.clone();
        apply(&mut whole, mode, region, block, 0);
        let margin = margin(mode, block);
        for (from, to) in [(0u32, 9u32), (9, 22), (22, 23), (23, 50), (50, 64)] {
            let top = from.saturating_sub(margin);
            let bottom = (to + margin).min(64);
            let mut part = band(&original, top, bottom);
            apply(&mut part, mode, region, block, top);
            for y in from..to {
                for x in 0..40 {
                    assert_eq!(
                        get(&part, x, y - top),
                        get(&whole, x, y),
                        "{mode:?} block {block}: pixel {x},{y} of band {from}..{to}"
                    );
                }
            }
        }
    }
}

fn apply(pixmap: &mut Pixmap, mode: RedactMode, region: Region, block: u32, top: u32) {
    match mode {
        RedactMode::Pixelate => pixelate(pixmap, region, block, top),
        RedactMode::Blur => blur(pixmap, region, block, top),
        RedactMode::Black => black(pixmap, region, top),
    }
}

#[test]
fn a_band_outside_the_region_is_left_alone() {
    let original = pixmap(10, 10, secret);
    let mut part = original.clone();
    let region = Region {
        left: 0,
        top: 0,
        right: 10,
        bottom: 4,
    };
    // The pixmap holds the image's rows 20..30: nothing of the region.
    blur(&mut part, region, 8, 20);
    pixelate(&mut part, region, 8, 20);
    black(&mut part, region, 20);
    assert_eq!(part.data(), original.data());
}
