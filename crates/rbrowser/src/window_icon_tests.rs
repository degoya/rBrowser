use super::*;

/// The width and height of a PNG, from its IHDR chunk.
fn png_size(png: &[u8]) -> (u32, u32) {
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n", "not a PNG");
    assert_eq!(&png[12..16], b"IHDR");
    let read = |at: usize| u32::from_be_bytes([png[at], png[at + 1], png[at + 2], png[at + 3]]);
    (read(16), read(20))
}

#[test]
fn each_icon_has_the_size_its_scale_asks_for() {
    for (dip, set) in [(SMALL_DIP, &SMALL), (LARGE_DIP, &LARGE)] {
        assert!(!set.is_empty());
        for (scale, png) in set.iter() {
            // An exact size per scale factor: Windows never scales it.
            let pixels = (dip as f32 * scale) as u32;
            assert_eq!(png_size(png), (pixels, pixels), "{dip} DIP at {scale}x");
        }
    }
}
