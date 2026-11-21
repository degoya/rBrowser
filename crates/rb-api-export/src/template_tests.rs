use super::*;
use rb_api_model::vars::Value;
use rb_api_model::{ApiRequest, Auth, Body, Entry};

/// `baseUrl` and `user` plain, `token` secret, `header` plain with the secret inside;
/// `$timestamp` known, every process variable unknown (the code op reads none).
struct Fake;

impl Lookup for Fake {
    fn variable(&self, name: &str) -> Option<Value> {
        match name {
            "baseUrl" => Some(Value::plain("https://api.example.com")),
            "user" => Some(Value::plain("ada")),
            "token" => Some(Value::secret("tok-secret-1")),
            "header" => Some(Value::plain("Bearer {{token}}")),
            "loop" => Some(Value::plain("{{loop}}")),
            _ => None,
        }
    }

    fn dynamic(&self, name: &str, _args: &str) -> Option<Value> {
        match name {
            "$timestamp" => Some(Value::plain("1700000000")),
            "$processEnv" | "$secret" => Some(Value::secret("from-the-process")),
            _ => None,
        }
    }
}

#[test]
fn pieces_split_variables_dynamic_ones_and_text() {
    let found = pieces("a{{x}}b{{$processEnv RB_SECRET_X}}{{ 1 + 1 }}{{");
    let kinds: Vec<String> = found
        .iter()
        .map(|piece| match piece {
            Piece::Text(text) => format!("T({text})"),
            Piece::Var { raw, name } => format!("V({raw},{name})"),
            Piece::Dynamic { raw, name, args } => format!("D({raw},{name},{args})"),
        })
        .collect();
    assert_eq!(
        kinds,
        [
            "T(a)",
            "V({{x}},x)",
            "T(b)",
            "D({{$processEnv RB_SECRET_X}},$processEnv,RB_SECRET_X)",
            "T({{)",
            "T( 1 + 1 }}{{)",
        ]
    );
}

#[test]
fn fill_puts_plain_values_in_and_keeps_every_secret_a_variable() {
    assert_eq!(
        fill_text("{{baseUrl}}/users/{{user}}?t={{$timestamp}}", &Fake),
        "https://api.example.com/users/ada?t=1700000000"
    );
    assert_eq!(fill_text("Bearer {{token}}", &Fake), "Bearer {{token}}");
    // A plain value holding a secret: the plain part in, the secret still a variable.
    assert_eq!(fill_text("{{header}}", &Fake), "Bearer {{token}}");
    // Process variables are never read, even when the lookup would answer.
    assert_eq!(
        fill_text("{{$processEnv RB_SECRET_TOKEN}} {{$secret api}}", &Fake),
        "{{$processEnv RB_SECRET_TOKEN}} {{$secret api}}"
    );
    assert_eq!(fill_text("{{unknown}}", &Fake), "{{unknown}}");
    // A value naming itself ends at the depth limit instead of looping.
    assert_eq!(fill_text("{{loop}}", &Fake), "{{loop}}");
}

#[test]
fn fill_covers_every_template_of_a_request() {
    let mut request = ApiRequest::new("POST", "{{baseUrl}}/login");
    request.headers = vec![Entry::new("X-User", "{{user}}")];
    request.body = Body::Form {
        fields: vec![
            Entry::new("user", "{{user}}"),
            Entry::new("pw", "{{token}}"),
        ],
    };
    request.auth = Auth::Basic {
        username: "{{user}}".into(),
        password: "{{token}}".into(),
    };
    let filled = fill(&request, &Fake);
    assert_eq!(filled.url, "https://api.example.com/login");
    assert_eq!(filled.headers[0].value, "ada");
    let Body::Form { fields } = &filled.body else {
        panic!("a form");
    };
    assert_eq!(fields[0].value, "ada");
    assert_eq!(fields[1].value, "{{token}}");
    assert_eq!(
        filled.auth,
        Auth::Basic {
            username: "ada".into(),
            password: "{{token}}".into()
        }
    );
}

#[test]
fn env_names_follow_the_shells_rules() {
    let names: Vec<Option<String>> = pieces(
        "{{api-key.v2}}{{$processEnv RB_SECRET_TOKEN}}{{$processEnv bad name}}{{$secret my-key}}{{$guid}}",
    )
    .iter()
    .map(env_name)
    .collect();
    assert_eq!(
        names,
        [
            Some("api_key_v2".to_owned()),
            Some("RB_SECRET_TOKEN".to_owned()),
            None,
            Some("RB_SECRET_MY_KEY".to_owned()),
            None,
        ]
    );
}

#[test]
fn encode_keeps_variables_and_encodes_the_text_around_them() {
    assert_eq!(encode("a b&{{key}}/\u{e9}"), "a%20b%26{{key}}%2F%C3%A9");
}
