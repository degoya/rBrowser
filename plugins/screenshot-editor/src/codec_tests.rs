use super::*;
use crate::testing::{at, chunk_types, png, with_text};

fn colour(x: u32, y: u32) -> [u8; 4] {
    [(x * 20) as u8, (y * 30) as u8, 77, 255]
}

#[test]
fn a_png_decodes_to_its_pixels() {
    let pixmap = decode(&png(6, 4, colour)).expect("decode");
    assert_eq!((pixmap.width(), pixmap.height()), (6, 4));
    let rgba = pixmap.take_demultiplied();
    assert_eq!(at(&rgba, 6, 5, 3), colour(5, 3));
}

#[test]
fn the_header_names_the_size_before_any_pixel_is_decoded() {
    assert_eq!(dimensions(&png(6, 4, colour)), Ok((6, 4)));
    let wide = png(MAX_SIDE + 1, 1, |_, _| [0, 0, 0, 255]);
    let refused = dimensions(&wide).expect_err("too wide");
    assert_eq!(
        refused.code(),
        Some("plugin.screenshot-editor.errors.image_too_large")
    );
    assert_eq!(refused.param("width"), Some("16385"));
    assert!(decode(&wide).is_err());
}

#[test]
fn something_else_than_a_png_is_refused() {
    for bytes in [&b""[..], b"GIF89a....", b"\x89PNG\r\n\x1a\n"] {
        assert_eq!(
            decode(bytes).expect_err("not a png").code(),
            Some("plugin.screenshot-editor.errors.not_png")
        );
    }
}

#[test]
fn the_exported_png_has_pixels_and_no_text_chunk() {
    let original = with_text(&png(5, 5, colour), "Comment", "secret@example.com");
    assert!(chunk_types(&original).contains(&"tEXt".to_owned()));
    let pixmap = decode(&original).expect("decode");
    let exported = encode_png(&pixmap).expect("encode");
    let types = chunk_types(&exported);
    assert_eq!(types.first().map(String::as_str), Some("IHDR"));
    assert_eq!(types.last().map(String::as_str), Some("IEND"));
    assert!(
        types
            .iter()
            .all(|kind| ["IHDR", "IDAT", "IEND"].contains(&kind.as_str())),
        "{types:?}"
    );
    let back = decode(&exported).expect("again").take_demultiplied();
    assert_eq!(back, pixmap.take_demultiplied());
}

#[test]
fn transparency_survives_a_png_and_turns_white_in_a_jpeg() {
    let half = |_, _| [200, 0, 0, 128];
    let pixmap = decode(&png(3, 3, half)).expect("decode");
    let exported = decode(&encode_png(&pixmap).expect("png")).expect("again");
    let pixel = at(&exported.take_demultiplied(), 3, 1, 1);
    assert_eq!(pixel[3], 128);
    let jpeg = encode_jpeg(&pixmap, 90).expect("jpeg");
    assert_eq!(&jpeg[..2], &[0xFF, 0xD8]);
    let decoded = image::load_from_memory_with_format(&jpeg, ImageFormat::Jpeg)
        .expect("decode jpeg")
        .into_rgb8();
    let [r, g, b] = decoded.get_pixel(1, 1).0;
    // 200 at half over white: about 228, 127, 127.
    assert!((220..=236).contains(&r) && (118..=136).contains(&g) && (118..=136).contains(&b));
}

#[test]
fn a_jpeg_carries_no_exif_or_comment() {
    let pixmap = decode(&png(8, 8, colour)).expect("decode");
    let jpeg = encode_jpeg(&pixmap, 85).expect("jpeg");
    // APP1 (Exif, XMP) and COM markers.
    for marker in [[0xFF, 0xE1], [0xFF, 0xFE]] {
        assert!(!jpeg.windows(2).any(|pair| pair == marker), "{marker:?}");
    }
}

/// A pattern the best compression packs far tighter than the fast one.
fn pattern(x: u32, y: u32) -> [u8; 4] {
    let v = ((x % 13) * 19 + (y % 11) * 7 + (x / 7) % 3) as u8;
    [v, v.wrapping_mul(3), v ^ 0x5a, 255]
}

/// Charge 9 review L4: an edited image whose fast PNG passes the write limit is encoded again
/// with the best compression; one still too large is a clear error.
#[test]
fn a_png_over_the_limit_is_compressed_harder_or_refused_clearly() {
    let pixmap = decode(&png(300, 200, pattern)).expect("decode");
    let fast = encode_png_with(&pixmap, CompressionType::Fast).expect("fast");
    let best = encode_png_with(&pixmap, CompressionType::Best).expect("best");
    assert!(best.len() < fast.len(), "{} >= {}", best.len(), fast.len());
    assert_eq!(encode_png_within(&pixmap, fast.len()).expect("fits"), fast);
    let harder = encode_png_within(&pixmap, fast.len() - 1).expect("compressed harder");
    assert_eq!(harder, best);
    assert_eq!(
        decode(&harder).expect("again").take_demultiplied(),
        pixmap.clone().take_demultiplied()
    );
    let refused = encode_png_within(&pixmap, best.len() - 1).expect_err("too large");
    assert_eq!(
        refused.code(),
        Some("plugin.screenshot-editor.errors.output_too_large")
    );
    assert_eq!(
        refused.param("limit"),
        Some((best.len() - 1).to_string().as_str())
    );
}
