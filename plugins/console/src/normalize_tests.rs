use super::*;

#[test]
fn numbers_become_one_mark_each() {
    assert_eq!(
        text("Layout failed at width 375"),
        "Layout failed at width #"
    );
    assert_eq!(
        text("Layout failed at width 1280"),
        "Layout failed at width #"
    );
    assert_eq!(text("took 1.5s for 12 items"), "took #.#s for # items");
    assert_eq!(text("user42 left"), "user# left");
}

#[test]
fn uuids_and_hashes_are_ids() {
    assert_eq!(
        text("request 8f14e45f-ceea-467a-9f0c-0123456789ab failed"),
        "request <id> failed"
    );
    assert_eq!(text("chunk 3fa9c0d1e2 missing"), "chunk <id> missing");
    // Hex letters alone are a word, not an id; a short hex run with a digit is a number.
    assert_eq!(text("deadbeefcafe"), "deadbeefcafe");
    assert_eq!(text("color ff00aa"), "color ff#aa");
}

#[test]
fn different_words_stay_different() {
    assert_ne!(text("Failed at 375"), text("Failed in 375"));
    assert_eq!(text("Failed at 375"), text("Failed at 1280"));
}

#[test]
fn whitespace_collapses_and_punctuation_stays() {
    assert_eq!(text("  a \n\t b:  c  "), "a b: c");
    assert_eq!(text("x(1, 2)"), "x(#, #)");
    assert_eq!(text(""), "");
}

#[test]
fn non_ascii_text_is_kept() {
    assert_eq!(text("caf\u{e9} 12 \u{2192} ok"), "caf\u{e9} # \u{2192} ok");
}

#[test]
fn source_url_drops_query_and_fragment() {
    assert_eq!(
        source_url("https://a.test/app.js?v=12#x"),
        "https://a.test/app.js"
    );
    assert_eq!(
        source_url("https://a.test/app.js#x"),
        "https://a.test/app.js"
    );
    assert_eq!(source_url("https://a.test/app.js"), "https://a.test/app.js");
}

#[test]
fn fnv1a_is_stable() {
    // The reference values of FNV-1a 64.
    assert_eq!(fnv1a(""), 0xcbf2_9ce4_8422_2325);
    assert_eq!(fnv1a("a"), 0xaf63_dc4c_8601_ec8c);
}

#[test]
fn cut_marks_the_cut() {
    assert_eq!(cut("abcdef", 10), "abcdef");
    assert_eq!(cut("abcdefghijk", 8), "abcde...");
}
