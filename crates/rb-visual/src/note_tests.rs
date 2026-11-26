use super::*;
use crate::raster::{Rgba, decode_png, encode_png};

fn png() -> Vec<u8> {
    let image = Rgba::new(2, 3, [9, 8, 7, 255]).expect("small");
    encode_png(&image).expect("encoded")
}

#[test]
fn a_note_is_read_back_and_the_image_is_unchanged() {
    let plain = png();
    let noted = with_note(&plain, IMAGES_PENDING_KEY, "4000").expect("a PNG");
    assert_eq!(note(&noted, IMAGES_PENDING_KEY).as_deref(), Some("4000"));
    assert!(images_pending(&noted));
    let (before, after) = (
        decode_png(&plain).expect("decodes"),
        decode_png(&noted).expect("decodes"),
    );
    assert_eq!(before.pixels(), after.pixels());
    assert_eq!(crate::png_size(&noted), Some((2, 3)));
}

#[test]
fn a_png_without_the_note_says_nothing() {
    let plain = png();
    assert_eq!(note(&plain, IMAGES_PENDING_KEY), None);
    assert!(!images_pending(&plain));
    let other = with_note(&plain, "rbrowser-other", "x").expect("a PNG");
    assert!(!images_pending(&other));
}

#[test]
fn a_note_beside_the_cut_of_a_stitched_page_keeps_both() {
    let section = png();
    let parts = [crate::stitch::Section {
        png: &section,
        rows: 3,
    }];
    let cut = crate::Truncated {
        page_height: 90_000,
        captured_height: 3,
    };
    let stitched = crate::stitch(&parts, Some(cut)).expect("stitched");
    let noted = with_note(&stitched, IMAGES_PENDING_KEY, "1").expect("a PNG");
    assert_eq!(crate::truncation(&noted), Some(cut));
    assert!(images_pending(&noted));
}

#[test]
fn anything_but_a_png_or_a_key_too_long_is_refused() {
    assert_eq!(with_note(b"\x89PNG fake", IMAGES_PENDING_KEY, "1"), None);
    assert_eq!(with_note(b"", IMAGES_PENDING_KEY, "1"), None);
    assert_eq!(with_note(&png(), &"k".repeat(80), "1"), None);
    assert!(!images_pending(b"no png"));
}
