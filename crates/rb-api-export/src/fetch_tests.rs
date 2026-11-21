use super::*;
use crate::fixtures::create_user;
use crate::{Language, Options, Shell, Variables};
use rb_api_model::{Auth, Entry, Part};

fn fetch(request: &ApiRequest, variables: Variables) -> crate::Code {
    crate::code(
        request,
        Language::Fetch,
        Options {
            shell: Shell::Posix,
            variables,
        },
    )
}

#[test]
fn a_json_post_with_a_bearer_token() {
    let out = fetch(&create_user(), Variables::Environment);
    assert_eq!(
        out.text,
        "const response = await fetch('https://api.example.com/users', {
  method: 'POST',
  headers: {
    'Accept': 'application/json',
    'Authorization': `Bearer ${process.env.token}`,
    'Content-Type': 'application/json',
  },
  body: '{\"name\": \"Ada\"}',
});
console.log(response.status, await response.text());
"
    );
    assert_eq!(out.env, ["token"]);
    let out = fetch(&create_user(), Variables::Template);
    assert!(out.text.contains("'Authorization': 'Bearer {{token}}',"));
}

#[test]
fn a_plain_get_is_one_call() {
    let out = fetch(
        &ApiRequest::new("GET", "https://x/"),
        Variables::Environment,
    );
    assert_eq!(
        out.text,
        "const response = await fetch('https://x/');\nconsole.log(response.status, await response.text());\n"
    );
}

#[test]
fn a_form_with_basic_auth_and_the_options() {
    let mut request = ApiRequest::new("POST", "https://x/login");
    request.options.follow_redirects = false;
    request.options.verify_tls = false;
    request.options.timeout_ms = 5000;
    request.auth = Auth::Basic {
        username: "ada".into(),
        password: "{{password}}".into(),
    };
    request.body = Body::Form {
        fields: vec![Entry::new("a", "1"), Entry::new("a", "2")],
    };
    let out = fetch(&request, Variables::Environment);
    assert_eq!(
        out.text,
        "const response = await fetch('https://x/login', {
  method: 'POST',
  headers: {
    'Authorization': 'Basic ' + btoa(`ada:${process.env.password}`),
  },
  body: new URLSearchParams([
    ['a', '1'],
    ['a', '2'],
  ]),
  redirect: 'manual',
  signal: AbortSignal.timeout(5000),
});
console.log(response.status, await response.text());
"
    );
    assert_eq!(out.notes, [Note::Insecure]);
}

#[test]
fn multipart_reads_its_files_with_open_as_blob() {
    let mut request = ApiRequest::new("POST", "https://x/up");
    request.body = Body::Multipart {
        parts: vec![
            Part {
                name: "title".into(),
                value: "Logo".into(),
                file: None,
                content_type: None,
            },
            Part {
                name: "meta".into(),
                value: "{}".into(),
                file: None,
                content_type: Some("application/json".into()),
            },
            Part {
                name: "logo".into(),
                value: String::new(),
                file: Some("./img/logo.png".into()),
                content_type: Some("image/png".into()),
            },
        ],
    };
    let out = fetch(&request, Variables::Environment);
    assert_eq!(
        out.text,
        "import { openAsBlob } from 'node:fs';

const form = new FormData();
form.append('title', 'Logo');
form.append('meta', new Blob(['{}'], { type: 'application/json' }));
form.append('logo', await openAsBlob('./img/logo.png', { type: 'image/png' }), 'logo.png');

const response = await fetch('https://x/up', {
  method: 'POST',
  body: form,
});
console.log(response.status, await response.text());
"
    );
}

#[test]
fn digest_and_repeated_headers_are_noted() {
    let mut request = ApiRequest::new("GET", "https://x/");
    request.headers = vec![Entry::new("Accept", "a"), Entry::new("accept", "b")];
    request.auth = Auth::Digest {
        username: "u".into(),
        password: "p".into(),
    };
    let out = fetch(&request, Variables::Environment);
    assert_eq!(out.notes, [Note::Digest, Note::DuplicateNames]);
}
