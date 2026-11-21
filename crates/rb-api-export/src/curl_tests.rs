use super::*;
use crate::fixtures::create_user;
use crate::{Language, Options, Variables, code};
use rb_api_model::{Auth, Entry, KeyPlace, Part};

fn curl(request: &ApiRequest, shell: Shell, variables: Variables) -> crate::Code {
    code(request, Language::Curl, Options { shell, variables })
}

#[test]
fn posix_with_the_secret_as_an_environment_variable() {
    let out = curl(&create_user(), Shell::Posix, Variables::Environment);
    assert_eq!(
        out.text,
        "curl --location --request POST https://api.example.com/users \\
  --header 'Accept: application/json' \\
  --header 'Authorization: Bearer '\"${token}\" \\
  --header 'Content-Type: application/json' \\
  --data-raw '{\"name\": \"Ada\"}'"
    );
    assert_eq!(out.env, ["token"]);
    assert!(out.notes.is_empty());
}

#[test]
fn powershell_calls_curl_exe() {
    let out = curl(&create_user(), Shell::PowerShell, Variables::Environment);
    assert_eq!(
        out.text,
        "curl.exe --location --request POST https://api.example.com/users `
  --header 'Accept: application/json' `
  --header \"Authorization: Bearer ${env:token}\" `
  --header 'Content-Type: application/json' `
  --data-raw '{\"name\": \"Ada\"}'"
    );
}

#[test]
fn cmd_escapes_the_quotes_of_the_body() {
    let out = curl(&create_user(), Shell::Cmd, Variables::Environment);
    assert_eq!(
        out.text,
        "curl --location --request POST https://api.example.com/users ^
  --header ^\"Accept: application/json^\" ^
  --header ^\"Authorization: Bearer %token%^\" ^
  --header ^\"Content-Type: application/json^\" ^
  --data-raw ^\"{\\^\"name\\^\": \\^\"Ada\\^\"}^\""
    );
}

#[test]
fn templates_stay_as_written_and_curl_globs_nothing() {
    let mut request = create_user();
    request.url = "{{baseUrl}}/users".into();
    let out = curl(&request, Shell::Posix, Variables::Template);
    assert!(
        out.text
            .starts_with("curl --globoff --location --request POST '{{baseUrl}}/users' \\\n"),
        "{}",
        out.text
    );
    assert!(
        out.text
            .contains("--header 'Authorization: Bearer {{token}}'")
    );
    assert!(out.env.is_empty());
}

#[test]
fn options_basic_auth_and_a_form() {
    let mut request = ApiRequest::new("PUT", "https://x/login");
    request.options.follow_redirects = false;
    request.options.verify_tls = false;
    request.options.timeout_ms = 2500;
    request.auth = Auth::Basic {
        username: "ada".into(),
        password: "{{password}}".into(),
    };
    request.body = Body::Form {
        fields: vec![
            Entry::new("user name", "ada lovelace"),
            Entry {
                enabled: false,
                ..Entry::new("debug", "1")
            },
        ],
    };
    let out = curl(&request, Shell::Posix, Variables::Environment);
    assert_eq!(
        out.text,
        "curl --insecure --max-time 2.5 --request PUT https://x/login \\
  --user ada:\"${password}\" \\
  --data-urlencode 'user%20name=ada lovelace'"
    );
}

#[test]
fn digest_head_and_a_query_key() {
    let mut request = ApiRequest::new("HEAD", "https://x/a");
    request.auth = Auth::ApiKey {
        name: "key".into(),
        value: "abc".into(),
        place: KeyPlace::Query,
    };
    let out = curl(&request, Shell::Posix, Variables::Environment);
    assert_eq!(out.text, "curl --location --head 'https://x/a?key=abc'");
    request.auth = Auth::Digest {
        username: "u".into(),
        password: "p".into(),
    };
    let out = curl(&request, Shell::Posix, Variables::Environment);
    assert_eq!(
        out.text,
        "curl --location --head https://x/a \\\n  --digest --user u:p"
    );
}

#[test]
fn multipart_text_and_file_parts() {
    let mut request = ApiRequest::new("POST", "https://x/upload");
    request.body = Body::Multipart {
        parts: vec![
            Part {
                name: "title".into(),
                value: "@not-a-file".into(),
                file: None,
                content_type: None,
            },
            Part {
                name: "meta".into(),
                value: "{\"a\":1}".into(),
                file: None,
                content_type: Some("application/json".into()),
            },
            Part {
                name: "logo".into(),
                value: String::new(),
                file: Some("./img/logo;1.png".into()),
                content_type: Some("image/png".into()),
            },
        ],
    };
    let out = curl(&request, Shell::Posix, Variables::Environment);
    assert_eq!(
        out.text,
        "curl --location --request POST https://x/upload \\
  --form-string title=@not-a-file \\
  --form 'meta=\"{\\\"a\\\":1}\";type=application/json' \\
  --form 'logo=@\"./img/logo;1.png\";type=image/png'"
    );
}

#[test]
fn raw_and_binary_bodies_send_no_type_they_do_not_name() {
    let mut request = ApiRequest::new("POST", "https://x/");
    request.body = Body::Binary {
        path: "./a.bin".into(),
    };
    let out = curl(&request, Shell::Posix, Variables::Environment);
    assert_eq!(
        out.text,
        "curl --location --request POST https://x/ \\
  --header Content-Type: \\
  --data-binary @./a.bin"
    );
    request.headers = vec![Entry::new("Content-Type", "text/plain")];
    request.body = Body::Raw {
        text: "line 1\nline 2".into(),
    };
    let out = curl(&request, Shell::Cmd, Variables::Environment);
    assert!(
        out.text.ends_with("--data-raw ^\"line 1 line 2^\""),
        "{}",
        out.text
    );
    assert_eq!(out.notes, [Note::LineBreaks]);
}

#[test]
fn graphql_is_sent_as_json() {
    let mut request = ApiRequest::new("POST", "https://x/graphql");
    request.body = Body::GraphQl {
        query: "{ me { id } }".into(),
        variables: String::new(),
    };
    let out = curl(&request, Shell::Posix, Variables::Environment);
    assert!(
        out.text
            .ends_with("--data-raw '{\"query\":\"{ me { id } }\"}'"),
        "{}",
        out.text
    );
}
