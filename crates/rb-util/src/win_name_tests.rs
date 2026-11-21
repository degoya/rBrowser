use super::*;

#[test]
fn a_segment_folds_as_windows_reads_it() {
    assert_eq!(folded(".GIT"), ".git");
    assert_eq!(folded(".rbrowser."), ".rbrowser");
    assert_eq!(folded(".Rbrowser. . "), ".rbrowser");
    assert_eq!(folded("..."), "");
    assert_eq!(folded("data.json"), "data.json");
}

#[test]
fn a_tilde_and_a_digit_may_be_a_short_name() {
    for short in ["GIT~1", "RBROWS~1", "config~12.jso", "a~1~b"] {
        assert!(is_short_name(short), "{short}");
    }
    for long in ["~", "a~b", "file~", "plain", ".git"] {
        assert!(!is_short_name(long), "{long}");
    }
}
