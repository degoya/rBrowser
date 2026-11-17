use super::*;

#[test]
fn page_text_without_secrets_comes_back_unchanged() {
    for text in [
        "GET https://shop.test/search?q=shoes&page=2#results",
        "Uncaught TypeError: x is undefined (exit code=1)",
        "https://shop.test/product?code=",
    ] {
        assert_eq!(redact_page_text(text), text);
    }
}

#[test]
fn oauth_and_signed_url_parameters_are_masked() {
    assert_eq!(
        redact_page_text("https://app.test/callback?code=4/0AbCdEf&state=xyz"),
        "https://app.test/callback?code=***&state=xyz"
    );
    assert_eq!(
        redact_page_text("https://app.test/#access_token=ya29.a0&expires_in=3599"),
        "https://app.test/#access_token=***&expires_in=3599"
    );
    assert_eq!(
        redact_page_text(
            "https://b.s3.test/k?X-Amz-Credential=AKIA%2F1&X-Amz-Signature=deadbeef&x=1"
        ),
        "https://b.s3.test/k?X-Amz-Credential=***&X-Amz-Signature=***&x=1"
    );
    assert_eq!(
        redact_page_text("blob?sv=2024&sig=AbC%3D&se=2026"),
        "blob?sv=2024&sig=***&se=2026"
    );
    assert_eq!(
        redact_page_text("https://idp.test/sso?SAMLResponse=PHNhbWw+&RelayState=a"),
        "https://idp.test/sso?SAMLResponse=***&RelayState=a"
    );
}

#[test]
fn the_logs_rules_hold_too_and_twice_is_once() {
    let text = "fetch https://u:p@api.test/v1?api_key=k1&oauth_verifier=v9 Bearer abc.def";
    let once = redact_page_text(text);
    assert_eq!(
        once,
        "fetch https://***@api.test/v1?api_key=***&oauth_verifier=*** Bearer ***"
    );
    assert_eq!(redact_page_text(&once), once);
}
