use super::*;
use rb_api_model::{Entry, OAuthGrant, Part};

#[test]
fn the_shape_holds_enabled_headers_auth_and_the_json_type() {
    let mut request = ApiRequest::new("POST", " {{baseUrl}}/users ");
    request.headers = vec![
        Entry::new("Accept", "application/json"),
        Entry {
            enabled: false,
            ..Entry::new("X-Debug", "1")
        },
    ];
    request.body = Body::Json { text: "{}".into() };
    request.auth = Auth::Bearer {
        token: "{{token}}".into(),
    };
    let mut notes = Vec::new();
    let shape = shape(&request, &mut notes);
    assert_eq!(shape.url, "{{baseUrl}}/users");
    assert_eq!(
        shape.headers,
        [
            ("Accept".to_owned(), "application/json".to_owned()),
            ("Authorization".to_owned(), "Bearer {{token}}".to_owned()),
            ("Content-Type".to_owned(), "application/json".to_owned()),
        ]
    );
    assert!(notes.is_empty());
}

#[test]
fn a_query_key_goes_on_the_url_and_oauth_is_noted() {
    let mut request = ApiRequest::new("GET", "https://x/a?b=1#top");
    request.auth = Auth::ApiKey {
        name: "api key".into(),
        value: "{{key}}".into(),
        place: KeyPlace::Query,
    };
    let mut notes = Vec::new();
    assert_eq!(
        shape(&request, &mut notes).url,
        "https://x/a?b=1&api%20key={{key}}#top"
    );
    request.auth = Auth::OAuth2 {
        grant: OAuthGrant::ClientCredentials,
        token_url: "https://id/token".into(),
        auth_url: String::new(),
        client_id: "app".into(),
        client_secret: String::new(),
        scope: String::new(),
        redirect_uri: String::new(),
    };
    let _ = shape(&request, &mut notes);
    assert_eq!(notes, [Note::OAuth2]);
}

#[test]
fn a_multipart_type_of_the_request_is_left_to_the_tool() {
    let mut request = ApiRequest::new("POST", "https://x/");
    request.headers = vec![Entry::new(
        "Content-Type",
        "multipart/form-data; boundary=b",
    )];
    request.body = Body::Multipart {
        parts: vec![Part {
            name: "a".into(),
            value: "1".into(),
            file: None,
            content_type: None,
        }],
    };
    assert!(shape(&request, &mut Vec::new()).headers.is_empty());
}

#[test]
fn small_helpers() {
    assert_eq!(graphql_payload("{ me }", ""), "{\"query\":\"{ me }\"}");
    assert_eq!(
        graphql_payload("q", " {\"id\": {{id}}} "),
        "{\"query\":\"q\",\"variables\":{\"id\": {{id}}}}"
    );
    assert_eq!(file_name_of("./img/logo.png"), "logo.png");
    assert_eq!(seconds(5000), "5");
    assert_eq!(seconds(2500), "2.5");
    assert_eq!(seconds(1234), "1.234");
}

#[test]
fn the_notes_ids_are_the_panels_text_keys() {
    let ids: Vec<serde_json::Value> = [
        Note::OAuth2,
        Note::Digest,
        Note::Insecure,
        Note::LineBreaks,
        Note::DuplicateNames,
        Note::Timeout,
    ]
    .iter()
    .map(|note| serde_json::to_value(note).expect("JSON"))
    .collect();
    assert_eq!(
        ids,
        [
            "oauth2",
            "digest",
            "insecure",
            "lineBreaks",
            "duplicateNames",
            "timeout"
        ]
    );
}
