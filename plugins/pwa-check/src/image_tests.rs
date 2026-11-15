use super::*;

/// A PNG header of `width` x `height` (signature and IHDR; enough to sniff).
pub fn png(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
    bytes.extend_from_slice(&width.to_be_bytes());
    bytes.extend_from_slice(&height.to_be_bytes());
    bytes.extend_from_slice(&[8, 6, 0, 0, 0]);
    bytes
}

#[test]
fn png_gif_and_jpeg_give_their_size() {
    assert_eq!(
        sniff(&png(192, 192)),
        Some(Image::sized(Format::Png, 192, 192))
    );
    let gif = b"GIF89a\x40\x00\x20\x00".to_vec();
    assert_eq!(sniff(&gif), Some(Image::sized(Format::Gif, 64, 32)));
    // SOI, an APP0 segment of 4 bytes, then SOF0: precision, height 0x0100, width 0x0200.
    let jpeg = [
        0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x04, 0x00, 0x00, 0xFF, 0xC0, 0x00, 0x11, 0x08, 0x01, 0x00,
        0x02, 0x00,
    ];
    assert_eq!(sniff(&jpeg), Some(Image::sized(Format::Jpeg, 512, 256)));
}

#[test]
fn webp_reads_its_three_chunk_kinds() {
    let riff = |chunk: &[u8], rest: &[u8]| {
        let mut bytes = b"RIFF\0\0\0\0WEBP".to_vec();
        bytes.extend_from_slice(chunk);
        bytes.extend_from_slice(rest);
        bytes
    };
    // VP8X: flags and reserved (4 bytes) at 20, then width-1 and height-1 as 24-bit LE.
    let vp8x = riff(b"VP8X", &[10, 0, 0, 0, 0, 0, 0, 0, 191, 0, 0, 191, 0, 0]);
    assert_eq!(sniff(&vp8x), Some(Image::sized(Format::Webp, 192, 192)));
    // VP8 (lossy): frame tag (3) and start code (3) from 20, then 14-bit width and height.
    let vp8 = riff(
        b"VP8 ",
        &[
            0, 0, 0, 0, 0, 0, 0, 0x9D, 0x01, 0x2A, 0x00, 0x02, 0x00, 0x02,
        ],
    );
    assert_eq!(sniff(&vp8), Some(Image::sized(Format::Webp, 512, 512)));
    // VP8L (lossless): signature 0x2f at 20, then 14-bit width-1 and height-1 packed.
    let (w, h) = (191u32, 95u32);
    let bits = w | (h << 14);
    let mut rest = vec![0, 0, 0, 0, 0x2f];
    rest.extend_from_slice(&bits.to_le_bytes());
    let vp8l = riff(b"VP8L", &rest);
    assert_eq!(sniff(&vp8l), Some(Image::sized(Format::Webp, 192, 96)));
}

#[test]
fn ico_takes_its_largest_entry_and_svg_has_no_size() {
    let mut ico = vec![0, 0, 1, 0, 2, 0];
    ico.extend_from_slice(&[32, 32, 0, 0, 1, 0, 32, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    ico.extend_from_slice(&[0, 0, 0, 0, 1, 0, 32, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(sniff(&ico), Some(Image::sized(Format::Ico, 256, 256)));
    let svg = b"<?xml version=\"1.0\"?>\n<svg xmlns=\"http://www.w3.org/2000/svg\"/>";
    assert_eq!(
        sniff(svg),
        Some(Image {
            format: Format::Svg,
            size: None
        })
    );
}

#[test]
fn anything_else_is_no_image() {
    assert_eq!(sniff(b""), None);
    assert_eq!(sniff(b"<!doctype html><html>"), None);
    assert_eq!(sniff(b"\x89PNG\r\n\x1a\n"), None);
    assert_eq!(sniff(&[0xFF, 0xD8, 0xFF]), None);
}

#[test]
fn declared_types_match_their_aliases() {
    assert!(Format::Jpeg.matches("image/jpg"));
    assert!(Format::Jpeg.matches(" IMAGE/JPEG "));
    assert!(Format::Ico.matches("image/vnd.microsoft.icon"));
    assert!(Format::Ico.matches("image/x-icon"));
    assert!(Format::Svg.matches("image/svg+xml"));
    assert!(!Format::Png.matches("image/webp"));
}

#[test]
fn sizes_parse_and_match() {
    assert_eq!(
        parse_sizes("192x192 512X512 bogus 0x0"),
        Sizes {
            any: false,
            listed: vec![(192, 192), (512, 512)]
        }
    );
    assert!(parse_sizes("any").any);
    let image = Image::sized(Format::Png, 512, 512);
    assert!(image.fits(&parse_sizes("192x192 512x512")));
    assert!(!image.fits(&parse_sizes("192x192")));
    let svg = Image {
        format: Format::Svg,
        size: None,
    };
    assert!(svg.fits(&parse_sizes("any")));
    assert!(svg.fits(&parse_sizes("512x512")));
}
