use super::*;
use crate::fixtures::create_user;
use crate::{Language, Options, Variables, code};
use rb_api_model::{Auth, Entry, Part};

fn httpie(request: &ApiRequest, shell: Shell) -> crate::Code {
    code(
        request,
        Language::Httpie,
        Options {
            shell,
            variables: Variables::Environment,
        },
    )
}

#[test]
fn a_json_post_with_a_bearer_token() {
    let out = httpie(&create_user(), Shell::Posix);
    assert_eq!(
        out.text,
        "http --follow POST https://api.example.com/users \\
  Accept:application/json \\
  'Authorization:Bearer '\"${token}\" \\
  Content-Type:application/json \\
  --raw '{\"name\": \"Ada\"}'"
    );
    assert_eq!(out.env, ["token"]);
}

#[test]
fn a_form_with_options_and_digest() {
    let mut request = ApiRequest::new("POST", "https://x/login");
    request.options.follow_redirects = false;
    request.options.verify_tls = false;
    request.options.timeout_ms = 5000;
    request.auth = Auth::Digest {
        username: "ada".into(),
        password: "{{password}}".into(),
    };
    request.body = Body::Form {
        fields: vec![Entry::new("a:b", "1"), Entry::new("empty", "")],
    };
    let out = httpie(&request, Shell::PowerShell);
    assert_eq!(
        out.text,
        "http --verify=no --timeout=5 --form POST https://x/login `
  'a\\:b=1' `
  empty= `
  --auth-type=digest --auth \"ada:${env:password}\""
    );
}

#[test]
fn multipart_files_binary_bodies_and_empty_headers() {
    let mut request = ApiRequest::new("PUT", "https://x/up");
    request.headers = vec![Entry::new("X-Empty", "")];
    request.body = Body::Multipart {
        parts: vec![
            Part {
                name: "title".into(),
                value: "Logo".into(),
                file: None,
                content_type: None,
            },
            Part {
                name: "logo".into(),
                value: String::new(),
                file: Some("./logo.png".into()),
                content_type: Some("image/png".into()),
            },
        ],
    };
    let out = httpie(&request, Shell::Posix);
    assert_eq!(
        out.text,
        "http --follow --multipart PUT https://x/up \\
  'X-Empty;' \\
  title=Logo \\
  'logo@./logo.png;type=image/png'"
    );
    request.body = Body::Binary {
        path: "./a.bin".into(),
    };
    let out = httpie(&request, Shell::Posix);
    assert!(
        out.text
            .ends_with("  'X-Empty;' \\\n  Content-Type: \\\n  @./a.bin"),
        "{}",
        out.text
    );
}
