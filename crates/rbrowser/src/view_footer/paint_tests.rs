use std::cell::Cell;

use super::*;

fn footer(background: u32, foreground: u32) -> CellFooter {
    CellFooter {
        background,
        foreground,
        ..CellFooter::default()
    }
}

/// A strip as CEF keeps one: a ground that the theme takes back, a text colour that stays.
#[derive(Default)]
struct FakeStrip {
    ground: Cell<Option<u32>>,
    text: Cell<Option<u32>>,
}

impl FakeStrip {
    /// What CEF does when the strip comes into a window or the window's theme changes, before
    /// it tells the delegate.
    fn apply_theme(&self) {
        self.ground.set(None);
    }
}

impl Paintable for FakeStrip {
    fn set_ground(&self, argb: u32) {
        self.ground.set(Some(argb));
    }

    fn set_text_color(&self, argb: u32) {
        self.text.set(Some(argb));
    }
}

#[test]
fn the_colours_are_opaque_and_follow_the_footer_in_every_clone() {
    let colors = StripColors::of(&footer(0x0022_c55e, 0x0000_0000));
    assert_eq!(colors.background(), 0xff22_c55e);
    assert_eq!(colors.text(), 0xff00_0000);
    // The delegate holds a clone: a footer written later reaches it.
    let delegate = colors.clone();
    colors.set(&footer(0x001e_3a8a, 0x00ff_ffff));
    assert_eq!(delegate.background(), 0xff1e_3a8a);
    assert_eq!(delegate.text(), 0xffff_ffff);
}

#[test]
fn a_strip_that_comes_into_a_window_is_painted_again() {
    // The owner's finding of 2026-11-14: built and painted, then attached - no ground.
    let strip = FakeStrip::default();
    let colors = StripColors::default();
    written(&strip, &colors, &footer(0x0022_c55e, 0x0000_0000));
    assert_eq!(strip.ground.get(), Some(0xff22_c55e));
    strip.apply_theme();
    assert_eq!(strip.ground.get(), None, "the theme took the ground");
    theme_changed(&strip, &colors);
    assert_eq!(strip.ground.get(), Some(0xff22_c55e));
    assert_eq!(strip.text.get(), Some(0xff00_0000));
}

#[test]
fn a_theme_change_paints_the_colours_written_last() {
    let strip = FakeStrip::default();
    let colors = StripColors::default();
    written(&strip, &colors, &footer(0x0022_c55e, 0x0000_0000));
    // A colour saved in the settings, then the window's theme switches.
    written(&strip, &colors, &footer(0x0014_532d, 0x00ff_ffff));
    strip.apply_theme();
    // The delegate holds its own clone of the colours.
    theme_changed(&strip, &colors.clone());
    assert_eq!(strip.ground.get(), Some(0xff14_532d));
    assert_eq!(strip.text.get(), Some(0xffff_ffff));
}
