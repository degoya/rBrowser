use super::*;
use crate::fixtures::create_user;
use rb_api_model::{Entry, KeyPlace, OAuthGrant, Part};
use serde_json::json;

fn one(request: ApiRequest) -> Value {
    let mut notes = Vec::new();
    let text = collection("Shop", &[("users/create.http".into(), request)], &mut notes);
    serde_json::from_str(&text).expect("JSON")
}

#[test]
fn a_collection_with_folders_and_a_json_request() {
    let value = one(create_user());
    assert_eq!(
        value,
        json!({
            "info": {
                "name": "Shop",
                "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"
            },
            "item": [{
                "name": "users",
                "item": [{
                    "name": "Create user",
                    "request": {
                        "method": "POST",
                        "header": [
                            { "key": "Accept", "value": "application/json" },
                            { "key": "X-Debug", "value": "1", "disabled": true }
                        ],
                        "url": "https://api.example.com/users",
                        "body": {
                            "mode": "raw",
                            "raw": "{\"name\": \"Ada\"}",
                            "options": { "raw": { "language": "json" } }
                        },
                        "auth": {
                            "type": "bearer",
                            "bearer": [{ "key": "token", "value": "{{token}}", "type": "string" }]
                        }
                    }
                }]
            }]
        })
    );
}

#[test]
fn options_comments_and_the_timeout_note() {
    let mut request = ApiRequest::new("GET", "{{baseUrl}}/a");
    request.comments = vec!["first".into(), "second".into()];
    request.options.follow_redirects = false;
    request.options.verify_tls = false;
    request.options.timeout_ms = 1000;
    let mut notes = Vec::new();
    let text = collection("x", &[("a.http".into(), request)], &mut notes);
    let value: Value = serde_json::from_str(&text).expect("JSON");
    let item = &value["item"][0];
    assert_eq!(item["name"], "a");
    assert_eq!(item["request"]["description"], "first\nsecond");
    assert_eq!(item["request"]["url"], "{{baseUrl}}/a");
    assert_eq!(
        item["protocolProfileBehavior"],
        json!({ "followRedirects": false, "strictSSL": false })
    );
    assert_eq!(notes, [Note::Timeout]);
}

#[test]
fn every_body_kind() {
    let body = |body: Body| {
        let mut request = ApiRequest::new("POST", "https://x/");
        request.body = body;
        one(request)["item"][0]["item"][0]["request"]["body"].clone()
    };
    assert_eq!(
        body(Body::Form {
            fields: vec![
                Entry::new("a", "1"),
                Entry {
                    enabled: false,
                    ..Entry::new("b", "2")
                }
            ]
        }),
        json!({ "mode": "urlencoded", "urlencoded": [
            { "key": "a", "value": "1", "type": "text" },
            { "key": "b", "value": "2", "type": "text", "disabled": true }
        ]})
    );
    assert_eq!(
        body(Body::Multipart {
            parts: vec![
                Part {
                    name: "t".into(),
                    value: "v".into(),
                    file: None,
                    content_type: None
                },
                Part {
                    name: "f".into(),
                    value: String::new(),
                    file: Some("./logo.png".into()),
                    content_type: Some("image/png".into())
                },
            ]
        }),
        json!({ "mode": "formdata", "formdata": [
            { "key": "t", "value": "v", "type": "text" },
            { "key": "f", "src": "./logo.png", "type": "file", "contentType": "image/png" }
        ]})
    );
    assert_eq!(
        body(Body::Binary {
            path: "./a.bin".into()
        }),
        json!({ "mode": "file", "file": { "src": "./a.bin" } })
    );
    assert_eq!(
        body(Body::GraphQl {
            query: "{ me }".into(),
            variables: "{}".into()
        }),
        json!({ "mode": "graphql", "graphql": { "query": "{ me }", "variables": "{}" } })
    );
    assert_eq!(
        body(Body::Raw { text: "x".into() }),
        json!({ "mode": "raw", "raw": "x", "options": { "raw": { "language": "text" } } })
    );
}

#[test]
fn every_auth_kind() {
    let auth = |auth: Auth| {
        let mut request = ApiRequest::new("GET", "https://x/");
        request.auth = auth;
        one(request)["item"][0]["item"][0]["request"]["auth"].clone()
    };
    assert_eq!(
        auth(Auth::Basic {
            username: "u".into(),
            password: "{{pw}}".into()
        }),
        json!({ "type": "basic", "basic": [
            { "key": "username", "value": "u", "type": "string" },
            { "key": "password", "value": "{{pw}}", "type": "string" }
        ]})
    );
    assert_eq!(
        auth(Auth::ApiKey {
            name: "X-Key".into(),
            value: "{{key}}".into(),
            place: KeyPlace::Query
        })["apikey"],
        json!([
            { "key": "key", "value": "X-Key", "type": "string" },
            { "key": "value", "value": "{{key}}", "type": "string" },
            { "key": "in", "value": "query", "type": "string" }
        ])
    );
    let oauth = auth(Auth::OAuth2 {
        grant: OAuthGrant::AuthorizationCode,
        token_url: "https://id/token".into(),
        auth_url: "https://id/auth".into(),
        client_id: "app".into(),
        client_secret: "{{clientSecret}}".into(),
        scope: "read".into(),
        redirect_uri: String::new(),
    });
    assert_eq!(oauth["type"], "oauth2");
    let entries = oauth["oauth2"].as_array().expect("entries");
    let get = |key: &str| {
        entries
            .iter()
            .find(|entry| entry["key"] == key)
            .map(|entry| entry["value"].clone())
    };
    assert_eq!(
        get("grant_type"),
        Some(json!("authorization_code_with_pkce"))
    );
    assert_eq!(get("clientSecret"), Some(json!("{{clientSecret}}")));
    assert_eq!(get("challengeAlgorithm"), Some(json!("S256")));
    assert_eq!(get("redirect_uri"), None);
    assert_eq!(
        one(ApiRequest::new("GET", "https://x/"))["item"][0]["item"][0]["request"].get("auth"),
        None
    );
}
