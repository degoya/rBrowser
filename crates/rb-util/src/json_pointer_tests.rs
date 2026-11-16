use super::escape;

#[test]
fn tilde_then_slash_so_the_segment_reads_back() {
    assert_eq!(escape("plain"), "plain");
    assert_eq!(escape("a/b"), "a~1b");
    assert_eq!(escape("a~b"), "a~0b");
    assert_eq!(escape("~/"), "~0~1");
    assert_eq!(escape("~1"), "~01");
    assert_eq!(escape(""), "");
}

#[test]
fn a_pointer_of_escaped_segments_resolves_in_a_document() {
    let pointer = format!("/{}/{}", escape("a/b"), escape("c~d"));
    assert_eq!(pointer, "/a~1b/c~0d");
}
