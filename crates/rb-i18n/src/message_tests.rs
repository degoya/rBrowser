use super::*;

fn render(message: &str, args: &[(&str, &str)]) -> String {
    format(message, args, Style::Plain)
}

#[test]
fn plain_text_is_returned_as_written() {
    assert_eq!(render("Reload", &[]), "Reload");
    assert_eq!(render("Can't open #1", &[]), "Can't open #1");
    assert_eq!(render("", &[]), "");
}

#[test]
fn named_placeholders_are_filled() {
    assert_eq!(
        render(
            "Open {url} in {view}",
            &[("url", "a.test"), ("view", "Phone")]
        ),
        "Open a.test in Phone"
    );
    assert_eq!(render("{a}{a}", &[("a", "x")]), "xx");
}

#[test]
fn a_missing_argument_keeps_its_placeholder_and_extra_ones_are_ignored() {
    assert_eq!(render("Open {url}", &[("other", "x")]), "Open {url}");
    assert_eq!(render("Open { url }", &[]), "Open { url }");
}

#[test]
fn plurals_pick_the_english_category() {
    let message = "{count, plural, one {# view} other {# views}}";
    assert_eq!(render(message, &[("count", "1")]), "1 view");
    assert_eq!(render(message, &[("count", "0")]), "0 views");
    assert_eq!(render(message, &[("count", "3")]), "3 views");
    assert_eq!(render(message, &[("count", "1.0")]), "1.0 views");
    assert_eq!(render(message, &[("count", "many")]), "many views");
}

#[test]
fn an_exact_selector_wins_over_the_category() {
    let message = "{n, plural, =0 {No views} =1 {One view} one {# view} other {# views}}";
    assert_eq!(render(message, &[("n", "0")]), "No views");
    assert_eq!(render(message, &[("n", "1")]), "One view");
    assert_eq!(render(message, &[("n", "2")]), "2 views");
}

#[test]
fn a_plural_branch_holds_placeholders_and_text_around_it_stays() {
    let message = "In {name}: {count, plural, one {# view of {name}} other {# views}}.";
    assert_eq!(
        render(message, &[("count", "1"), ("name", "Grid")]),
        "In Grid: 1 view of Grid."
    );
}

#[test]
fn a_plural_without_its_argument_keeps_its_source() {
    let message = "Have {n, plural, one {# view} other {# views}}";
    assert_eq!(render(message, &[]), message);
}

#[test]
fn a_hash_outside_a_plural_is_text() {
    assert_eq!(render("#{n} #", &[("n", "2")]), "#2 #");
}

#[test]
fn malformed_messages_are_refused() {
    for bad in [
        "{",
        "}",
        "a } b",
        "{url",
        "{}",
        "{1url}",
        "{n, plural}",
        "{n, plural, one {x}}",
        "{n, plural, other {x} other {y}}",
        "{n, plural, lots {x} other {y}}",
        "{n, plural, =x {x} other {y}}",
        "{n, plural, other x}",
        "{n, select, other {x}}",
        "{n, plural, other {x}",
    ] {
        assert!(validate(bad).is_err(), "{bad:?} should be refused");
    }
    for good in [
        "Text",
        "{a} and {b_2}",
        "{ a }",
        "{n,plural,one{# a}other{# b}}",
        "{n, plural, =0 {none} zero {z} one {o} two {t} few {f} many {m} other {#}}",
    ] {
        assert!(
            validate(good).is_ok(),
            "{good:?} should be accepted: {:?}",
            validate(good)
        );
    }
}

#[test]
fn a_malformed_message_is_shown_as_written() {
    assert_eq!(render("{broken", &[("broken", "x")]), "{broken");
}

#[test]
fn the_english_rule_is_one_for_the_integer_one_only() {
    assert_eq!(plural_category("1"), PluralCategory::One);
    assert_eq!(plural_category(" 1 "), PluralCategory::One);
    assert_eq!(plural_category("-1"), PluralCategory::One);
    for other in ["0", "2", "1.0", "1.5", "", "x", "11"] {
        assert_eq!(plural_category(other), PluralCategory::Other, "{other:?}");
    }
}
