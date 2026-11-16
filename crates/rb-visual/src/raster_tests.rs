use super::*;

#[test]
fn a_png_round_trips() {
    let mut image = Rgba::new(3, 2, [10, 20, 30, 255]).expect("small");
    image.put(1, 1, [200, 100, 50, 255]);
    let png = encode_png(&image).expect("encoded");
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    let back = decode_png(&png).expect("decoded");
    assert_eq!(back, image);
}

#[test]
fn rgb_and_grey_pngs_become_rgba() {
    for (color, channels) in [(png::ColorType::Rgb, 3), (png::ColorType::Grayscale, 1)] {
        let mut bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut bytes, 2, 1);
            encoder.set_color(color);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().expect("header");
            writer
                .write_image_data(&vec![77; 2 * channels])
                .expect("data");
        }
        let image = decode_png(&bytes).expect("decoded");
        assert_eq!((image.width(), image.height()), (2, 1));
        assert_eq!(image.get(1, 0), Some([77, 77, 77, 255]));
    }
}

#[test]
fn garbage_is_an_error() {
    assert!(matches!(decode_png(b"not a png"), Err(VisualError::Png(_))));
}

#[test]
fn huge_images_are_refused() {
    assert!(matches!(
        Rgba::new(MAX_SIDE + 1, 1, [0; 4]),
        Err(VisualError::TooLarge { .. })
    ));
    assert!(matches!(
        Rgba::new(MAX_SIDE, MAX_SIDE, [0; 4]),
        Err(VisualError::TooLarge { .. })
    ));
}

#[test]
fn fill_and_blit_clip_at_the_edges() {
    let mut canvas = Rgba::new(4, 4, [0, 0, 0, 255]).expect("small");
    canvas.fill(2, 2, 10, 10, [255, 0, 0, 255]);
    assert_eq!(canvas.get(3, 3), Some([255, 0, 0, 255]));
    assert_eq!(canvas.get(1, 1), Some([0, 0, 0, 255]));
    let tile = Rgba::new(3, 3, [0, 255, 0, 255]).expect("small");
    canvas.blit(&tile, 3, 0);
    assert_eq!(canvas.get(3, 0), Some([0, 255, 0, 255]));
    assert_eq!(canvas.get(2, 0), Some([0, 0, 0, 255]));
    assert_eq!(canvas.get(4, 0), None);
}

#[test]
fn transparent_pixels_blend_over_the_canvas() {
    let mut canvas = Rgba::new(1, 1, [255, 255, 255, 255]).expect("small");
    let tile = Rgba::new(1, 1, [0, 0, 0, 0]).expect("small");
    canvas.blit(&tile, 0, 0);
    assert_eq!(canvas.get(0, 0), Some([255, 255, 255, 255]));
}

#[test]
fn the_size_is_read_from_the_header() {
    let image = Rgba::new(7, 5, [1, 2, 3, 255]).expect("small");
    let png = encode_png(&image).expect("encoded");
    assert_eq!(png_size(&png), Some((7, 5)));
    assert_eq!(png_size(b"\x89PNG fake"), None);
}
