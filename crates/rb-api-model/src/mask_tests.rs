use serde_json::json;

use super::*;

#[test]
fn a_secret_is_masked_as_it_is_and_escaped() {
    let secrets = vec!["p@ss word".to_owned(), "q\"x".to_owned()];
    assert_eq!(
        mask("a p@ss word b p@ss%20word c q\\\"x", &secrets),
        "a *** b *** c ***"
    );
}

#[test]
fn the_longest_secret_goes_first() {
    let secrets = vec!["abc".to_owned(), "abcdef".to_owned()];
    assert_eq!(mask("abcdef abc", &secrets), "*** ***");
}

#[test]
fn a_short_value_is_left_alone() {
    assert_eq!(mask("a ab", &["ab".to_owned()]), "a ab");
}

#[test]
fn json_is_masked_in_values_and_keys() {
    let secrets = vec!["tok-1".to_owned()];
    let masked = mask_json(
        &json!({ "headers": { "authorization": "Bearer tok-1" }, "tok-1": [1, "tok-1"] }),
        &secrets,
    );
    assert_eq!(
        masked,
        json!({ "headers": { "authorization": "Bearer ***" }, "***": [1, "***"] })
    );
}

#[test]
fn a_secret_is_masked_form_encoded_as_a_query_carries_it() {
    let secrets = vec!["p@ss word/x".to_owned()];
    assert_eq!(mask("?key=p%40ss+word%2Fx&b=1", &secrets), "?key=***&b=1");
}
