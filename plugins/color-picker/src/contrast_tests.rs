use super::*;
use crate::color::parse;

fn rating(foreground: &str, background: &str) -> Rating {
    Rating::of(
        parse(foreground).expect("foreground"),
        parse(background).expect("background"),
    )
}

#[test]
fn grey_767676_on_white_passes_aa_not_aaa() {
    let rating = rating("#767676", "#ffffff");
    assert_eq!(rating.ratio, 4.54);
    assert!(rating.normal.aa);
    assert!(!rating.normal.aaa);
    assert!(rating.large.aa);
    assert!(rating.large.aaa);
}

#[test]
fn the_level_is_decided_on_the_exact_ratio() {
    // #777 on white is 4.478:1: shown as 4.48, AA only for large text.
    let rating = rating("#777", "#fff");
    assert_eq!(rating.ratio, 4.48);
    assert!(!rating.normal.aa);
    assert!(rating.large.aa);
    assert!(!rating.large.aaa);
}

#[test]
fn swapping_keeps_the_ratio_and_extremes_are_1_and_21() {
    assert_eq!(rating("#fff", "#767676").ratio, 4.54);
    assert_eq!(rating("#000", "#fff").ratio, 21.0);
    let same = rating("#1e88e5", "#1e88e5");
    assert_eq!(same.ratio, 1.0);
    assert!(!same.large.aa);
}

#[test]
fn a_translucent_foreground_is_blended_over_the_background() {
    // Half-transparent black over white is grey 127.5 (#808080): 3.98:1.
    let half = rating("rgba(0, 0, 0, 0.5)", "#fff");
    assert_eq!(half.foreground.hex(), "#808080");
    assert_eq!(half.ratio, 3.98);
    // A translucent background sits on white.
    let over_white = rating("#000", "rgba(0, 0, 0, 0)");
    assert_eq!(over_white.background.hex(), "#ffffff");
}

#[test]
fn the_answer_names_ratio_and_levels() {
    let json = rating("#767676", "#ffffff").to_json();
    assert_eq!(json["ratio"], 4.54);
    assert_eq!(json["normal"]["aa"], true);
    assert_eq!(json["normal"]["aaa"], false);
    assert_eq!(json["large"]["aaa"], true);
    assert_eq!(json["foreground"]["hex"], "#767676");
    assert_eq!(json["background"]["oklch"], "oklch(100% 0 0)");
}

#[test]
fn the_tool_reads_any_format() {
    let answer = tool(&serde_json::json!({ "foreground": "rgb(118, 118, 118)", "background": "hsl(0, 0%, 100%)" }))
        .expect("rating");
    assert_eq!(answer["ratio"], 4.54);
    for args in [
        serde_json::json!({ "foreground": "#767676" }),
        serde_json::json!({ "foreground": "nope", "background": "#fff" }),
    ] {
        let refused = tool(&args).expect_err("refused");
        assert_eq!(refused.code.as_deref(), Some("plugins.error.invalid"));
    }
}
