use rb_config::ProjectConfig;
use serde_json::json;

use super::*;

fn config() -> ProjectConfig {
    serde_json::from_value(json!({
        "version": 1,
        "environments": [
            { "id": "local", "url": "http://localhost:8080/" },
            { "id": "staging", "url": "https://staging.example.test/shop/" },
            { "id": "preview", "url": "https://preview.example.test/" },
            { "id": "insecure", "url": "http://plain.example.test/" },
            { "id": "embedded", "url": "https://ada:pw@embedded.example.test/" }
        ],
        "basicAuth": { "environments": ["local", "staging", "insecure", "embedded"] }
    }))
    .expect("config")
}

fn challenge(origin_url: &'static str, host: &'static str, port: i32) -> Challenge<'static> {
    Challenge {
        origin_url,
        is_proxy: false,
        host,
        port,
        realm: "Staging",
        scheme: "basic",
    }
}

#[test]
fn the_environments_own_origin_is_answered() {
    let found = eligible(
        &challenge("https://staging.example.test/", "staging.example.test", 443),
        &config(),
    )
    .expect("answered");
    assert_eq!(found.environment, "staging");
    assert_eq!(found.origin.to_string(), "https://staging.example.test:443");
    let local = eligible(
        &challenge("http://localhost:8080/", "localhost", 8080),
        &config(),
    )
    .expect("loopback http is answered");
    assert_eq!(local.environment, "local");
}

#[test]
fn another_host_never_gets_the_credential() {
    for (url, host, port) in [
        // A second site the page loads from, or a redirect target elsewhere.
        ("https://evil.example.test/", "evil.example.test", 443),
        (
            "https://cdn.staging.example.test/",
            "cdn.staging.example.test",
            443,
        ),
        (
            "https://staging.example.test.evil.test/",
            "staging.example.test.evil.test",
            443,
        ),
        // The same host on another port or scheme is another origin.
        (
            "https://staging.example.test:8443/",
            "staging.example.test",
            8443,
        ),
        ("http://localhost:9000/", "localhost", 9000),
    ] {
        assert_eq!(
            eligible(&challenge(url, host, port), &config()).map(|found| found.environment),
            Err(Refusal::NotAnEnvironment),
            "{url}"
        );
    }
}

#[test]
fn an_environment_without_basic_auth_is_not_answered() {
    assert_eq!(
        eligible(
            &challenge("https://preview.example.test/", "preview.example.test", 443),
            &config()
        )
        .map(|found| found.environment),
        Err(Refusal::NotAnEnvironment)
    );
}

#[test]
fn plain_http_to_another_machine_is_never_answered() {
    assert_eq!(
        eligible(
            &challenge("http://plain.example.test/", "plain.example.test", 80),
            &config()
        )
        .map(|found| found.environment),
        Err(Refusal::Origin(OriginError::PlainHttp))
    );
}

#[test]
fn an_environment_url_with_a_credential_in_it_is_never_answered() {
    // The config's own `user:password@` is refused, not taken as the credential.
    assert_eq!(
        eligible(
            &challenge(
                "https://embedded.example.test/",
                "embedded.example.test",
                443
            ),
            &config()
        )
        .map(|found| found.environment),
        Err(Refusal::NotAnEnvironment)
    );
    // A challenge naming user info is refused outright.
    assert_eq!(
        eligible(
            &challenge(
                "https://x:y@staging.example.test/",
                "staging.example.test",
                443
            ),
            &config()
        )
        .map(|found| found.environment),
        Err(Refusal::Origin(OriginError::UserInfo))
    );
}

#[test]
fn a_proxy_another_scheme_or_a_host_that_does_not_match_is_refused() {
    let mut proxy = challenge("https://staging.example.test/", "staging.example.test", 443);
    proxy.is_proxy = true;
    assert_eq!(
        eligible(&proxy, &config()).map(|found| found.environment),
        Err(Refusal::Proxy)
    );
    for scheme in ["digest", "ntlm", "negotiate", ""] {
        let mut other = challenge("https://staging.example.test/", "staging.example.test", 443);
        other.scheme = scheme;
        assert_eq!(
            eligible(&other, &config()).map(|found| found.environment),
            Err(Refusal::Scheme),
            "{scheme}"
        );
    }
    let mut upper = challenge("https://staging.example.test/", "staging.example.test", 443);
    upper.scheme = "Basic";
    assert!(eligible(&upper, &config()).is_ok());
    // The origin says staging, the host CEF names does not: refused.
    assert_eq!(
        eligible(
            &challenge("https://staging.example.test/", "evil.example.test", 443),
            &config()
        )
        .map(|found| found.environment),
        Err(Refusal::HostPort)
    );
    assert_eq!(
        eligible(
            &challenge("https://staging.example.test/", "staging.example.test", -1),
            &config()
        )
        .map(|found| found.environment),
        Err(Refusal::HostPort)
    );
}
