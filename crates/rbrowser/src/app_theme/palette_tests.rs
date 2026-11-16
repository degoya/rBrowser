use super::*;
use crate::view_footer::contrast;

/// WCAG AA for text.
const AA: f64 = 4.5;

fn both() -> [(Resolved, NativePalette); 2] {
    [(Resolved::Light, LIGHT), (Resolved::Dark, DARK)]
}

#[test]
fn system_follows_windows_and_a_choice_stays_put() {
    assert_eq!(resolve(Theme::System, false), Resolved::Light);
    assert_eq!(resolve(Theme::System, true), Resolved::Dark);
    assert_eq!(resolve(Theme::Light, true), Resolved::Light);
    assert_eq!(resolve(Theme::Dark, false), Resolved::Dark);
    assert!(Resolved::Dark.is_dark());
    assert!(!Resolved::Light.is_dark());
    assert_eq!(palette(Resolved::Light), LIGHT);
    assert_eq!(palette(Resolved::Dark), DARK);
}

#[test]
fn the_dark_ground_is_dark_and_the_light_ground_light() {
    // Black and white: whichever the ground is nearer to.
    assert!(contrast(LIGHT.ground, 0x00_0000) > contrast(LIGHT.ground, 0xff_ffff));
    assert!(contrast(DARK.ground, 0xff_ffff) > contrast(DARK.ground, 0x00_0000));
    assert!(contrast(LIGHT.menu_ground, 0x00_0000) > contrast(LIGHT.menu_ground, 0xff_ffff));
    assert!(contrast(DARK.menu_ground, 0xff_ffff) > contrast(DARK.menu_ground, 0x00_0000));
}

#[test]
fn every_text_of_the_strips_reaches_aa_in_both_themes() {
    for (theme, colors) in both() {
        for (what, text) in [("text", colors.text), ("muted", colors.muted)] {
            let ratio = contrast(text, colors.ground);
            assert!(
                ratio >= AA,
                "{theme:?}: {what} on the ground is {ratio:.2} : 1"
            );
        }
    }
}

#[test]
fn every_text_of_the_menus_reaches_aa_in_both_themes() {
    for (theme, colors) in both() {
        let pairs = [
            ("text", colors.menu_text, colors.menu_ground),
            ("accelerator", colors.muted, colors.menu_ground),
            ("hovered", colors.menu_hover_text, colors.menu_hover_ground),
        ];
        for (what, text, ground) in pairs {
            let ratio = contrast(text, ground);
            assert!(ratio >= AA, "{theme:?}: menu {what} is {ratio:.2} : 1");
        }
    }
}

/// WCAG AA for user interface components (1.4.11).
const UI: f64 = 3.0;

#[test]
fn the_leaders_mark_stands_off_the_strip_and_keeps_its_text_readable() {
    for (theme, colors) in both() {
        // The mark is seen without telling hues apart: 3 : 1 off the plain strip.
        let off = contrast(colors.mark_ground, colors.ground);
        assert!(
            off >= UI,
            "{theme:?}: the mark is {off:.2} : 1 off the strip"
        );
        let text = contrast(colors.mark_text, colors.mark_ground);
        assert!(text >= AA, "{theme:?}: text on the mark is {text:.2} : 1");
    }
}

#[test]
fn the_leaders_label_carries_a_sign_besides_its_colour() {
    assert!(!ACTIVE_SIGN.trim().is_empty());
    assert_eq!(
        marked("Mobile 375", true),
        format!("{ACTIVE_SIGN}Mobile 375")
    );
    assert_eq!(marked("Mobile 375", false), "Mobile 375");
    // The strip's text as it is: marked once, unmarked again, never twice.
    let shown = marked("Mobile 375", true);
    assert_eq!(marked(&shown, true), shown);
    assert_eq!(marked(&shown, false), "Mobile 375");
    assert_eq!(marked("", true), ACTIVE_SIGN);
}

#[test]
fn a_colour_laid_over_a_ground_mixes_by_its_alpha() {
    assert_eq!(composite(0xff12_3456, 0x00ff_ffff), 0x0012_3456);
    assert_eq!(composite(0x0012_3456, 0x00ab_cdef), 0x00ab_cdef);
    assert_eq!(composite(0x8000_0000, 0x00ff_ffff), 0x007f_7f7f);
    assert_eq!(opaque(0x0012_3456), 0xff12_3456);
    assert_eq!(opaque(0xab12_3456), 0xff12_3456);
}
