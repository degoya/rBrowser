use rb_api_model::{Body, to_http};

use super::*;

fn captured(method: &str, url: &str, headers: &[(&str, &str)], body: Option<&str>) -> Captured {
    Captured {
        method: method.into(),
        url: url.into(),
        headers: headers
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect(),
        body: body.map(str::to_owned),
        body_unread: false,
    }
}

fn header<'a>(converted: &'a Converted, name: &str) -> Option<&'a str> {
    converted
        .request
        .headers
        .iter()
        .find(|entry| entry.name.eq_ignore_ascii_case(name))
        .map(|entry| entry.value.as_str())
}

fn value_of<'a>(converted: &'a Converted, name: &str) -> Option<Option<&'a str>> {
    converted
        .variables
        .iter()
        .find(|variable| variable.name == name)
        .map(|variable| variable.value.as_deref())
}

#[test]
fn a_bearer_token_becomes_a_variable_and_its_value_stays_out_of_the_file() {
    let converted = convert(&captured(
        "GET",
        "https://api.example.test/v1/me",
        &[
            ("Authorization", "Bearer tok-live-123456"),
            ("Accept", "application/json"),
        ],
        None,
    ))
    .expect("an http request converts");
    assert_eq!(
        header(&converted, "Authorization"),
        Some("Bearer {{api-example-test-token}}")
    );
    assert_eq!(header(&converted, "Accept"), Some("application/json"));
    assert_eq!(
        value_of(&converted, "api-example-test-token"),
        Some(Some("tok-live-123456"))
    );
    let text = to_http(&converted.request);
    assert!(!text.contains("tok-live-123456"), "{text}");
}

#[test]
fn cookies_and_token_headers_become_variables_and_browser_headers_are_left_out() {
    let converted = convert(&captured(
        "GET",
        "https://api.example.test/v1/cart",
        &[
            ("Cookie", "sid=abc123456; theme=dark"),
            ("X-Api-Key", "key-plain-one"),
            ("X-CSRF-Token", "csrf-1122334455"),
            ("Host", "api.example.test"),
            ("Content-Length", "0"),
            ("Accept-Encoding", "gzip, br, zstd"),
            ("sec-ch-ua", "\"Chromium\";v=\"140\""),
            (":authority", "api.example.test"),
            ("X-Request-Id", "r-1"),
        ],
        None,
    ))
    .expect("converts");
    assert_eq!(
        header(&converted, "Cookie"),
        Some("{{api-example-test-cookie}}")
    );
    assert_eq!(
        header(&converted, "X-Api-Key"),
        Some("{{api-example-test-x-api-key}}")
    );
    assert_eq!(
        header(&converted, "X-CSRF-Token"),
        Some("{{api-example-test-x-csrf-token}}")
    );
    for gone in [
        "Host",
        "Content-Length",
        "Accept-Encoding",
        "sec-ch-ua",
        ":authority",
    ] {
        assert_eq!(header(&converted, gone), None, "{gone}");
    }
    assert_eq!(header(&converted, "X-Request-Id"), Some("r-1"));
    assert_eq!(
        value_of(&converted, "api-example-test-cookie"),
        Some(Some("sid=abc123456; theme=dark"))
    );
}

#[test]
fn a_value_the_capture_masked_becomes_a_variable_without_a_value() {
    let converted = convert(&captured(
        "GET",
        "https://api.example.test/v1/feed?access_token=***&page=2",
        &[("X-Api-Key", "***")],
        None,
    ))
    .expect("converts");
    assert_eq!(
        converted.request.url,
        "https://api.example.test/v1/feed?access_token={{api-example-test-access-token}}&page=2"
    );
    assert_eq!(
        value_of(&converted, "api-example-test-access-token"),
        Some(None)
    );
    assert_eq!(
        value_of(&converted, "api-example-test-x-api-key"),
        Some(None)
    );
    assert!(!converted.incomplete);
}

#[test]
fn a_secret_query_value_the_capture_kept_is_kept_as_written() {
    let converted = convert(&captured(
        "GET",
        "https://api.example.test/v1/search?q=shoes&api_key=k%2B123456",
        &[],
        None,
    ))
    .expect("converts");
    assert_eq!(
        converted.request.url,
        "https://api.example.test/v1/search?q=shoes&api_key={{api-example-test-api-key}}"
    );
    assert_eq!(
        value_of(&converted, "api-example-test-api-key"),
        Some(Some("k%2B123456"))
    );
}

#[test]
fn a_json_body_keeps_its_fields_and_its_secret_ones_become_variables() {
    let converted = convert(&captured(
        "POST",
        "https://api.example.test/v1/login",
        &[("Content-Type", "application/json; charset=utf-8")],
        Some(r#"{"user":"ada","password":"pw-123456","nested":{"refresh_token":"rt-998877"}}"#),
    ))
    .expect("converts");
    let Body::Json { text } = &converted.request.body else {
        panic!("a JSON body: {:?}", converted.request.body);
    };
    assert!(text.contains("\"ada\""), "{text}");
    assert!(text.contains("{{api-example-test-password}}"), "{text}");
    assert!(
        text.contains("{{api-example-test-refresh-token}}"),
        "{text}"
    );
    assert!(
        !text.contains("pw-123456") && !text.contains("rt-998877"),
        "{text}"
    );
    assert_eq!(
        value_of(&converted, "api-example-test-password"),
        Some(Some("pw-123456"))
    );
}

#[test]
fn a_json_body_without_secrets_keeps_its_text_as_it_was() {
    let converted = convert(&captured(
        "PUT",
        "https://api.example.test/v1/items/7",
        &[("content-type", "application/json")],
        Some(r#"{"name":"lamp","count":2}"#),
    ))
    .expect("converts");
    assert_eq!(
        converted.request.body,
        Body::Json {
            text: r#"{"name":"lamp","count":2}"#.into()
        }
    );
    assert!(converted.variables.is_empty());
}

#[test]
fn a_form_body_is_read_as_fields_with_its_secrets_as_variables() {
    let converted = convert(&captured(
        "POST",
        "https://api.example.test/session",
        &[("Content-Type", "application/x-www-form-urlencoded")],
        Some("user=ada+lovelace&password=pw%26123456"),
    ))
    .expect("converts");
    let Body::Form { fields } = &converted.request.body else {
        panic!("a form body: {:?}", converted.request.body);
    };
    assert_eq!(fields[0].name, "user");
    assert_eq!(fields[0].value, "ada lovelace");
    assert_eq!(fields[1].value, "{{api-example-test-password}}");
    assert_eq!(
        value_of(&converted, "api-example-test-password"),
        Some(Some("pw&123456"))
    );
}

#[test]
fn another_body_is_raw_text() {
    let converted = convert(&captured(
        "POST",
        "https://api.example.test/upload",
        &[("Content-Type", "text/plain")],
        Some("hello"),
    ))
    .expect("converts");
    assert_eq!(
        converted.request.body,
        Body::Raw {
            text: "hello".into()
        }
    );
}

#[test]
fn a_known_secret_elsewhere_in_the_request_becomes_its_variable_too() {
    let converted = convert(&captured(
        "POST",
        "https://api.example.test/v1/echo",
        &[
            ("Authorization", "Bearer tok-live-123456"),
            ("X-Echo", "was tok-live-123456"),
            ("Content-Type", "text/plain"),
        ],
        Some("token is tok-live-123456"),
    ))
    .expect("converts");
    assert_eq!(
        header(&converted, "X-Echo"),
        Some("was {{api-example-test-token}}")
    );
    assert_eq!(
        converted.request.body,
        Body::Raw {
            text: "token is {{api-example-test-token}}".into()
        }
    );
}

#[test]
fn the_title_folder_and_path_come_from_the_method_host_and_path() {
    let converted = convert(&captured(
        "get",
        "https://API.example.test:8443/v1/users/42?x=1#part",
        &[],
        None,
    ))
    .expect("converts");
    assert_eq!(converted.request.method, "GET");
    assert_eq!(converted.request.name, "GET /v1/users/42");
    assert_eq!(converted.folder, "api-example-test-8443");
    assert_eq!(converted.path, "/v1/users/42");
    assert_eq!(
        converted.request.url,
        "https://API.example.test:8443/v1/users/42?x=1"
    );
    let root = convert(&captured("GET", "http://127.0.0.1:3000", &[], None)).expect("converts");
    assert_eq!(root.request.name, "GET /");
    assert_eq!(root.folder, "127-0-0-1-3000");
}

/// The w31 review: a secret in the path is its variable there, and the path a duplicate is
/// found by is the one the saved file holds.
#[test]
fn the_path_of_a_duplicate_is_the_one_with_the_variables() {
    let converted = convert(&captured(
        "GET",
        "https://api.example.test/v1/keys/tok-live-123456?x=1",
        &[("Authorization", "Bearer tok-live-123456")],
        None,
    ))
    .expect("converts");
    assert_eq!(converted.path, "/v1/keys/{{api-example-test-token}}");
    assert_eq!(
        converted.request.url,
        "https://api.example.test/v1/keys/{{api-example-test-token}}?x=1"
    );
}

#[test]
fn a_variable_name_starts_with_a_letter() {
    let converted = convert(&captured(
        "GET",
        "http://127.0.0.1:3000/me",
        &[("Authorization", "Basic YWRhOnB3LTEyMzQ1Ng==")],
        None,
    ))
    .expect("converts");
    assert_eq!(
        header(&converted, "Authorization"),
        Some("Basic {{host-127-0-0-1-3000-basic}}")
    );
    assert!(
        converted
            .variables
            .iter()
            .all(|variable| rb_api_model::vars::is_name(&variable.name))
    );
}

#[test]
fn only_http_and_https_are_taken() {
    for url in [
        "data:text/plain,hi",
        "chrome-extension://abc/x.js",
        "ws://a.test/socket",
        "",
    ] {
        assert!(
            matches!(
                convert(&captured("GET", url, &[], None)),
                Err(Refused::NotHttp)
            ),
            "{url}"
        );
    }
}

#[test]
fn the_user_info_of_the_address_is_left_out() {
    let converted = convert(&captured(
        "GET",
        "https://ada:pw@api.example.test/v1",
        &[],
        None,
    ))
    .expect("converts");
    assert_eq!(converted.request.url, "https://api.example.test/v1");
}

#[test]
fn a_body_over_the_limit_is_left_out_and_said() {
    let large = "x".repeat(MAX_BODY_CHARS + 1);
    let converted = convert(&captured(
        "POST",
        "https://api.example.test/upload",
        &[("Content-Type", "text/plain")],
        Some(&large),
    ))
    .expect("converts");
    assert_eq!(converted.request.body, Body::None);
    assert_eq!(converted.body_missing, Some(BodyMissing::TooLong));
}

#[test]
fn an_address_masked_or_cut_elsewhere_is_said() {
    let masked = convert(&captured(
        "GET",
        "https://api.example.test/v1/***/items",
        &[],
        None,
    ))
    .expect("converts");
    assert!(masked.incomplete);
    let long = format!("https://api.example.test/{}...", "a".repeat(2100));
    assert!(
        convert(&captured("GET", &long, &[], None))
            .expect("converts")
            .incomplete
    );
}

#[test]
fn two_values_under_one_name_get_two_variables() {
    let converted = convert(&captured(
        "GET",
        "https://api.example.test/v1?token=one-123456",
        &[("X-Token", "two-123456"), ("Token", "three-123456")],
        None,
    ))
    .expect("converts");
    assert_eq!(
        value_of(&converted, "api-example-test-token"),
        Some(Some("one-123456"))
    );
    assert_eq!(
        value_of(&converted, "api-example-test-token-2"),
        Some(Some("three-123456"))
    );
    assert_eq!(
        header(&converted, "Token"),
        Some("{{api-example-test-token-2}}")
    );
}

#[test]
fn the_referer_keeps_its_address_with_the_secrets_as_variables() {
    let converted = convert(&captured(
        "GET",
        "https://api.example.test/v1",
        &[(
            "Referer",
            "https://shop.example.test/cb?code=abc-123456&x=1",
        )],
        None,
    ))
    .expect("converts");
    assert_eq!(
        header(&converted, "Referer"),
        Some("https://shop.example.test/cb?code={{api-example-test-code}}&x=1")
    );
}

#[test]
fn a_variable_shows_its_name_and_never_its_value() {
    let variable = Variable {
        name: "a-token".into(),
        value: Some("tok-live-123456".into()),
    };
    let shown = format!("{variable:?}");
    assert!(
        shown.contains("a-token") && !shown.contains("tok-live"),
        "{shown}"
    );
}
