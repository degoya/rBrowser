use rb_api_model::{ApiRequest, Auth, Body, Entry};

use super::*;

#[test]
fn names_of_secrets_are_told_from_their_neighbours() {
    for name in [
        "Authorization",
        "X-Api-Key",
        "api_key",
        "access_token",
        "client_secret",
        "password",
        "Cookie",
        "key",
        "X-CSRF-Token",
    ] {
        assert!(is_secret_name(name), "{name}");
    }
    for name in [
        "tokenUrl",
        "token_type",
        "username",
        "client_id",
        "Accept",
        "auth_url",
        "author",
        "Content-Type",
    ] {
        assert!(!is_secret_name(name), "{name}");
    }
}

#[test]
fn a_template_is_no_secret() {
    assert!(is_template("{{token}}"));
    assert!(is_template(" {{a}}{{ b }} "));
    assert!(is_template("{{$secret token}}"));
    assert!(!is_template("abc{{x}}"));
    assert!(!is_template("s3cr3t"));
    assert!(!is_template(""));
}

#[test]
fn a_placeholder_names_the_variable_and_never_the_value() {
    let mut secrets = Secrets::default();
    assert_eq!(
        secrets.value("Login", "X Api.Key", "abc"),
        "{{$secret x-api-key}}"
    );
    assert_eq!(
        secrets.value("Login", "X Api.Key", "abc"),
        "{{$secret x-api-key}}"
    );
    assert_eq!(
        secrets.value("Other", "X Api.Key", "different"),
        "{{$secret x-api-key-2}}"
    );
    assert_eq!(secrets.value("Login", "token", "{{token}}"), "{{token}}");
    assert_eq!(secrets.value("Login", "token", ""), "");
    let notes = secrets.notes();
    assert_eq!(notes.len(), 2);
    assert_eq!(notes[0].name, "x-api-key");
    assert_eq!(notes[0].variable, "RB_SECRET_X_API_KEY");
    assert_eq!(notes[1].variable, "RB_SECRET_X_API_KEY_2");
    assert!(!format!("{secrets:?}").contains("abc"));
}

#[test]
fn a_request_loses_every_literal_secret() {
    let mut request = ApiRequest::new(
        "POST",
        "https://ada:hunter2@api.example.com/x?page=1&api_key=k3y&token={{token}}",
    );
    request
        .headers
        .push(Entry::new("Authorization", "Bearer abc.def"));
    request.headers.push(Entry::new("Cookie", "sid=1"));
    request
        .headers
        .push(Entry::new("Accept", "application/json"));
    request.auth = Auth::Basic {
        username: "ada".into(),
        password: "pw".into(),
    };
    request.body = Body::Json {
        text: r#"{"user":"ada","password":"pw2","nested":{"client_secret":"cs"}}"#.into(),
    };
    let mut secrets = Secrets::default();
    secrets.scrub(&mut request, "Login");
    assert_eq!(
        request.url,
        "https://ada:{{password}}@api.example.com/x?page=1&api_key={{api-key}}&token={{token}}"
    );
    assert_eq!(secrets.take_url_slots(), ["password", "api-key"]);
    assert!(secrets.take_url_slots().is_empty());
    assert_eq!(request.headers[0].value, "Bearer {{$secret token}}");
    assert_eq!(request.headers[1].value, "{{$secret cookie}}");
    assert_eq!(request.headers[2].value, "application/json");
    assert_eq!(
        request.auth,
        Auth::Basic {
            username: "ada".into(),
            password: "{{$secret password-2}}".into()
        }
    );
    let Body::Json { text } = &request.body else {
        panic!("json body")
    };
    assert!(!text.contains("pw2") && !text.contains("\"cs\""), "{text}");
    assert!(text.contains("{{$secret client-secret}}"), "{text}");
    let names: Vec<&str> = secrets.notes().iter().map(|n| n.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "password",
            "api-key",
            "token",
            "cookie",
            "password-2",
            "password-3",
            "client-secret"
        ]
    );
}

#[test]
fn form_fields_and_oauth_secrets_become_placeholders() {
    let mut request = ApiRequest::new("POST", "https://a.example/token");
    request.body = Body::Form {
        fields: vec![
            Entry::new("grant_type", "password"),
            Entry::new("password", "pw"),
        ],
    };
    request.auth = Auth::OAuth2 {
        grant: rb_api_model::OAuthGrant::ClientCredentials,
        token_url: "https://a.example/token".into(),
        auth_url: String::new(),
        client_id: "app".into(),
        client_secret: "shh".into(),
        scope: String::new(),
        redirect_uri: String::new(),
    };
    let mut secrets = Secrets::default();
    secrets.scrub(&mut request, "Token");
    let Body::Form { fields } = &request.body else {
        panic!("form")
    };
    assert_eq!(fields[0].value, "password");
    assert_eq!(fields[1].value, "{{$secret password}}");
    let Auth::OAuth2 { client_secret, .. } = &request.auth else {
        panic!("oauth2")
    };
    assert_eq!(client_secret, "{{$secret client-secret}}");
}

#[test]
fn a_secret_without_its_value_gets_a_slot() {
    let mut secrets = Secrets::default();
    assert_eq!(
        secrets.slot("environment Local", "apiSecret"),
        "{{$secret apisecret}}"
    );
    assert_eq!(secrets.notes()[0].variable, "RB_SECRET_APISECRET");
}

#[test]
fn a_placeholder_name_starts_as_a_variables_name() {
    assert_eq!(placeholder_name("2fa code"), "secret-2fa-code");
    assert_eq!(placeholder_name("--"), "secret");
}
