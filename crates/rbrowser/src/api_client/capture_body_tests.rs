use rb_api_model::{Body, to_http};

use crate::api_client::capture::{BodyMissing, Captured, Converted, convert};

fn post(content_type: Option<&str>, body: &str) -> Converted {
    let headers = content_type
        .map(|value| vec![("Content-Type".to_owned(), value.to_owned())])
        .unwrap_or_default();
    convert(&Captured {
        method: "POST".into(),
        url: "https://api.example.test/v1/login".into(),
        headers,
        body: Some(body.to_owned()),
        body_unread: false,
    })
    .expect("an http request converts")
}

fn value_of<'a>(converted: &'a Converted, name: &str) -> Option<Option<&'a str>> {
    converted
        .variables
        .iter()
        .find(|variable| variable.name == name)
        .map(|variable| variable.value.as_deref())
}

/// The finding of the w31 review: `fetch(url, {method: 'POST', body: JSON.stringify(...)})`
/// without headers is sent as `text/plain;charset=UTF-8`.
#[test]
fn json_sent_as_plain_text_has_its_secrets_made_variables() {
    let converted = post(
        Some("text/plain;charset=UTF-8"),
        r#"{"user":"ada","password":"pw-123456"}"#,
    );
    let Body::Json { text } = &converted.request.body else {
        panic!("a JSON body: {:?}", converted.request.body);
    };
    assert!(text.contains("{{api-example-test-password}}"), "{text}");
    assert!(!to_http(&converted.request).contains("pw-123456"));
    assert_eq!(
        value_of(&converted, "api-example-test-password"),
        Some(Some("pw-123456"))
    );
    assert_eq!(converted.body_missing, None);
    let untyped = post(None, r#"[{"password":"pw-123456"}]"#);
    assert!(!to_http(&untyped.request).contains("pw-123456"));
    assert!(matches!(untyped.request.body, Body::Json { .. }));
}

#[test]
fn a_multipart_body_has_its_secret_fields_made_variables() {
    let text = "--XyZ\r\nContent-Disposition: form-data; name=\"user\"\r\n\r\nada\r\n\
                --XyZ\r\nContent-Disposition: form-data; name=\"password\"\r\n\r\npw-123456\r\n\
                --XyZ--\r\n";
    let converted = post(Some("multipart/form-data; boundary=XyZ"), text);
    let Body::Multipart { parts } = &converted.request.body else {
        panic!("a multipart body: {:?}", converted.request.body);
    };
    assert_eq!(parts.len(), 2);
    assert_eq!(
        (parts[0].name.as_str(), parts[0].value.as_str()),
        ("user", "ada")
    );
    assert_eq!(parts[1].value, "{{api-example-test-password}}");
    assert!(!to_http(&converted.request).contains("pw-123456"));
    assert_eq!(
        value_of(&converted, "api-example-test-password"),
        Some(Some("pw-123456"))
    );
}

#[test]
fn a_multipart_body_that_cannot_be_checked_is_left_out_and_said() {
    let file = "--XyZ\r\nContent-Disposition: form-data; name=\"token\"\r\n\r\ntok-live-123456\r\n\
                --XyZ\r\nContent-Disposition: form-data; name=\"f\"; filename=\"a.bin\"\r\n\r\n\
                secret bytes\r\n--XyZ--\r\n";
    let converted = post(Some("multipart/form-data; boundary=XyZ"), file);
    assert_eq!(converted.request.body, Body::None);
    assert_eq!(converted.body_missing, Some(BodyMissing::Unchecked));
    assert!(
        converted.variables.is_empty(),
        "no variable of a body left out"
    );
    let no_boundary = post(
        Some("multipart/form-data"),
        "--XyZ\r\nContent-Disposition: form-data; name=\"a\"\r\n\r\nb\r\n--XyZ--",
    );
    assert_eq!(no_boundary.body_missing, Some(BodyMissing::Unchecked));
    let unclosed = post(
        Some("multipart/form-data; boundary=XyZ"),
        "--XyZ\r\nContent-Disposition: form-data; name=\"a\"\r\n\r\nb",
    );
    assert_eq!(unclosed.body_missing, Some(BodyMissing::Unchecked));
}

#[test]
fn other_text_naming_a_secret_before_a_value_is_left_out() {
    for text in [
        "user=ada&password=pw-123456",
        "<login><secret>pw-123456</secret></login>",
        "secret: pw-123456",
        r#"{"password": "pw-123456""#,
    ] {
        let converted = post(Some("text/plain"), text);
        assert_eq!(converted.request.body, Body::None, "{text}");
        assert_eq!(
            converted.body_missing,
            Some(BodyMissing::Unchecked),
            "{text}"
        );
        assert!(!to_http(&converted.request).contains("pw-123456"), "{text}");
    }
    let broken_json = post(Some("application/json"), r#"{"password":"pw-123456""#);
    assert_eq!(broken_json.body_missing, Some(BodyMissing::Unchecked));
    let prose = post(Some("text/plain"), "the token is not here");
    assert_eq!(
        prose.request.body,
        Body::Raw {
            text: "the token is not here".into()
        }
    );
    assert_eq!(prose.body_missing, None);
}

#[test]
fn json_deeper_than_looked_through_is_left_out() {
    let deep = format!(
        "{}{{\"password\":\"pw-123456\"}}{}",
        "[".repeat(80),
        "]".repeat(80)
    );
    let converted = post(Some("application/json"), &deep);
    assert_eq!(converted.request.body, Body::None);
    assert_eq!(converted.body_missing, Some(BodyMissing::Unchecked));
}

#[test]
fn a_body_the_view_could_not_hand_over_is_said() {
    let converted = convert(&Captured {
        method: "PUT".into(),
        url: "https://api.example.test/v1/items/7".into(),
        headers: vec![("Content-Type".into(), "application/json".into())],
        body: None,
        body_unread: true,
    })
    .expect("converts");
    assert_eq!(converted.request.body, Body::None);
    assert_eq!(converted.body_missing, Some(BodyMissing::Unread));
    assert_eq!(BodyMissing::Unread.id(), "unread");
    assert_eq!(BodyMissing::Unchecked.id(), "unchecked");
    assert_eq!(BodyMissing::TooLong.id(), "tooLong");
}

#[test]
fn whether_a_request_carried_a_body() {
    use crate::api_client::capture::body_expected;
    let headers = |pairs: &[(&str, &str)]| -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect()
    };
    assert!(body_expected("POST", &headers(&[])));
    assert!(body_expected(
        "PATCH",
        &headers(&[("Content-Length", "12")])
    ));
    assert!(!body_expected("POST", &headers(&[("content-length", "0")])));
    assert!(body_expected(
        "DELETE",
        &headers(&[("Content-Type", "application/json")])
    ));
    assert!(!body_expected("DELETE", &headers(&[])));
}
