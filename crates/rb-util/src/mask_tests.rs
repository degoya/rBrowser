use super::*;

#[test]
fn sensitive_names_are_matched_case_blind() {
    assert!(sensitive_header("Set-Cookie") && sensitive_header("X-CSRF-Token"));
    assert!(sensitive_header("x-amz-security-token") && sensitive_header("WWW-Authenticate"));
    assert!(!sensitive_header("content-type"));
    assert!(sensitive_param("Token") && sensitive_param("api_key") && sensitive_param("sid"));
    assert!(!sensitive_param("page"));
}

#[test]
fn headers_and_urls_keep_their_shape_and_lose_their_secrets() {
    let headers = vec![
        ("Cookie".to_owned(), "sid=1".to_owned()),
        ("Accept".to_owned(), "*/*".to_owned()),
    ];
    assert_eq!(
        mask_headers(&headers),
        vec![
            ("Cookie".to_owned(), MASK.to_owned()),
            ("Accept".to_owned(), "*/*".to_owned()),
        ]
    );
    assert_eq!(
        mask_url("https://a.test/p?q=1&Token=abc&flag#f"),
        "https://a.test/p?q=1&Token=***&flag#f"
    );
    assert_eq!(mask_url("https://a.test/p"), "https://a.test/p");
}
