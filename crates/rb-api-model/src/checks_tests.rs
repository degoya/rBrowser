use super::*;

fn json(path: &str, compare: Compare, value: &str) -> Check {
    Check::Json {
        path: path.into(),
        compare,
        value: value.into(),
    }
}

#[test]
fn every_kind_has_one_line_and_reads_back() {
    let checks = [
        (
            Check::Status {
                expected: "2xx".into(),
            },
            "status 2xx",
        ),
        (
            json("$.user.name", Compare::Equals, "\"Ada\""),
            "json $.user.name == \"Ada\"",
        ),
        (
            json("$.tags", Compare::Contains, "admin"),
            "json $.tags contains admin",
        ),
        (
            Check::Header {
                name: "Content-Type".into(),
                compare: Compare::Contains,
                value: "json".into(),
            },
            "header Content-Type contains json",
        ),
        (Check::Time { below_ms: 500 }, "time < 500"),
    ];
    for (check, line) in checks {
        assert_eq!(check.to_line(), line);
        assert_eq!(Check::from_line(line), Ok(check));
    }
}

#[test]
fn a_path_with_spaces_splits_at_its_operator() {
    assert_eq!(
        Check::from_line("json $['full name'] == Ada Lovelace"),
        Ok(json("$['full name']", Compare::Equals, "Ada Lovelace"))
    );
    assert_eq!(
        Check::from_line("header X-Empty =="),
        Ok(Check::Header {
            name: "X-Empty".into(),
            compare: Compare::Equals,
            value: String::new(),
        })
    );
}

#[test]
fn a_line_that_does_not_read_names_why() {
    for (line, reason) in [
        ("", "check kind"),
        ("smell good", "check kind"),
        ("status", "check status"),
        ("json $.a", "check operator"),
        ("json $.a > 3", "check operator"),
        ("header", "check operator"),
        ("time 500", "check time"),
        ("time < soon", "check time"),
    ] {
        assert_eq!(Check::from_line(line), Err(reason), "{line}");
    }
}

#[test]
fn a_check_is_held_to_its_form() {
    let good = [
        Check::Status {
            expected: "201".into(),
        },
        Check::Status {
            expected: "4xx".into(),
        },
        json("$..id", Compare::Equals, "7"),
        Check::Time { below_ms: 1 },
    ];
    for check in &good {
        assert_eq!(check.validate(), Ok(()), "{check:?}");
    }
    let bad = [
        Check::Status {
            expected: "2x".into(),
        },
        Check::Status {
            expected: "600".into(),
        },
        json("user.name", Compare::Equals, "x"),
        json("$.a", Compare::Equals, "two\nlines"),
        Check::Header {
            name: "Bad Name".into(),
            compare: Compare::Equals,
            value: String::new(),
        },
        Check::Time { below_ms: 0 },
        Check::Time {
            below_ms: crate::request::MAX_TIMEOUT_MS + 1,
        },
    ];
    for check in &bad {
        assert!(check.validate().is_err(), "{check:?}");
    }
}

#[test]
fn the_model_is_camel_case_json() {
    let value = serde_json::to_value(json("$.a", Compare::Contains, "x")).expect("serialises");
    assert_eq!(
        value,
        serde_json::json!({ "kind": "json", "path": "$.a", "compare": "contains", "value": "x" })
    );
    let time: Check = serde_json::from_value(serde_json::json!({ "kind": "time", "belowMs": 250 }))
        .expect("reads");
    assert_eq!(time, Check::Time { below_ms: 250 });
}
