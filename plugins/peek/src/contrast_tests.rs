use super::*;
use crate::color::{Rgba, parse};

fn color(text: &str) -> Rgba {
    parse(text).expect("a colour")
}

#[test]
fn large_text_is_24px_or_bold_14pt() {
    assert!(is_large_text(24.0, 400));
    assert!(!is_large_text(23.9, 400));
    assert!(is_large_text(18.67, 700));
    assert!(!is_large_text(18.67, 600));
    assert!(!is_large_text(18.6, 700));
    assert!(is_large_text(32.0, 100));
}

#[test]
fn levels_for_normal_text() {
    assert_eq!(Level::rate(7.0, false), Level::Aaa);
    assert_eq!(Level::rate(6.99, false), Level::Aa);
    assert_eq!(Level::rate(4.5, false), Level::Aa);
    // Not rounded up: 4.48 stays below 4.5 - AA for large text only.
    assert_eq!(Level::rate(4.4999, false), Level::AaLarge);
    assert_eq!(Level::rate(3.0, false), Level::AaLarge);
    assert_eq!(Level::rate(2.99, false), Level::Fail);
}

#[test]
fn levels_for_large_text() {
    assert_eq!(Level::rate(4.5, true), Level::Aaa);
    assert_eq!(Level::rate(4.49, true), Level::Aa);
    assert_eq!(Level::rate(3.0, true), Level::Aa);
    assert_eq!(Level::rate(2.99, true), Level::Fail);
}

#[test]
fn the_fixture_pair_is_aa_large_at_normal_size_and_aa_when_large() {
    let normal = Contrast::of(color("#777"), color("#fff"), 16.0, 400);
    assert_eq!(normal.ratio, 4.48);
    assert_eq!(normal.level, Level::AaLarge);
    assert!(!normal.large_text);
    let large = Contrast::of(color("#777"), color("#fff"), 24.0, 400);
    assert_eq!(large.level, Level::Aa);
    assert!(large.large_text);
}

#[test]
fn a_translucent_foreground_is_blended_over_the_background_first() {
    // Black at half alpha over white is grey 127.5 (unrounded): 3.98:1.
    let contrast = Contrast::of(color("rgba(0, 0, 0, 0.5)"), color("#fff"), 16.0, 400);
    assert_eq!(contrast.foreground, color("rgb(127.5, 127.5, 127.5)"));
    assert_eq!(contrast.ratio, 3.98);
    assert_eq!(contrast.level, Level::AaLarge);
}

#[test]
fn the_json_names_the_level_and_both_colours() {
    let json = Contrast::of(color("#000"), color("#fff"), 16.0, 400).to_json();
    assert_eq!(json["ratio"], 21.0);
    assert_eq!(json["level"], "aaa");
    assert_eq!(json["largeText"], false);
    assert_eq!(json["foreground"], "#000000");
    assert_eq!(json["background"], "#ffffff");
    assert_eq!(Level::AaLarge.key(), "aa_large");
    assert_eq!(Level::Fail.key(), "fail");
    assert_eq!(Level::Aa.key(), "aa");
}
