use rb_api_model::{Entry, EnvFile};
use serde_json::json;

use super::*;

fn env(name: &str) -> Option<String> {
    match name {
        "RB_SECRET_TOKEN" => Some("from-env".to_owned()),
        "HOME" => Some("/home/someone".to_owned()),
        _ => None,
    }
}

fn layers() -> Layers {
    Layers {
        public: EnvFile::from_json(&json!({ "dev": { "user": "ada" } })).expect("reads"),
        private: EnvFile::from_json(&json!({ "dev": { "password": "pw-1" } })).expect("reads"),
        environment: Some("dev".into()),
        base_url: Some("http://localhost:3000".into()),
    }
}

#[test]
fn every_field_is_resolved_and_the_secrets_named() {
    let reader: &EnvReader = &env;
    let variables = Variables {
        layers: layers(),
        env: reader,
        now: 1_700_000_000,
    };
    let mut request = ApiRequest::new("POST", "{{baseUrl}}/login?t={{$timestamp}}");
    request
        .headers
        .push(Entry::new("X-Token", "{{$processEnv RB_SECRET_TOKEN}}"));
    request.body = Body::Form {
        fields: vec![Entry::new("user", "{{user}}")],
    };
    request.auth = Auth::Basic {
        username: "{{user}}".into(),
        password: "{{password}}".into(),
    };
    let (resolved, secrets) = resolve(&request, &variables).expect("resolves");
    assert_eq!(resolved.url, "http://localhost:3000/login?t=1700000000");
    assert_eq!(resolved.headers[0].value, "from-env");
    assert_eq!(
        resolved.auth,
        Auth::Basic {
            username: "ada".into(),
            password: "pw-1".into()
        }
    );
    assert_eq!(secrets, ["from-env", "pw-1"]);
}

#[test]
fn only_rb_secret_variables_of_the_process_are_read() {
    let reader: &EnvReader = &env;
    let variables = Variables {
        layers: Layers::default(),
        env: reader,
        now: 0,
    };
    let request = ApiRequest::new("GET", "https://a.example/{{$processEnv HOME}}/{{missing}}");
    let missing = resolve(&request, &variables).expect_err("refused");
    assert_eq!(missing.names, ["$processEnv", "missing"]);
    let secret = ApiRequest::new("GET", "https://a.example/{{$secret token}}");
    let (resolved, secrets) = resolve(&secret, &variables).expect("resolves");
    assert_eq!(resolved.url, "https://a.example/from-env");
    assert_eq!(secrets, ["from-env"]);
}

#[test]
fn the_secret_variable_name_is_upper_case_under_the_prefix() {
    assert_eq!(
        secret_variable("api-key.v2"),
        Some("RB_SECRET_API_KEY_V2".into())
    );
    assert_eq!(secret_variable("a b"), None);
    assert_eq!(secret_variable(""), None);
    let reader: &EnvReader = &env;
    let variables = Variables {
        layers: Layers::default(),
        env: reader,
        now: 0,
    };
    let id = variables.dynamic("$guid", "").expect("a guid").text;
    assert_eq!(id.len(), 36);
    assert_eq!(id.chars().nth(14), Some('4'));
}

#[test]
fn a_secret_reads_the_private_values_first_then_the_process() {
    let reader: &EnvReader = &env;
    let mut layers = layers();
    layers.private = EnvFile::from_json(&json!({
        "dev": { "password": "pw-1" },
        "$shared": { "token": "shared-token", "password": "shared-pw" }
    }))
    .expect("reads");
    let variables = Variables {
        layers,
        env: reader,
        now: 0,
    };
    let request = ApiRequest::new(
        "GET",
        "https://a.example/{{$secret password}}/{{$secret token}}",
    );
    let (resolved, secrets) = resolve(&request, &variables).expect("resolves");
    // The environment's private value, then `$shared`'s - both before RB_SECRET_TOKEN.
    assert_eq!(resolved.url, "https://a.example/pw-1/shared-token");
    assert_eq!(secrets, ["pw-1", "shared-token"]);
    assert_eq!(
        private_secret(&variables.layers, "password").as_deref(),
        Some("pw-1")
    );
    assert_eq!(private_secret(&variables.layers, "9 bad"), None);
    let missing = ApiRequest::new("GET", "https://a.example/{{$secret nobody}}");
    assert!(resolve(&missing, &variables).is_err());
}
