use super::{rules, slice, units};

#[test]
fn style_rules_inside_media_are_listed_with_their_ranges() {
    let text = "a { color: red }\n@media (min-width: 1024px) {\n  .wide { display: grid; }\n}\n";
    let found = rules(text);
    let preludes: Vec<&str> = found.iter().map(|rule| rule.prelude.as_str()).collect();
    assert_eq!(preludes, ["a", ".wide"]);
    let wide = &found[1];
    assert_eq!(
        slice(text, wide.start, wide.end),
        ".wide { display: grid; }"
    );
    assert_eq!(slice(text, found[0].start, found[0].body), "a {");
}

#[test]
fn comments_strings_and_other_at_rules_do_not_confuse_the_braces() {
    let text = "/* { not a rule } */ @font-face { font-family: \"X}\"; }\n\
                @import url(a.css);\n.b::after { content: '{'; }\n@keyframes spin { from { opacity: 0 } to { opacity: 1 } }";
    let found = rules(text);
    let preludes: Vec<&str> = found.iter().map(|rule| rule.prelude.as_str()).collect();
    // The keyframes' steps are blocks of their own inside the at-rule, listed before it ends.
    assert_eq!(
        preludes,
        ["@font-face", ".b::after", "from", "to", "@keyframes spin"]
    );
}

#[test]
fn offsets_count_utf16_units_as_cdp_does() {
    // "\u{1F600}" is two UTF-16 units, "\u{e4}" one.
    let text = ".\u{e4}\u{1F600} { x: y }\n.n { }";
    assert_eq!(units(text), 20);
    let found = rules(text);
    assert_eq!(found[1].prelude, ".n");
    assert_eq!(found[1].start, 14);
    assert_eq!(slice(text, found[1].start, found[1].end), ".n { }");
}

#[test]
fn an_unclosed_rule_is_not_listed() {
    assert!(rules("a { color: red").is_empty());
    assert!(rules("").is_empty());
}
