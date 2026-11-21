use super::*;

#[test]
fn the_three_http_date_forms_are_read() {
    assert_eq!(parse("Thu, 01 Jan 1970 00:00:00 GMT"), Some(0));
    assert_eq!(parse("Fri, 20 Nov 2026 10:00:00 GMT"), Some(1_795_168_800));
    assert_eq!(parse("Friday, 20-Nov-26 10:00:00 GMT"), Some(1_795_168_800));
    assert_eq!(parse("Fri Nov 20 10:00:00 2026"), Some(1_795_168_800));
    assert_eq!(parse("Wed, 29 Feb 2024 23:59:59 GMT"), Some(1_709_251_199));
}

#[test]
fn two_digit_years_follow_rfc_6265() {
    assert_eq!(parse("Thu, 01-Jan-70 00:00:00 GMT"), Some(0));
    assert_eq!(parse("Sat, 01-Jan-00 00:00:00 GMT"), Some(946_684_800));
}

#[test]
fn anything_else_is_no_date() {
    for text in [
        "",
        "now",
        "Thu, 32 Jan 1970 00:00:00 GMT",
        "Thu, 01 Foo 1970 00:00:00 GMT",
        "Thu, 01 Jan 1970 25:00:00 GMT",
        "Thu, 01 Jan 1500 00:00:00 GMT",
        "01 Jan 1970",
    ] {
        assert_eq!(parse(text), None, "{text}");
    }
    // An unknown weekday is no part of the date.
    assert_eq!(parse("\u{e9}\u{1F600}, 01 Jan 1970 00:00:00 GMT"), Some(0));
    let long = "1 ".repeat(10_000);
    assert_eq!(parse(&long), None);
}
