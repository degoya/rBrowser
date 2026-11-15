use super::*;

/// A PNG header of `width` x `height` (signature and IHDR; enough to sniff).
#[must_use]
pub fn png(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
    bytes.extend_from_slice(&width.to_be_bytes());
    bytes.extend_from_slice(&height.to_be_bytes());
    bytes.extend_from_slice(&[8, 6, 0, 0, 0]);
    bytes
}

/// A JPEG header of `width` x `height`: SOI, an APP0 segment of 4 bytes, then SOF0.
#[must_use]
pub fn jpeg(width: u16, height: u16) -> Vec<u8> {
    let mut bytes = vec![
        0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x04, 0x00, 0x00, 0xFF, 0xC0, 0x00, 0x11, 0x08,
    ];
    bytes.extend_from_slice(&height.to_be_bytes());
    bytes.extend_from_slice(&width.to_be_bytes());
    bytes
}

#[test]
fn png_gif_jpeg_and_webp_give_their_size() {
    assert_eq!(
        sniff(&png(1200, 630)),
        Some(Image::sized(Format::Png, 1200, 630))
    );
    assert_eq!(
        sniff(&jpeg(512, 256)),
        Some(Image::sized(Format::Jpeg, 512, 256))
    );
    let gif = b"GIF89a\x40\x00\x20\x00".to_vec();
    assert_eq!(sniff(&gif), Some(Image::sized(Format::Gif, 64, 32)));
    let mut vp8x = b"RIFF\0\0\0\0WEBPVP8X".to_vec();
    vp8x.extend_from_slice(&[10, 0, 0, 0, 0, 0, 0, 0, 191, 0, 0, 191, 0, 0]);
    assert_eq!(sniff(&vp8x), Some(Image::sized(Format::Webp, 192, 192)));
}

#[test]
fn svg_has_no_size_and_html_is_no_image() {
    let svg = sniff(b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>").expect("svg");
    assert_eq!((svg.format, svg.size), (Format::Svg, None));
    assert_eq!(sniff(b"<!doctype html><html></html>"), None);
    assert_eq!(sniff(&[0xFF, 0xD8, 0x00]), None);
}
