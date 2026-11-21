use super::*;
use serde_json::json;

#[test]
fn a_request_reads_from_the_panels_json_with_defaults() {
    let request: ApiRequest = serde_json::from_value(json!({
        "method": "POST",
        "url": "{{baseUrl}}/users",
        "headers": [{ "name": "Accept", "value": "application/json" }],
        "body": { "kind": "json", "text": "{\"a\":1}" },
        "auth": { "type": "bearer", "token": "{{token}}" }
    }))
    .expect("reads");
    assert_eq!(request.headers[0], Entry::new("Accept", "application/json"));
    assert_eq!(request.body.kind(), "json");
    assert_eq!(request.auth.kind(), "bearer");
    assert_eq!(request.options, Options::default());
    assert!(request.check().is_ok());
}

#[test]
fn every_body_and_auth_kind_round_trips_through_json() {
    let bodies = [
        Body::None,
        Body::Json { text: "{}".into() },
        Body::Form {
            fields: vec![Entry::new("a", "1")],
        },
        Body::Multipart {
            parts: vec![Part {
                name: "logo".into(),
                value: String::new(),
                file: Some("./logo.png".into()),
                content_type: Some("image/png".into()),
            }],
        },
        Body::Raw { text: "x".into() },
        Body::Binary {
            path: "./a.bin".into(),
        },
        Body::GraphQl {
            query: "{ me { id } }".into(),
            variables: String::new(),
        },
    ];
    for body in bodies {
        let text = serde_json::to_string(&body).expect("writes");
        assert_eq!(serde_json::from_str::<Body>(&text).expect("reads"), body);
    }
    let auth = Auth::OAuth2 {
        grant: OAuthGrant::AuthorizationCode,
        token_url: "https://id.example.com/token".into(),
        auth_url: "https://id.example.com/auth".into(),
        client_id: "app".into(),
        client_secret: String::new(),
        scope: "read".into(),
        redirect_uri: String::new(),
    };
    let text = serde_json::to_value(&auth).expect("writes");
    assert_eq!(text["type"], "oauth2");
    assert_eq!(text["grant"], "authorizationCode");
    assert_eq!(text["tokenUrl"], "https://id.example.com/token");
    assert_eq!(text["clientId"], "app");
    assert_eq!(serde_json::from_value::<Auth>(text).expect("reads"), auth);
    let key = serde_json::to_value(Auth::ApiKey {
        name: "api_key".into(),
        value: "{{key}}".into(),
        place: KeyPlace::Query,
    })
    .expect("writes");
    assert_eq!(key["in"], "query");
}

#[test]
fn an_unknown_field_is_refused() {
    let read = serde_json::from_value::<ApiRequest>(json!({
        "method": "GET", "url": "https://a.example", "script": "pm.test()"
    }));
    assert!(read.is_err());
}

#[test]
fn the_check_refuses_what_a_file_or_a_sender_cannot_hold() {
    let ok = ApiRequest::new("GET", "https://a.example/");
    assert!(ok.check().is_ok());
    assert_eq!(
        ApiRequest::new("get", "https://a.example").check(),
        Err(Invalid::Method)
    );
    assert_eq!(
        ApiRequest::new("GE T", "https://a.example").check(),
        Err(Invalid::Method)
    );
    assert_eq!(ApiRequest::new("GET", " ").check(), Err(Invalid::Url));
    assert_eq!(
        ApiRequest::new("GET", "https://a.example/\nHost: b").check(),
        Err(Invalid::Url)
    );
    let mut header = ok.clone();
    header.headers.push(Entry::new("X-A", "1\r\nX-B: 2"));
    assert_eq!(header.check(), Err(Invalid::Header("X-A".into())));
    let mut name = ok.clone();
    name.headers.push(Entry::new("Bad Name", "1"));
    assert!(matches!(name.check(), Err(Invalid::Header(_))));
    let mut timeout = ok.clone();
    timeout.options.timeout_ms = 0;
    assert_eq!(timeout.check(), Err(Invalid::Timeout));
    let mut large = ok;
    large.body = Body::Raw {
        text: "x".repeat(MAX_BODY_BYTES + 1),
    };
    assert_eq!(large.check(), Err(Invalid::BodyTooLarge));
}

#[test]
fn a_header_is_found_case_insensitively_and_only_when_enabled() {
    let mut request = ApiRequest::new("GET", "https://a.example");
    request.headers.push(Entry {
        name: "Content-Type".into(),
        value: "text/plain".into(),
        enabled: false,
    });
    request
        .headers
        .push(Entry::new("content-type", "application/json"));
    assert_eq!(request.header("CONTENT-TYPE"), Some("application/json"));
    assert_eq!(request.header("Accept"), None);
}

#[test]
fn the_default_content_type_follows_the_body() {
    assert_eq!(
        Body::Json { text: "{}".into() }.default_content_type(),
        Some("application/json")
    );
    assert_eq!(
        Body::Form { fields: Vec::new() }.default_content_type(),
        Some("application/x-www-form-urlencoded")
    );
    assert_eq!(
        Body::Raw {
            text: String::new()
        }
        .default_content_type(),
        None
    );
}

/// The w31 security review: a text part that would read back as a file include is refused, so
/// no store or export writes one.
#[test]
fn a_text_part_that_reads_as_a_file_include_is_refused() {
    let mut request = ApiRequest::new("POST", "https://api.example.test/upload");
    request.body = Body::Multipart {
        parts: vec![Part {
            name: "note".into(),
            value: "< ../../.env".into(),
            file: None,
            content_type: None,
        }],
    };
    assert_eq!(request.check(), Err(Invalid::Field("note".into())));
    request.body = Body::Multipart {
        parts: vec![Part {
            name: "logo".into(),
            value: String::new(),
            file: Some("./logo.png".into()),
            content_type: None,
        }],
    };
    assert!(request.check().is_ok());
}
