use tiny_skia::{Color, Pixmap};

use super::*;

fn white(width: u32, height: u32) -> Pixmap {
    let mut pixmap = Pixmap::new(width, height).expect("pixmap");
    pixmap.fill(Color::WHITE);
    pixmap
}

fn inked(pixmap: &Pixmap) -> Vec<(u32, u32)> {
    let mut points = Vec::new();
    for (i, pixel) in pixmap.data().chunks_exact(4).enumerate() {
        if pixel[0] < 128 {
            let i = i as u32;
            points.push((i % pixmap.width(), i / pixmap.width()));
        }
    }
    points
}

#[test]
fn the_embedded_font_has_the_metrics_the_panel_uses() {
    assert!(metrics_match());
    assert!(FONT.len() < 64 * 1024, "{}", FONT.len());
}

#[test]
fn text_is_drawn_inside_its_box_in_its_colour() {
    let mut pixmap = white(200, 60);
    draw(&mut pixmap, 10.0, 10.0, "Bug #12", 20.0, Rgb(0, 0, 0));
    let ink = inked(&pixmap);
    assert!(ink.len() > 60, "{}", ink.len());
    let right = 10.0 + width("Bug #12", 20.0);
    let bottom = 10.0 + line_height(20.0);
    for (x, y) in ink {
        assert!(x >= 10 && (x as f32) <= right, "{x}");
        assert!(y >= 10 && (y as f32) <= bottom, "{y}");
    }
}

#[test]
fn text_beyond_the_image_is_cut_not_a_failure() {
    let mut pixmap = white(20, 20);
    draw(
        &mut pixmap,
        -30.0,
        15.0,
        "outside and over",
        30.0,
        Rgb(0, 0, 0),
    );
    draw(&mut pixmap, 0.0, 0.0, "\u{4e00}", 12.0, Rgb(0, 0, 0));
}

#[test]
fn widths_follow_the_monospaced_advance() {
    assert_eq!(width("abcd", 10.0), 24.0);
    assert_eq!(width("", 10.0), 0.0);
    assert!((line_height(10.0) - 13.62).abs() < 0.001);
}

/// What `panel/annotation-font.css` holds: [`FONT`] as a data URL.
fn font_css() -> String {
    use base64::Engine;
    let data = base64::engine::general_purpose::STANDARD.encode(FONT);
    format!(
        "/* font/annotation-font.ttf as a data URL, written by src/text_tests.rs\n   \
         (RB_UPDATE_SCHEMA=1 cargo test --manifest-path plugins/screenshot-editor/Cargo.toml font).\n   \
         The panel's frame is sandboxed (origin null) and a font file is fetched in CORS mode,\n   \
         which the plugin scheme does not answer; a data URL needs no request (RB-080-63). */\n\
         @font-face {{\n  font-family: 'ShotAnnotation';\n  \
         src: url('data:font/ttf;base64,{data}') format('truetype');\n  \
         font-weight: normal;\n  font-style: normal;\n}}\n"
    )
}

// RB-080-63: the owner's log showed `Access to font at '.../annotation-font.ttf' from origin
// 'null' has been blocked by CORS policy`. The panel names no font file; the face is the
// component's font, byte for byte.
#[test]
fn the_panel_embeds_the_annotation_font_as_a_data_url() {
    let panel = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("panel");
    let path = panel.join("annotation-font.css");
    if std::env::var_os("RB_UPDATE_SCHEMA").is_some() {
        std::fs::write(&path, font_css()).expect("write the font CSS");
    }
    let css = std::fs::read_to_string(&path).unwrap_or_default();
    assert!(
        css == font_css(),
        "panel/annotation-font.css is not the font; RB_UPDATE_SCHEMA=1 writes it"
    );
    for name in ["dist/panel.css", "index.html"] {
        let text = std::fs::read_to_string(panel.join(name)).expect(name);
        assert!(!text.contains(".ttf"), "{name} names a font file");
    }
    let html = std::fs::read_to_string(panel.join("index.html")).expect("index.html");
    assert!(html.contains("href=\"annotation-font.css\""));
}
