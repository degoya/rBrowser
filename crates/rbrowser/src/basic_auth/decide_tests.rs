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

/// RB-099-04: the challenge as CEF 156 really delivers it once Chromium's own prompt is off
/// (`net_service::LoginDelegate`): `origin_url` is the challenged request's whole URL - path and
/// query included -, `host` and `port` are the challenger's, `scheme` is Chromium's lower-case
/// `basic`. An environment behind https on its default port, the address written without a
/// trailing slash, is answered for any page of it, and a redirect onto `www.` is not.
#[test]
fn the_challenge_cef_delivers_for_a_page_of_the_environment_is_answered() {
    let config: ProjectConfig = serde_json::from_value(json!({
        "version": 1,
        "environments": [{ "id": "development", "url": "https://dev.example.test" }],
        "basicAuth": { "environments": ["development"] }
    }))
    .expect("config");
    for url in [
        "https://dev.example.test/",
        "https://dev.example.test/de/shop/?utm_source=x#top",
        "https://DEV.example.test:443/favicon.ico",
    ] {
        let found = eligible(&challenge(url, "dev.example.test", 443), &config)
            .unwrap_or_else(|refusal| panic!("{url}: {refusal}"));
        assert_eq!(found.environment, "development");
        assert_eq!(found.origin.to_string(), "https://dev.example.test:443");
    }
    assert_eq!(
        eligible(
            &challenge("https://www.dev.example.test/", "www.dev.example.test", 443),
            &config
        ),
        Err(Refusal::NotAnEnvironment)
    );
}

/// RB-099-04: environments with one address share one sign-in - the first listed of them
/// answers, its credential named by the origin all of them have.
#[test]
fn environments_with_one_address_share_the_answer() {
    let config: ProjectConfig = serde_json::from_value(json!({
        "version": 1,
        "environments": [
            { "id": "local", "url": "http://localhost:3000" },
            { "id": "staging", "url": "http://localhost:3000/" },
            { "id": "production", "url": "http://127.0.0.1:3000/" }
        ],
        "basicAuth": { "environments": ["staging", "local"] }
    }))
    .expect("config");
    let found = eligible(
        &challenge("http://localhost:3000/app", "localhost", 3000),
        &config,
    )
    .expect("answered");
    assert_eq!(found.environment, "local");
    assert_eq!(found.origin.to_string(), "http://localhost:3000");
}

/// RB-099-04: what rBrowser's sign-in dialog names - the server's origin, a proxy's host and
/// port - and never the request's path, query or user info.
#[test]
fn the_dialog_names_the_origin_only() {
    let page = challenge(
        "https://Staging.example.test/shop/?token=x",
        "staging.example.test",
        443,
    );
    assert_eq!(shown_origin(&page), "https://staging.example.test:443");
    let proxy = Challenge {
        is_proxy: true,
        ..challenge("https://staging.example.test/", "proxy.example.test", 3128)
    };
    assert_eq!(shown_origin(&proxy), "proxy.example.test:3128");
    // The scheme stays when the URL may carry no credential (review w35); the user info never shows.
    let user_info = challenge(
        "https://ada:pw@staging.example.test/",
        "staging.example.test",
        443,
    );
    assert_eq!(shown_origin(&user_info), "https://staging.example.test:443");
    let plain = challenge("http://plain.example.test/x?y=1", "plain.example.test", 80);
    assert_eq!(shown_origin(&plain), "http://plain.example.test:80");
    assert!(unencrypted(&plain));
    assert!(!unencrypted(&challenge(
        "http://localhost:8080/",
        "localhost",
        8080
    )));
    assert!(!unencrypted(&page));
    let plain_proxy = Challenge {
        is_proxy: true,
        ..plain
    };
    assert!(
        !unencrypted(&plain_proxy),
        "a proxy's own scheme is not the request's"
    );
}

/// RB-099-04: "Save for environment ..." only for an environment's own address that may carry
/// a credential (https, http on this machine), a server's Basic challenge - listed under
/// `basicAuth.environments` or not.
#[test]
fn an_answer_may_be_saved_only_for_an_environments_own_address() {
    let config = config();
    let saveable_for = |url, host, port| saveable(&challenge(url, host, port), &config);
    assert_eq!(
        saveable_for(
            "https://preview.example.test/x",
            "preview.example.test",
            443
        )
        .as_deref(),
        Some("preview")
    );
    assert_eq!(
        saveable_for("http://localhost:8080/", "localhost", 8080).as_deref(),
        Some("local")
    );
    assert_eq!(
        saveable_for("https://evil.example.test/", "evil.example.test", 443),
        None
    );
    assert_eq!(
        saveable_for("http://plain.example.test/", "plain.example.test", 80),
        None
    );
    assert_eq!(
        saveable_for("https://staging.example.test/", "evil.example.test", 443),
        None,
        "the host CEF names is not the origin's"
    );
    let proxy = Challenge {
        is_proxy: true,
        ..challenge("https://staging.example.test/", "staging.example.test", 443)
    };
    assert_eq!(saveable(&proxy, &config), None);
    let digest = Challenge {
        scheme: "digest",
        ..challenge("https://staging.example.test/", "staging.example.test", 443)
    };
    assert_eq!(saveable(&digest, &config), None);
}
