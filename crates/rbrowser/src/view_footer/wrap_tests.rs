use rb_config::FooterWrap;

use super::*;

/// The footer of the owner's 375 px view (RB-080-53), 50 characters.
const MOBILE: &str = "mobile | 0-767px | 375 \u{d7} 812 @3x | max-width 767px";

fn lines(text: &str, available: u32) -> Vec<String> {
    fit(text, Some(available), FooterWrap::Auto).lines
}

#[test]
fn the_width_is_the_characters_the_insets_and_the_slack() {
    assert_eq!(text_width(""), INSETS + SLACK);
    // 10 characters of 6.05 DIP, rounded up.
    assert_eq!(text_width("0123456789"), 61 + INSETS + SLACK);
    // A character counts once, whatever its bytes.
    assert_eq!(text_width("\u{d7}\u{221e}"), text_width("ab"));
    assert_eq!(MOBILE.chars().count(), 50);
    assert_eq!(text_width(MOBILE), 303 + INSETS + SLACK);
}

#[test]
fn a_text_that_fits_stays_on_one_line() {
    let fitted = fit(MOBILE, Some(375), FooterWrap::Auto);
    assert_eq!(fitted.lines, [MOBILE]);
    assert!(!fitted.cut);
    // Exactly as wide as the strip still fits.
    assert_eq!(lines(MOBILE, text_width(MOBILE)), [MOBILE]);
}

#[test]
fn the_cut_footer_of_the_narrow_view_breaks_between_two_fields() {
    // The strip of the screenshot is 145 DIP wide and showed "mobile | 0-767px | 3...".
    assert!(text_width(MOBILE) > 145);
    // At 220 DIP the breaks after the second and the third field fit; the first one leaves the
    // lines more alike.
    let fitted = fit(MOBILE, Some(220), FooterWrap::Auto);
    assert_eq!(
        fitted.lines,
        ["mobile | 0-767px", "375 \u{d7} 812 @3x | max-width 767px"]
    );
    assert!(!fitted.cut, "both lines fit: nothing is cut");
    assert!(fitted.lines.iter().all(|line| text_width(line) <= 220));
}

#[test]
fn the_break_that_fits_leaves_the_lines_most_alike() {
    // Every break fits in 150 DIP; the one in the middle is taken.
    assert_eq!(
        lines("aaaa | bbbb | cccc | dddd", 150),
        ["aaaa | bbbb", "cccc | dddd"]
    );
    // Only the break after the long first field fits.
    assert_eq!(
        lines("aaaaaaaaaaaaaaaaaaaa | b | c", 150),
        ["aaaaaaaaaaaaaaaaaaaa", "b | c"]
    );
}

#[test]
fn what_two_lines_do_not_hold_is_cut_at_the_end() {
    // 145 DIP hold 21 characters: the first line as full as it gets, the second one is cut.
    let fitted = fit(MOBILE, Some(145), FooterWrap::Auto);
    assert_eq!(
        fitted.lines,
        ["mobile | 0-767px", "375 \u{d7} 812 @3x | max-width 767px"]
    );
    assert!(fitted.cut);
    // Not even the first field fits: it stands alone, and is cut too.
    let fitted = fit(
        "a-very-long-breakpoint-name | 0-767px",
        Some(60),
        FooterWrap::Auto,
    );
    assert_eq!(fitted.lines, ["a-very-long-breakpoint-name", "0-767px"]);
    assert!(fitted.cut);
}

#[test]
fn never_more_than_two_lines() {
    for width in [1, 30, 60, 100, 145, 200, 375, 1920] {
        let count = lines(MOBILE, width).len();
        assert!((1..=MAX_LINES).contains(&count), "{width}: {count} lines");
    }
}

#[test]
fn one_field_has_nowhere_to_break() {
    let fitted = fit("0-767px", Some(20), FooterWrap::Auto);
    assert_eq!(fitted.lines, ["0-767px"]);
    assert!(fitted.cut);
}

#[test]
fn never_keeps_one_line_and_says_that_it_is_cut() {
    let fitted = fit(MOBILE, Some(145), FooterWrap::Never);
    assert_eq!(fitted.lines, [MOBILE]);
    assert!(fitted.cut);
    assert!(!fit(MOBILE, Some(375), FooterWrap::Never).cut);
}

#[test]
fn a_strip_without_a_width_yet_takes_one_line() {
    for available in [None, Some(0)] {
        let fitted = fit(MOBILE, available, FooterWrap::Auto);
        assert_eq!(fitted.lines, [MOBILE]);
        assert!(!fitted.cut);
    }
}

#[test]
fn the_lines_together_are_the_text() {
    let fitted = fit(MOBILE, Some(220), FooterWrap::Auto);
    assert_eq!(fitted.lines.join(SEPARATOR), MOBILE);
}

#[test]
fn a_footer_takes_at_most_half_of_its_view() {
    // RB-080-78: on a canvas zoomed far out the strip was higher than the view.
    let line = super::super::FOOTER_HEIGHT;
    assert_eq!(lines_for_height(None), MAX_LINES);
    assert_eq!(lines_for_height(Some(4 * line)), 2);
    assert_eq!(lines_for_height(Some(1000)), 2);
    assert_eq!(lines_for_height(Some(4 * line - 1)), 1);
    assert_eq!(lines_for_height(Some(2 * line)), 1);
    assert_eq!(lines_for_height(Some(2 * line - 1)), 0);
    assert_eq!(lines_for_height(Some(0)), 0);
}

#[test]
fn a_low_view_keeps_one_line_or_none() {
    let line = super::super::FOOTER_HEIGHT;
    // Two lines by the width, one by the height: one line, its end cut.
    let one = fit_room(MOBILE, Some(200), Some(3 * line), FooterWrap::Auto);
    assert_eq!(one.lines, [MOBILE]);
    assert!(one.cut);
    // No room for a line: no line.
    let none = fit_room(MOBILE, Some(375), Some(line), FooterWrap::Auto);
    assert!(none.lines.is_empty());
    // Room enough: as by the width alone.
    assert_eq!(
        fit_room(MOBILE, Some(200), Some(400), FooterWrap::Auto),
        fit(MOBILE, Some(200), FooterWrap::Auto)
    );
}
