use super::*;

#[test]
fn letters_get_accents_and_the_text_grows_by_forty_percent() {
    assert_eq!(pseudo_localize("Reload"), "[R\u{e9}l\u{f3}\u{e1}d~]");
    assert_eq!(pseudo_localize("OK"), "[\u{d3}K]");
    assert_eq!(pseudo_localize(""), "");
}

#[test]
fn every_text_is_at_least_forty_percent_longer() {
    for text in [
        "a",
        "OK",
        "Save",
        "Reload all views",
        "Open the project folder in a new window",
    ] {
        let before = text.chars().count();
        let after = pseudo_localize(text).chars().count();
        assert!(after * 5 >= before * 7, "{text:?}: {before} -> {after}");
    }
}

#[test]
fn digits_and_punctuation_stay() {
    let mut out = String::new();
    accent("3 x 4 = 12!", &mut out);
    assert_eq!(out, "3 x 4 = 12!");
}
