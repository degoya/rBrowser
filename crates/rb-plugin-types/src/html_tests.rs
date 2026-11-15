use super::escape;

#[test]
fn each_of_the_five_characters_is_escaped() {
    assert_eq!(escape("&"), "&amp;");
    assert_eq!(escape("<"), "&lt;");
    assert_eq!(escape(">"), "&gt;");
    assert_eq!(escape("\""), "&quot;");
    assert_eq!(escape("'"), "&#39;");
}

#[test]
fn a_script_in_an_attribute_or_a_single_quoted_one_cannot_break_out() {
    assert_eq!(
        escape("<script>alert('x') & \"y\"</script>"),
        "&lt;script&gt;alert(&#39;x&#39;) &amp; &quot;y&quot;&lt;/script&gt;"
    );
    assert_eq!(escape("' onclick='x"), "&#39; onclick=&#39;x");
}

#[test]
fn plain_text_newlines_and_other_characters_stay() {
    assert_eq!(escape("Hallo Welt\n\u{e9}\t"), "Hallo Welt\n\u{e9}\t");
    assert_eq!(escape(""), "");
}
