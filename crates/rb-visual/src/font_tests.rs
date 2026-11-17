use super::*;

#[test]
fn a_glyph_is_drawn_where_its_bits_are() {
    let mut image = Rgba::new(12, 16, [0, 0, 0, 0]).expect("small");
    let ink = [255, 255, 255, 255];
    draw_text(&mut image, (1, 1), "I", 2, ink, 12);
    // The top bar of `I` (0x0E): columns 1..=3, so pixels 3..=8 at scale 2.
    assert_eq!(image.get(1, 1), Some([0, 0, 0, 0]));
    assert_eq!(image.get(3, 1), Some(ink));
    assert_eq!(image.get(8, 2), Some(ink));
    assert_eq!(image.get(9, 1), Some([0, 0, 0, 0]));
}

#[test]
fn text_is_cut_off_at_the_width() {
    let mut image = Rgba::new(20, 7, [0, 0, 0, 0]).expect("small");
    draw_text(&mut image, (0, 0), "MMMM", 1, [9, 9, 9, 255], 11);
    // Two glyphs fit into 11 px; the third starts at 12.
    assert!((12..20).all(|x| (0..7).all(|y| image.get(x, y) == Some([0, 0, 0, 0]))));
    assert_eq!(image.get(6, 0), Some([9, 9, 9, 255]));
}

#[test]
fn lower_case_and_unknown_characters_have_glyphs() {
    assert_eq!(glyph('m'), glyph('M'));
    assert_ne!(glyph('x'), glyph('X'));
    assert_eq!(glyph('\u{4e2d}'), glyph('?'));
}
