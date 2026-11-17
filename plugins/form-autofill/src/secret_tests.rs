use super::*;
use serde_json::Value;

/// The cases the page script's twin is held to as well (`web/src/capture.test.ts`).
const CASES: &str = include_str!("../tests/fixtures/sensitive-fields.json");

fn text(case: &Value, key: &str) -> String {
    case[key].as_str().unwrap_or_default().to_owned()
}

#[test]
fn the_shared_cases_hold() {
    let cases: Vec<Value> = serde_json::from_str(CASES).expect("the cases");
    assert!(cases.len() > 20);
    for case in &cases {
        let field = Field {
            control: text(case, "type"),
            name: text(case, "name"),
            id: text(case, "id"),
            autocomplete: text(case, "autocomplete"),
            label: text(case, "label"),
            placeholder: text(case, "placeholder"),
            ..Field::default()
        };
        assert_eq!(
            reason(&field).unwrap_or_default(),
            text(case, "reason"),
            "{case}"
        );
    }
}

#[test]
fn a_locator_that_names_a_secret_is_one() {
    for key in ["password", "csrf_token", "userPin", "cvc", "one-time-code"] {
        assert!(secret_text(key), "{key}");
    }
    for key in ["email", "givenName", "shipping", "compass", ""] {
        assert!(!secret_text(key), "{key}");
    }
}

#[test]
fn camel_case_and_separators_split_words() {
    assert_eq!(words("userPinCode"), ["user", "pin", "code"]);
    assert_eq!(
        words("__Request-Verification_token2"),
        ["request", "verification", "token2"]
    );
    assert_eq!(words("ABCdef"), ["abcdef"]);
}
