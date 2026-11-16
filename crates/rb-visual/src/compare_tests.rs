use std::path::PathBuf;

use serde_json::json;

use super::*;
use crate::Rgba;

const WHITE: [u8; 4] = [255, 255, 255, 255];

fn subject() -> Subject {
    Subject {
        view: 2,
        label: "iPhone 15".to_owned(),
        breakpoint: "mobile".to_owned(),
        slug: "index".to_owned(),
        url: "http://localhost:5173/".to_owned(),
    }
}

fn meta(engine_version: &str) -> BaselineMeta {
    BaselineMeta {
        engine: "chromium".to_owned(),
        engine_version: engine_version.to_owned(),
        dpr: 1.0,
        ..BaselineMeta::default()
    }
}

/// A page with a button of `color`.
fn page(color: [u8; 4]) -> Vec<u8> {
    let mut image = Rgba::new(60, 40, WHITE).expect("image");
    image.fill(10, 10, 20, 8, color);
    encode_png(&image).expect("png")
}

fn baseline(png: Vec<u8>, meta: Option<BaselineMeta>) -> Baseline {
    Baseline {
        png,
        meta,
        path: PathBuf::from(".rbrowser/baselines/index/mobile.png"),
    }
}

const BLUE: [u8; 4] = [0, 90, 200, 255];
const RED: [u8; 4] = [200, 30, 30, 255];

#[test]
fn an_unchanged_page_is_ok_without_a_diff_image() {
    let stored = baseline(page(BLUE), Some(meta("156")));
    let found = compare(
        subject(),
        Some(&stored),
        &page(BLUE),
        &meta("156"),
        &DiffOptions::default(),
    )
    .expect("compared");
    assert_eq!(found.comparison.status, Status::Ok);
    assert!(!found.comparison.unreliable());
    assert_eq!(found.diff_png, None);
    assert_eq!(found.comparison.total_pixels, 2400);
}

#[test]
fn a_changed_button_colour_differs_with_a_diff_image_and_its_box() {
    let stored = baseline(page(BLUE), Some(meta("156")));
    let found = compare(
        subject(),
        Some(&stored),
        &page(RED),
        &meta("156"),
        &DiffOptions::default(),
    )
    .expect("compared");
    let comparison = &found.comparison;
    assert_eq!(comparison.status, Status::Differs);
    assert!(comparison.status.is_regression());
    assert_eq!(comparison.different_pixels, 160);
    assert_eq!(comparison.percent(), 6.67);
    assert_eq!(
        comparison.regions,
        [Region {
            x: 10,
            y: 10,
            width: 20,
            height: 8,
            pixels: 160,
        }]
    );
    let diff = decode_png(found.diff_png.as_deref().expect("a diff image")).expect("png");
    assert_eq!(diff.get(15, 12), Some(crate::diff::DIFF_COLOR));
}

#[test]
fn no_baseline_is_new() {
    let found = compare(
        subject(),
        None,
        &page(BLUE),
        &meta("156"),
        &DiffOptions::default(),
    )
    .expect("compared");
    assert_eq!(found.comparison.status, Status::New);
    assert!(!found.comparison.status.is_regression());
    assert_eq!(
        found.comparison.actual_size,
        Some(Size {
            width: 60,
            height: 40,
        })
    );
}

#[test]
fn another_size_is_a_size_mismatch_naming_both() {
    let stored = baseline(page(BLUE), Some(meta("156")));
    let taller = encode_png(&Rgba::new(60, 50, WHITE).expect("image")).expect("png");
    let found = compare(
        subject(),
        Some(&stored),
        &taller,
        &meta("156"),
        &DiffOptions::default(),
    )
    .expect("compared");
    assert_eq!(found.comparison.status, Status::SizeMismatch);
    assert!(found.comparison.status.is_regression());
    assert_eq!(
        found.comparison.baseline_size.map(|size| size.height),
        Some(40)
    );
    assert_eq!(
        found.comparison.actual_size.map(|size| size.height),
        Some(50)
    );
}

#[test]
fn a_size_mismatch_is_found_from_the_headers_without_decoding() {
    // Only the header of a huge baseline: it is never decoded, so no allocation of its pixels.
    let huge = encode_png(&Rgba::new(60, 50, WHITE).expect("image")).expect("png");
    let mut header = huge[..33].to_vec();
    header[16..20].copy_from_slice(&60_000_u32.to_be_bytes());
    header[20..24].copy_from_slice(&4_000_u32.to_be_bytes());
    let stored = baseline(header, Some(meta("156")));
    let found = compare(
        subject(),
        Some(&stored),
        &page(BLUE),
        &meta("156"),
        &DiffOptions::default(),
    )
    .expect("compared from the headers");
    assert_eq!(found.comparison.status, Status::SizeMismatch);
    assert_eq!(
        found.comparison.baseline_size,
        Some(Size {
            width: 60_000,
            height: 4_000,
        })
    );
}

#[test]
fn a_baseline_that_is_no_png_is_an_error() {
    let stored = baseline(b"not a png".to_vec(), Some(meta("156")));
    assert!(
        compare(
            subject(),
            Some(&stored),
            &page(BLUE),
            &meta("156"),
            &DiffOptions::default()
        )
        .is_err()
    );
}

#[test]
fn another_engine_version_makes_the_comparison_unreliable() {
    let stored = baseline(page(BLUE), Some(meta("155")));
    let found = compare(
        subject(),
        Some(&stored),
        &page(BLUE),
        &meta("156"),
        &DiffOptions::default(),
    )
    .expect("compared");
    assert_eq!(found.comparison.status, Status::Ok);
    assert!(found.comparison.unreliable());
    assert_eq!(found.comparison.meta_mismatches[0].field, "engineVersion");
    let stored = baseline(page(BLUE), None);
    let found = compare(
        subject(),
        Some(&stored),
        &page(BLUE),
        &meta("156"),
        &DiffOptions::default(),
    )
    .expect("compared");
    assert!(found.comparison.meta_missing && found.comparison.unreliable());
}

#[test]
fn a_capture_that_is_no_png_is_an_error() {
    assert!(
        compare(
            subject(),
            None,
            b"not a png",
            &meta("156"),
            &DiffOptions::default()
        )
        .is_err()
    );
}

#[test]
fn the_record_is_camel_case_json_with_the_subject_flat() {
    let comparison = Comparison {
        status: Status::SizeMismatch,
        ratio: 0.5,
        ..Comparison::failed(subject(), "x".to_owned())
    };
    let value = serde_json::to_value(&comparison).expect("serializes");
    assert_eq!(value["view"], json!(2));
    assert_eq!(value["breakpoint"], json!("mobile"));
    assert_eq!(value["status"], json!("sizeMismatch"));
    assert_eq!(value["differentPixels"], json!(0));
    assert!(value.get("metaMissing").is_none());
    let back: Comparison = serde_json::from_value(value).expect("parses");
    assert_eq!(back, comparison);
}
