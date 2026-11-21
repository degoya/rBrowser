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

#[test]
fn the_one_list_holds_every_credential_name_the_other_lists_had() {
    // RB-094-10: the union of the app's, the log's, the page text's and the recorder's lists.
    for name in [
        "X-Amz-Signature",
        "X-Amz-Credential",
        "X-Amz-Security-Token",
        "X-Goog-Signature",
        "X-Goog-Credential",
        "jwt",
        "ticket",
        "SAMLResponse",
        "SAMLRequest",
        "assertion",
        "oauth_verifier",
        "code",
        "id_token",
        "access_key",
        "private_key",
        "passphrase",
        "api-key",
        "authtoken",
    ] {
        assert!(sensitive_param(name), "{name}");
    }
    // OAuth's `state` is a CSRF nonce, not a credential: no list treated it as one.
    assert!(!sensitive_param("state"));
}

#[test]
fn the_fragment_s_parameters_and_the_user_info_are_masked_too() {
    assert_eq!(
        mask_url("https://app.test/cb#access_token=ya29&expires_in=3599"),
        "https://app.test/cb#access_token=***&expires_in=3599"
    );
    // A hash route's own query, and `;` as a separator.
    assert_eq!(
        mask_url("https://app.test/#/done?jwt=e.y.j&x=1;ticket=t"),
        "https://app.test/#/done?jwt=***&x=1;ticket=***"
    );
    assert_eq!(
        mask_url("https://user:pw@a.test/p?q=1"),
        "https://***@a.test/p?q=1"
    );
    assert_eq!(mask_url("https://a.test/p#frag"), "https://a.test/p#frag");
    assert_eq!(mask_url("mailto:a@b.test"), "mailto:a@b.test");
    for url in [
        "https://u:p@a.test/?code=c#id_token=t",
        "https://a.test/?=x&&",
        "about:blank",
    ] {
        let once = mask_url(url);
        assert_eq!(mask_url(&once), once, "{url}");
    }
}

#[test]
fn a_caller_adds_its_own_names_and_mask() {
    let extra = vec![" OTP ".to_owned()];
    assert_eq!(
        mask_url_with("https://a.test/?otp=1&page=2#token=t", &extra, "########"),
        "https://a.test/?otp=########&page=2#token=########"
    );
    assert_eq!(
        mask_url_with("https://a.test/?=x&&", &[String::new()], "#"),
        "https://a.test/?=x&&"
    );
}
