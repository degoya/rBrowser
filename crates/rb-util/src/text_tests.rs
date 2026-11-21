use super::{cut, cut_ellipsis};

#[test]
fn a_cut_keeps_at_most_max_characters_and_no_marker() {
    assert_eq!(cut("abcdef", 4), "abcd");
    assert_eq!(cut("abc", 4), "abc");
    assert_eq!(cut("", 4), "");
    assert_eq!(cut("abc", 0), "");
    // Characters, not bytes: a multi-byte character is never split.
    assert_eq!(
        cut("\u{3b1}\u{3b2}\u{3b3}\u{3b4}", 3),
        "\u{3b1}\u{3b2}\u{3b3}"
    );
}

#[test]
fn a_cut_with_ellipsis_is_never_longer_than_max() {
    assert_eq!(cut_ellipsis("abcdef", 5), "ab...");
    assert_eq!(cut_ellipsis("abcde", 5), "abcde", "it fits: no marker");
    assert_eq!(cut_ellipsis("abc", 5), "abc");
    assert_eq!(
        cut_ellipsis("\u{3b1}\u{3b2}\u{3b3}\u{3b4}\u{3b5}x", 5),
        "\u{3b1}\u{3b2}..."
    );
    for max in 0..8 {
        let out = cut_ellipsis("a long text, cut", max);
        assert!(out.chars().count() <= max, "{max}: {out}");
    }
    // Below the marker's length the text is cut without one.
    assert_eq!(cut_ellipsis("abcdef", 2), "ab");
}
