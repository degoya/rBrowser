use super::*;

/// A PNG of `width` x `height` RGB pixels, `paint(x, y)` each, as CDP answers it (base64).
pub(crate) fn png_base64(width: u32, height: u32, paint: impl Fn(u32, u32) -> [u8; 3]) -> String {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, width, height);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().expect("header");
        let data: Vec<u8> = (0..height)
            .flat_map(|y| (0..width).map(move |x| (x, y)))
            .flat_map(|(x, y)| paint(x, y))
            .collect();
        writer.write_image_data(&data).expect("data");
    }
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// A PNG that is only its header: `width` x `height` RGB, interlaced or not, with an empty
/// image - what a decoder reads before any pixel.
pub(crate) fn png_header_base64(width: u32, height: u32, interlaced: bool) -> String {
    fn crc32(bytes: &[u8]) -> u32 {
        let mut crc = 0xffff_ffff_u32;
        for byte in bytes {
            crc ^= u32::from(*byte);
            for _ in 0..8 {
                crc = if crc & 1 == 1 {
                    (crc >> 1) ^ 0xedb8_8320
                } else {
                    crc >> 1
                };
            }
        }
        !crc
    }
    fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
        out.extend_from_slice(&u32::try_from(data.len()).expect("small").to_be_bytes());
        let mut body = kind.to_vec();
        body.extend_from_slice(data);
        out.extend_from_slice(&body);
        out.extend_from_slice(&crc32(&body).to_be_bytes());
    }
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut header = Vec::new();
    header.extend_from_slice(&width.to_be_bytes());
    header.extend_from_slice(&height.to_be_bytes());
    header.extend_from_slice(&[8, 2, 0, 0, u8::from(interlaced)]);
    chunk(&mut out, b"IHDR", &header);
    chunk(
        &mut out,
        b"IDAT",
        &[0x78, 0x9c, 0x03, 0x00, 0x00, 0x00, 0x00, 0x01],
    );
    chunk(&mut out, b"IEND", &[]);
    base64::engine::general_purpose::STANDARD.encode(out)
}

const BLUE: [u8; 3] = [0x1e, 0x88, 0xe5];
const WHITE: [u8; 3] = [255, 255, 255];

#[test]
fn a_viewport_point_is_the_image_pixel_in_the_middle_of_its_css_pixel() {
    assert_eq!(image_point(200.0, 0.0, 1.0), 200);
    assert_eq!(image_point(200.7, 0.0, 1.0), 200);
    // DPR 3: the CSS pixel 200 covers the image pixels 600 to 602.
    assert_eq!(image_point(200.0, 0.0, 3.0), 601);
    // A view shown at half its size: two CSS px to one image pixel.
    assert_eq!(image_point(200.0, 0.0, 0.5), 100);
    assert_eq!(image_point(201.0, 0.0, 0.5), 100);
}

#[test]
fn a_clip_of_the_document_maps_by_its_origin() {
    // The fallback: a clip of the document, the point in document coordinates.
    assert_eq!(image_point(200.0, 195.0, 1.0), 5);
    assert_eq!(image_point(200.0, 198.0, 3.0), 7);
    // DPR 1, loupe 11: 5 CSS px on each side; DPR 3: 2.
    assert_eq!(
        clip_around(100.0, 50.0, 1.0, 11),
        Clip {
            x: 95.0,
            y: 45.0,
            width: 11.0,
            height: 11.0
        }
    );
    assert_eq!(clip_around(100.7, 50.2, 3.0, 11).width, 5.0);
    // Never left of or above the document; a nonsense DPR is 1.
    assert_eq!(clip_around(2.0, 0.0, 1.0, 11).x, 0.0);
    assert_eq!(clip_around(10.0, 10.0, 0.0, 3).width, 3.0);
    // The pipette's tile: TILE_HALF device pixels on each side, inside MAX_TILE_BYTES.
    for dpr in [0.25, 0.5, 1.0, 1.25, 1.5, 2.0, 2.625, 3.0, 3.9, 4.0, 8.0] {
        let clip = tile_around(500.0, 400.0, dpr);
        assert_eq!(clip.x + (clip.width - 1.0) / 2.0, 500.0, "dpr {dpr}");
        let side = (clip.width * dpr).ceil() as usize;
        assert!(side * side * 3 <= MAX_TILE_BYTES, "dpr {dpr}: {side}");
    }
    // A clip's sample names the document's device pixel.
    let image = decode(&png_base64(33, 33, |_, _| BLUE)).expect("decodes");
    let sample = sample(&image, (98.0, 48.0), 3.0, (100.0, 50.0), 11).expect("inside");
    assert_eq!(sample.device, (301, 151));
}

#[test]
fn a_region_is_cut_at_the_image_edges() {
    assert_eq!(
        Region::around(100, 50, 5, 800, 600),
        Region {
            left: 95,
            top: 45,
            width: 11,
            height: 11
        }
    );
    assert_eq!(
        Region::around(2, 598, 5, 800, 600),
        Region {
            left: 0,
            top: 593,
            width: 8,
            height: 7
        }
    );
    // A point outside the image keeps nothing.
    assert_eq!(Region::around(900, 50, 5, 800, 600).width, 0);
    assert_eq!(
        Region {
            left: 790,
            top: 0,
            width: 50,
            height: 5
        }
        .within(800, 600),
        Region {
            left: 790,
            top: 0,
            width: 10,
            height: 5
        }
    );
}

#[test]
fn only_the_region_is_kept() {
    let shot = png_base64(40, 30, |x, y| {
        [
            u8::try_from(x).expect("small"),
            u8::try_from(y).expect("small"),
            7,
        ]
    });
    let image = decode_region(&shot, |width, height| {
        Region::around(20, 10, 2, width, height)
    })
    .expect("decodes");
    assert_eq!((image.width, image.height), (40, 30));
    assert_eq!(image.rgb().len(), 5 * 5 * 3);
    assert_eq!(
        image.pixel(18, 8).map(Rgba::hex).as_deref(),
        Some("#120807")
    );
    assert_eq!(
        image.pixel(22, 12).map(Rgba::hex).as_deref(),
        Some("#160c07")
    );
    // Outside the region nothing is known.
    assert_eq!(image.pixel(17, 8), None);
    assert_eq!(image.pixel(20, 13), None);
}

#[test]
fn a_scan_sees_every_row_besides_the_region() {
    let shot = png_base64(40, 30, |x, y| {
        [
            u8::try_from(x).expect("small"),
            u8::try_from(y).expect("small"),
            9,
        ]
    });
    let mut rows = Vec::new();
    let mut visit = |y: u32, row: &[Rgb]| rows.push((y, row.len(), row[39]));
    let image = decode_scan(
        &shot,
        |width, height| Region::around(5, 5, 1, width, height),
        Some(&mut visit),
    )
    .expect("decodes");
    assert_eq!(image.rgb().len(), 3 * 3 * 3);
    // Every row, also those below the region, whole.
    assert_eq!(rows.len(), 30);
    assert_eq!(rows[29], (29, 40, [39, 29, 9]));
}

#[test]
fn a_mapping_with_an_offset_reads_the_viewport_inside_a_larger_image() {
    // RB-080-77: the window image of a view shows its viewport from the image pixel (3, 2) at
    // 1.5 image px per CSS px; the CSS pixel (10, 4) is the image pixel (3 + 15, 2 + 6).
    let image = decode(&png_base64(60, 40, |x, y| {
        if (x, y) == (18, 8) { BLUE } else { WHITE }
    }))
    .expect("decodes");
    let mapping = Mapping {
        origin: (0.0, 0.0),
        offset: (3, 2),
        scale: 1.5,
    };
    assert_eq!(mapping.point((10.0, 4.0)), (18, 8));
    let sample = sample_mapped(&image, &mapping, (10.0, 4.0), 5).expect("inside");
    assert_eq!(sample.color.hex(), "#1e88e5");
    // The device pixel counts from the viewport's corner, not the image's.
    assert_eq!(sample.device, (15, 6));
    assert_eq!(sample.grid[12].map(Rgba::hex).as_deref(), Some("#1e88e5"));
}

#[test]
fn a_flat_area_answers_its_exact_colour_at_dpr_1_and_3() {
    for dpr in [1.0_f64, 3.0] {
        // The view's image: a viewport of 400 x 300 CSS px at the DPR, blue left of x = 201.
        let device = |css: f64| (css * dpr).round() as u32;
        let edge = device(201.0);
        let shot = png_base64(device(400.0), device(300.0), |x, _| {
            if x < edge { BLUE } else { WHITE }
        });
        let image = decode(&shot).expect("decodes");
        let sample = sample(&image, (0.0, 0.0), dpr, (200.0, 120.0), 11).expect("inside");
        assert_eq!(sample.color.hex(), "#1e88e5", "dpr {dpr}");
        assert!((sample.scale - dpr).abs() < 1e-9);
        // The image pixel in the middle of CSS pixel (200, 120).
        let middle = (dpr * 200.5).floor() as u32;
        assert_eq!(sample.device, (middle, (dpr * 120.5).floor() as u32));
        // The loupe: 11 x 11 image pixels, the centre is the sample.
        assert_eq!(sample.grid.len(), 121);
        assert_eq!(sample.grid[60].map(Rgba::hex).as_deref(), Some("#1e88e5"));
        // One CSS pixel to the right is white.
        let right = super::sample(&image, (0.0, 0.0), dpr, (201.0, 120.0), 11).expect("inside");
        assert_eq!(right.color.hex(), "#ffffff", "dpr {dpr}");
    }
}

#[test]
fn loupe_cells_outside_the_image_are_empty() {
    let image = decode(&png_base64(5, 5, |_, _| BLUE)).expect("decodes");
    let sample = sample(&image, (0.0, 0.0), 1.0, (0.0, 0.0), 5).expect("inside");
    assert_eq!(sample.grid.len(), 25);
    assert_eq!(sample.grid[0], None);
    assert_eq!(sample.grid[12].map(Rgba::hex).as_deref(), Some("#1e88e5"));
    // A point outside the image has no sample.
    assert!(super::sample(&image, (0.0, 0.0), 1.0, (40.0, 0.0), 5).is_none());
}

#[test]
fn what_is_no_png_is_refused() {
    assert!(decode("not base64!").is_err());
    let text = base64::engine::general_purpose::STANDARD.encode(b"GIF89a");
    assert!(decode(&text).is_err());
    // More pixels than an image of a view may have: refused before any is decoded.
    let huge = decode(&png_header_base64(4097, 2048, false)).expect_err("too large");
    assert_eq!(
        huge.params.get("reason").map(String::as_str),
        Some("screenshot_size")
    );
    // An interlaced PNG (Chromium writes none) is not read row by row.
    assert!(decode(&png_header_base64(4, 4, true)).is_err());
}

#[test]
fn grey_and_alpha_pngs_read_as_rgb() {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, 1, 1);
        encoder.set_color(png::ColorType::GrayscaleAlpha);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().expect("header");
        writer.write_image_data(&[0x76, 0xff]).expect("data");
    }
    let image = decode(&base64::engine::general_purpose::STANDARD.encode(bytes)).expect("png");
    assert_eq!(image.pixel(0, 0).map(Rgba::hex).as_deref(), Some("#767676"));
}

#[test]
fn the_tile_stays_under_a_page_message() {
    // 64 KiB of JSON text: the RGB bytes as base64 plus the tile's numbers.
    let side = 2 * TILE_HALF as usize + 1;
    assert!(side * side * 3 <= MAX_TILE_BYTES);
    assert!(MAX_TILE_BYTES.div_ceil(3) * 4 + 256 <= 64 * 1024);
    // Wide enough for the largest loupe with room to move.
    assert!(side >= 2 * 31);
}
