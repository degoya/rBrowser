use super::*;

/// A PNG's signature and IHDR chunk: all the sniffing reads.
pub fn png(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
    bytes.extend_from_slice(&width.to_be_bytes());
    bytes.extend_from_slice(&height.to_be_bytes());
    bytes.extend_from_slice(&[8, 6, 0, 0, 0, 0, 0, 0, 0]);
    bytes
}

fn jpeg(width: u16, height: u16) -> Vec<u8> {
    let mut bytes = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x04, 0x4A, 0x46];
    bytes.extend_from_slice(&[0xFF, 0xC0, 0x00, 0x11, 0x08]);
    bytes.extend_from_slice(&height.to_be_bytes());
    bytes.extend_from_slice(&width.to_be_bytes());
    bytes.extend_from_slice(&[3, 1, 0x22, 0, 2, 0x11, 1, 3, 0x11, 1]);
    bytes
}

fn webp(chunk: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut bytes = b"RIFF\0\0\0\0WEBP".to_vec();
    bytes.extend_from_slice(chunk);
    bytes.extend_from_slice(&[0, 0, 0, 0]);
    bytes.extend_from_slice(body);
    bytes
}

#[test]
fn a_png_tells_its_size() {
    let info = sniff(&png(393, 852), Kind::Png).expect("a PNG");
    assert_eq!(
        info,
        ImageInfo {
            kind: Kind::Png,
            width: 393,
            height: 852
        }
    );
    assert_eq!(info.kind.mime(), "image/png");
}

#[test]
fn a_jpeg_tells_its_size_from_its_frame_header() {
    let info = sniff(&jpeg(1440, 900), Kind::Jpeg).expect("a JPEG");
    assert_eq!((info.width, info.height), (1440, 900));
}

#[test]
fn the_three_webp_forms_tell_their_size() {
    // VP8: the key frame's start code, then 14-bit width and height.
    let mut lossy = vec![0, 0, 0, 0x9D, 0x01, 0x2A];
    lossy.extend_from_slice(&768u16.to_le_bytes());
    lossy.extend_from_slice(&1024u16.to_le_bytes());
    let info = sniff(&webp(b"VP8 ", &lossy), Kind::Webp).expect("VP8");
    assert_eq!((info.width, info.height), (768, 1024));

    // VP8L: the signature 0x2f, then width - 1 and height - 1 in 14 bits each.
    let bits: u32 = (768 - 1) | ((1024 - 1) << 14);
    let mut lossless = vec![0x2F];
    lossless.extend_from_slice(&bits.to_le_bytes());
    let info = sniff(&webp(b"VP8L", &lossless), Kind::Webp).expect("VP8L");
    assert_eq!((info.width, info.height), (768, 1024));

    // VP8X: flags, three reserved bytes, then width - 1 and height - 1 in 24 bits each.
    let mut extended = vec![0, 0, 0, 0];
    extended.extend_from_slice(&(1440u32 - 1).to_le_bytes()[..3]);
    extended.extend_from_slice(&(900u32 - 1).to_le_bytes()[..3]);
    let info = sniff(&webp(b"VP8X", &extended), Kind::Webp).expect("VP8X");
    assert_eq!((info.width, info.height), (1440, 900));
}

#[test]
fn content_that_is_not_the_named_type_is_refused() {
    assert_eq!(sniff(&png(10, 10), Kind::Jpeg), Err(Refusal::BadType));
    assert_eq!(sniff(&jpeg(10, 10), Kind::Png), Err(Refusal::BadType));
    assert_eq!(sniff(b"<svg xmlns=", Kind::Png), Err(Refusal::BadType));
    assert_eq!(sniff(b"", Kind::Webp), Err(Refusal::BadType));
    assert_eq!(sniff(&png(10, 10)[..20], Kind::Png), Err(Refusal::BadType));
}

#[test]
fn sizes_beyond_the_limits_are_refused() {
    assert_eq!(sniff(&png(0, 10), Kind::Png), Err(Refusal::BadType));
    assert_eq!(
        sniff(&png(MAX_SIDE + 1, 10), Kind::Png),
        Err(Refusal::TooManyPixels)
    );
    assert_eq!(
        sniff(&png(MAX_SIDE, MAX_SIDE), Kind::Png),
        Err(Refusal::TooManyPixels)
    );
    // 16 Mpx (Charge 8 review M3): 64 MiB as RGBA in every view that shows it.
    assert!(sniff(&png(4096, 4096), Kind::Png).is_ok());
    assert!(sniff(&png(2880, 5_000), Kind::Png).is_ok());
    assert_eq!(
        sniff(&png(4096, 4097), Kind::Png),
        Err(Refusal::TooManyPixels)
    );
    assert_eq!(
        sniff(&png(2880, 11_000), Kind::Png),
        Err(Refusal::TooManyPixels)
    );
    let mut big = png(10, 10);
    big.resize(MAX_IMAGE_BYTES + 1, 0);
    assert_eq!(sniff(&big, Kind::Png), Err(Refusal::TooLarge));
}

#[test]
fn the_kind_comes_from_the_extension_in_any_case() {
    assert_eq!(Kind::of("mobile.png"), Some(Kind::Png));
    assert_eq!(Kind::of("a/Desktop.JPG"), Some(Kind::Jpeg));
    assert_eq!(Kind::of("x.jpeg"), Some(Kind::Jpeg));
    assert_eq!(Kind::of("x.webp"), Some(Kind::Webp));
    assert_eq!(Kind::of("x.svg"), None);
    assert_eq!(Kind::of("x.gif"), None);
    assert_eq!(Kind::of("png"), None);
}

#[test]
fn a_name_tells_its_export_scale_and_its_width() {
    assert_eq!(name_hints("home-1440@2x.png"), (Some(1440), 2));
    assert_eq!(name_hints("designs/mobile@3x.webp"), (None, 3));
    assert_eq!(name_hints("tablet-768.jpg"), (Some(768), 1));
    assert_eq!(name_hints("mobile.png"), (None, 1));
    // No scale beyond 4, no width of one digit or past the side limit.
    assert_eq!(name_hints("a@9x.png"), (None, 1));
    assert_eq!(name_hints("a-7.png"), (None, 1));
    assert_eq!(name_hints("a-99999.png"), (None, 1));
    assert_eq!(name_hints("v2-0393.png"), (Some(393), 1));
}

#[test]
fn the_fingerprint_is_stable_and_changes_with_the_content() {
    assert_eq!(fingerprint(b""), "cbf29ce484222325");
    assert_eq!(fingerprint(b"a"), "af63dc4c8601ec8c");
    assert_ne!(fingerprint(b"ab"), fingerprint(b"ba"));
}

#[test]
fn base64_is_the_standard_alphabet_with_padding() {
    assert_eq!(base64(b""), "");
    assert_eq!(base64(b"f"), "Zg==");
    assert_eq!(base64(b"fo"), "Zm8=");
    assert_eq!(base64(b"foo"), "Zm9v");
    assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    assert_eq!(base64(&[0xFF, 0xFE, 0x00]), "//4A");
}

#[test]
fn base64_reads_back_what_it_wrote_and_refuses_anything_else() {
    for length in 0..10 {
        let bytes: Vec<u8> = (0..length).map(|index| (index * 37 + 11) as u8).collect();
        assert_eq!(unbase64(&base64(&bytes)), Some(bytes));
    }
    assert_eq!(unbase64("iVBORw0K"), Some(b"\x89PNG\r\n".to_vec()));
    for bad in ["abc", "ab=c", "a===", "ab\ncd", "ab$d", "====", "abc=d"] {
        assert_eq!(unbase64(bad), None, "{bad}");
    }
}
