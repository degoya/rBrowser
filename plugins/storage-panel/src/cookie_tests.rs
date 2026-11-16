use serde_json::json;

use super::*;

const ANSWER: &str = r#"{"cookies":[
  {"name":"consent","value":"yes","domain":"127.0.0.1","path":"/","expires":-1,"size":10,
   "httpOnly":false,"secure":false,"session":true,"priority":"Medium","sameParty":false,
   "sourceScheme":"NonSecure","sourcePort":18767},
  {"name":"sid","value":"s3cr3t","domain":".example.com","path":"/app","expires":1893456000.5,
   "httpOnly":true,"secure":true,"session":false,"sameSite":"Lax",
   "partitionKey":{"topLevelSite":"https://example.com","hasCrossSiteAncestor":false}},
  {"name":"","value":"x","domain":"a","path":"/"},
  {"name":"old","value":"1","domain":"b","path":"/","partitionKey":"https://b.example"}
]}"#;

#[test]
fn every_attribute_of_a_cdp_cookie_is_read_httponly_included() {
    let cookies = parse_cookies(ANSWER).expect("reads");
    assert_eq!(cookies.len(), 3, "the nameless entry is skipped");
    let consent = &cookies[0];
    assert_eq!(consent.name, "consent");
    assert_eq!(consent.expires, None, "a session cookie has no expiry");
    assert!(!consent.http_only);
    let sid = &cookies[1];
    assert!(sid.http_only && sid.secure);
    assert_eq!(sid.same_site.as_deref(), Some("Lax"));
    assert_eq!(sid.expires, Some(1_893_456_000.5));
    assert_eq!(sid.partition.as_deref(), Some("https://example.com"));
    assert_eq!(cookies[2].partition.as_deref(), Some("https://b.example"));
}

#[test]
fn a_broken_answer_is_an_error() {
    assert!(parse_cookies("{}").is_err());
    assert!(parse_cookies("nope").is_err());
}

#[test]
fn cookies_compare_by_value_and_flags_not_expiry() {
    let mut cookies = parse_cookies(ANSWER).expect("reads");
    let a = cookies.remove(1);
    let mut b = a.clone();
    b.expires = Some(1.0);
    assert!(a.same_as(&b));
    b.http_only = false;
    assert!(!a.same_as(&b));
    assert_eq!(a.identity(), b.identity());
}

#[test]
fn a_spec_checks_name_value_and_every_attribute() {
    let spec = spec(
        "featureFlag",
        "on",
        &json!({ "domain": ".Example.com", "path": "/x", "expires": 2e9, "httpOnly": true,
                 "secure": true, "sameSite": "Strict" }),
    )
    .expect("valid");
    assert_eq!(spec.domain.as_deref(), Some(".example.com"));
    assert!(spec.http_only && spec.secure);
    assert_eq!(spec.path, "/x");

    let plain = super::spec("a", "b", &json!(null)).expect("defaults");
    assert_eq!(plain.path, "/");
    assert_eq!(plain.domain, None);

    for (name, value, attrs, field) in [
        ("", "v", json!({}), "name"),
        ("a;b", "v", json!({}), "name"),
        ("a b", "v", json!({}), "name"),
        ("a", "x;y", json!({}), "value"),
        ("a", "line\nbreak", json!({}), "value"),
        ("a", "v", json!({ "domain": "a/b" }), "domain"),
        ("a", "v", json!({ "path": "relative" }), "path"),
        ("a", "v", json!({ "expires": -1 }), "expires"),
        ("a", "v", json!({ "sameSite": "lax" }), "sameSite"),
        ("a", "v", json!({ "secure": "yes" }), "secure"),
        (
            "a",
            "v",
            json!({ "partition": "https://x.example/p" }),
            "partition",
        ),
        ("a", "v", json!("x"), "cookie"),
    ] {
        assert_eq!(
            super::spec(name, value, &attrs).err().as_deref(),
            Some(field)
        );
    }
    let long = "v".repeat(MAX_COOKIE_BYTES);
    assert_eq!(
        super::spec("a", &long, &json!({})).err().as_deref(),
        Some("value")
    );
}

#[test]
fn set_params_keep_host_only_cookies_host_only() {
    let host_only = spec("consent", "yes", &json!({})).expect("valid");
    let params = set_params(&host_only, "http://127.0.0.1:18767/storage/");
    assert_eq!(params["url"], "http://127.0.0.1:18767/");
    assert!(params.get("domain").is_none());

    let reported = spec(
        "consent",
        "yes",
        &json!({ "domain": "127.0.0.1", "secure": true }),
    )
    .expect("valid");
    assert_eq!(
        set_params(&reported, "http://127.0.0.1:18767/")["url"],
        "https://127.0.0.1/"
    );

    let domain = spec(
        "sid",
        "1",
        &json!({ "domain": ".example.com", "sameSite": "None", "expires": 5.0,
                 "partition": "https://example.com" }),
    )
    .expect("valid");
    let params = set_params(&domain, "https://a.example.com/");
    assert_eq!(params["domain"], ".example.com");
    assert_eq!(params["sameSite"], "None");
    assert_eq!(params["expires"], 5.0);
    assert_eq!(
        params["partitionKey"]["topLevelSite"],
        "https://example.com"
    );
    assert!(params.get("url").is_none());
}

#[test]
fn delete_params_name_the_exact_cookie() {
    let params = delete_params(
        "sid",
        Some(".example.com"),
        "/app",
        None,
        "https://example.com/",
    );
    assert_eq!(
        params,
        json!({ "name": "sid", "domain": ".example.com", "path": "/app" })
    );
    let params = delete_params(
        "a",
        None,
        "/",
        Some("https://x.example"),
        "http://h.example:81/q",
    );
    assert_eq!(params["domain"], "h.example");
    assert_eq!(params["partitionKey"]["topLevelSite"], "https://x.example");
}
