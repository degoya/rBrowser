use super::*;
use crate::checks::Compare;

fn answer<'a>(headers: &'a [(String, String)], body: &'a str) -> Observed<'a> {
    Observed {
        status: 201,
        headers,
        body: body.as_bytes(),
        total_ms: 120.0,
    }
}

fn headers() -> Vec<(String, String)> {
    vec![
        (
            "content-type".into(),
            "application/JSON; charset=utf-8".into(),
        ),
        ("Set-Cookie".into(), "a=1".into()),
        ("set-cookie".into(), "b=2".into()),
    ]
}

const BODY: &str =
    r#"{"user":{"name":"Ada","id":7,"score":1.0},"tags":["admin","ops"],"meta":{"page":1}}"#;

fn json(path: &str, compare: Compare, value: &str) -> Check {
    Check::Json {
        path: path.into(),
        compare,
        value: value.into(),
    }
}

fn header(name: &str, compare: Compare, value: &str) -> Check {
    Check::Header {
        name: name.into(),
        compare,
        value: value.into(),
    }
}

fn judged(check: Check, body: &str) -> Verdict {
    let headers = headers();
    judge(&[check], &answer(&headers, body)).remove(0)
}

#[test]
fn a_status_matches_its_code_or_its_class() {
    for (expected, passed) in [("201", true), ("2xx", true), ("200", false), ("4xx", false)] {
        let verdict = judged(
            Check::Status {
                expected: expected.into(),
            },
            BODY,
        );
        assert_eq!(verdict.passed, passed, "{expected}");
        assert_eq!(verdict.actual.as_deref(), Some("201"));
        assert_eq!(verdict.check, format!("status {expected}"));
    }
}

#[test]
fn a_json_value_equals_as_json_or_as_text() {
    let cases = [
        (json("$.user.name", Compare::Equals, "\"Ada\""), true),
        (json("$.user.name", Compare::Equals, "Ada"), true),
        (json("$.user.id", Compare::Equals, "7"), true),
        (json("$.user.id", Compare::Equals, "7.0"), true),
        (json("$.user.score", Compare::Equals, "1"), true),
        (json("$.user.id", Compare::Equals, "8"), false),
        (json("$.meta", Compare::Equals, r#"{"page":1}"#), true),
        (
            json("$.tags[*]", Compare::Equals, r#"["admin","ops"]"#),
            true,
        ),
    ];
    for (check, passed) in cases {
        let line = check.to_line();
        assert_eq!(judged(check, BODY).passed, passed, "{line}");
    }
}

#[test]
fn contains_looks_into_a_text_a_list_or_an_object() {
    let cases = [
        (json("$.user.name", Compare::Contains, "Ad"), true),
        (json("$.tags", Compare::Contains, "admin"), true),
        (json("$.tags", Compare::Contains, "\"ops\""), true),
        (json("$.tags", Compare::Contains, "root"), false),
        (json("$.user", Compare::Contains, "name"), true),
        (json("$.user", Compare::Contains, "age"), false),
        (json("$.user.id", Compare::Contains, "7"), false),
        (json("$..name", Compare::Contains, "Ada"), true),
    ];
    for (check, passed) in cases {
        let line = check.to_line();
        assert_eq!(judged(check, BODY).passed, passed, "{line}");
    }
}

#[test]
fn a_json_check_without_its_value_fails_and_says_why() {
    let missing = judged(json("$.user.age", Compare::Equals, "3"), BODY);
    assert!(!missing.passed);
    assert_eq!(missing.problem, Some(Problem::Missing));
    let not_json = judged(json("$.a", Compare::Equals, "1"), "<html>");
    assert!(!not_json.passed);
    assert_eq!(not_json.problem, Some(Problem::NotJson));
    let no_path = judged(json("$[?(@)]", Compare::Equals, "1"), BODY);
    assert!(!no_path.passed);
    assert_eq!(no_path.problem, Some(Problem::BadPath));
}

#[test]
fn a_header_is_found_in_any_case_and_contains_ignores_case() {
    assert!(judged(header("Content-Type", Compare::Contains, "json"), BODY).passed);
    assert!(
        !judged(
            header("Content-Type", Compare::Equals, "application/json"),
            BODY
        )
        .passed
    );
    let cookies = judged(header("set-cookie", Compare::Equals, "a=1, b=2"), BODY);
    assert!(cookies.passed);
    let missing = judged(header("X-Id", Compare::Equals, ""), BODY);
    assert!(!missing.passed);
    assert_eq!(missing.problem, Some(Problem::Missing));
}

#[test]
fn the_time_must_stay_under_its_bound() {
    assert!(judged(Check::Time { below_ms: 121 }, BODY).passed);
    let slow = judged(Check::Time { below_ms: 120 }, BODY);
    assert!(!slow.passed);
    assert_eq!(slow.actual.as_deref(), Some("120 ms"));
}

#[test]
fn a_long_actual_value_is_cut() {
    let long = format!("{{\"a\":\"{}\"}}", "x".repeat(1000));
    let verdict = judged(json("$.a", Compare::Equals, "y"), &long);
    let actual = verdict.actual.expect("an actual value");
    assert!(actual.chars().count() <= MAX_ACTUAL + 1);
    assert!(actual.ends_with('\u{2026}'));
}

#[test]
fn every_check_gets_one_verdict_in_order() {
    let headers = headers();
    let checks = [
        Check::Status {
            expected: "2xx".into(),
        },
        Check::Time { below_ms: 1 },
    ];
    let verdicts = judge(&checks, &answer(&headers, BODY));
    assert_eq!(
        verdicts.iter().map(|v| v.passed).collect::<Vec<_>>(),
        [true, false]
    );
    assert!(!all_passed(&verdicts));
    assert!(all_passed(&verdicts[..1]));
}
