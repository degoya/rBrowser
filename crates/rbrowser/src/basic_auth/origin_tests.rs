use super::*;

fn origin(url: &str) -> AuthOrigin {
    AuthOrigin::parse(url).expect(url)
}

#[test]
fn an_https_origin_keeps_scheme_host_and_port_in_lower_case() {
    let parsed = origin("HTTPS://Staging.Example.TEST/shop/?q=1#top");
    assert_eq!(parsed.scheme(), "https");
    assert_eq!(parsed.host(), "staging.example.test");
    assert_eq!(parsed.port(), 443);
    assert_eq!(parsed.to_string(), "https://staging.example.test:443");
}

#[test]
fn the_default_port_and_an_explicit_one_are_the_same_origin() {
    assert_eq!(
        origin("https://staging.example.test/"),
        origin("https://staging.example.test:443/a")
    );
    assert_ne!(
        origin("https://staging.example.test/"),
        origin("https://staging.example.test:8443/")
    );
    assert_ne!(
        origin("https://staging.example.test/"),
        origin("https://www.staging.example.test/")
    );
}

#[test]
fn plain_http_is_only_for_the_own_machine() {
    for url in [
        "http://localhost:8080/",
        "http://app.localhost/",
        "http://127.0.0.1:3000/",
        "http://127.1/",
        "http://[::1]:5173/",
    ] {
        assert!(AuthOrigin::parse(url).is_ok(), "{url}");
    }
    for url in [
        "http://staging.example.test/",
        "http://10.0.0.5/",
        "http://localhost.example.test/",
        "http://[::2]/",
    ] {
        assert_eq!(AuthOrigin::parse(url), Err(OriginError::PlainHttp), "{url}");
    }
}

#[test]
fn a_credential_in_the_url_is_refused() {
    for url in [
        "https://admin:secret@staging.example.test/",
        "https://admin@staging.example.test/",
        "https://:secret@staging.example.test/",
    ] {
        assert_eq!(AuthOrigin::parse(url), Err(OriginError::UserInfo), "{url}");
    }
}

#[test]
fn other_schemes_and_garbage_are_refused() {
    for url in [
        "ftp://staging.example.test/",
        "file:///C:/secret.txt",
        "rbrowser://ui/",
        "data:text/plain,x",
        "not a url",
        "",
    ] {
        assert!(AuthOrigin::parse(url).is_err(), "{url}");
    }
}

#[test]
fn the_challenge_host_and_port_must_be_the_origins() {
    let parsed = origin("https://staging.example.test/");
    assert!(parsed.matches_host_port("staging.example.test", 443));
    assert!(parsed.matches_host_port("STAGING.example.test", 443));
    assert!(!parsed.matches_host_port("staging.example.test", 8443));
    assert!(!parsed.matches_host_port("evil.example.test", 443));
    let v6 = origin("http://[::1]:5173/");
    assert!(v6.matches_host_port("[::1]", 5173));
    assert!(v6.matches_host_port("::1", 5173));
}
