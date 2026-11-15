use serde_json::json;

use super::*;

#[test]
fn sensitive_headers_are_masked_case_blind_and_the_others_kept() {
    let headers = vec![
        ("Cookie".to_owned(), "sid=1".to_owned()),
        ("Accept".to_owned(), "*/*".to_owned()),
        ("AUTHORIZATION".to_owned(), "Bearer x".to_owned()),
        ("set-cookie".to_owned(), "a=b".to_owned()),
    ];
    assert_eq!(
        mask_headers(&headers),
        vec![
            ("Cookie".to_owned(), MASK.to_owned()),
            ("Accept".to_owned(), "*/*".to_owned()),
            ("AUTHORIZATION".to_owned(), MASK.to_owned()),
            ("set-cookie".to_owned(), MASK.to_owned()),
        ]
    );
}

#[test]
fn sensitive_query_parameters_are_masked_and_the_rest_of_the_url_stays() {
    assert_eq!(
        mask_url("https://a.test/p?q=1&Token=abc&api_key=k#frag"),
        "https://a.test/p?q=1&Token=***&api_key=***#frag"
    );
    assert_eq!(mask_url("https://a.test/p"), "https://a.test/p");
    assert_eq!(mask_url("https://a.test/p?flag"), "https://a.test/p?flag");
}

#[test]
fn an_entry_is_masked_in_place() {
    let mut entry = json!({
        "url": "https://a.test/?sid=9",
        "headers": [["cookie", "a=b"], ["accept", "*/*"]],
        "response": { "status": 200, "headers": [["set-cookie", "x=y"], ["content-type", "text/html"]] },
    });
    mask_entry(&mut entry);
    assert_eq!(entry["url"], "https://a.test/?sid=***");
    assert_eq!(
        entry["headers"],
        json!([["cookie", "***"], ["accept", "*/*"]])
    );
    assert_eq!(
        entry["response"]["headers"],
        json!([["set-cookie", "***"], ["content-type", "text/html"]])
    );
}
