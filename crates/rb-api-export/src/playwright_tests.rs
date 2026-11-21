use super::*;
use crate::fixtures::create_user;
use crate::{Language, Options, Shell, Variables};
use rb_api_model::{Entry, Part};

fn playwright(request: &ApiRequest) -> crate::Code {
    crate::code(
        request,
        Language::Playwright,
        Options {
            shell: Shell::Posix,
            variables: Variables::Environment,
        },
    )
}

#[test]
fn a_json_post_as_a_test() {
    let out = playwright(&create_user());
    assert_eq!(
        out.text,
        "import { test, expect } from '@playwright/test';

test('Create user', async ({ request }) => {
  const response = await request.fetch('https://api.example.com/users', {
    method: 'POST',
    headers: {
      'Accept': 'application/json',
      'Authorization': `Bearer ${process.env.token}`,
      'Content-Type': 'application/json',
    },
    data: '{\"name\": \"Ada\"}',
  });
  expect(response.ok()).toBeTruthy();
});
"
    );
    assert_eq!(out.env, ["token"]);
}

#[test]
fn an_untitled_get_with_the_options() {
    let mut request = ApiRequest::new("GET", "https://x/");
    request.options.follow_redirects = false;
    request.options.verify_tls = false;
    request.options.timeout_ms = 1000;
    let out = playwright(&request);
    assert_eq!(
        out.text,
        "import { test, expect } from '@playwright/test';

test('GET https://x/', async ({ request }) => {
  const response = await request.fetch('https://x/', {
    maxRedirects: 0,
    ignoreHTTPSErrors: true,
    timeout: 1000,
  });
  expect(response.ok()).toBeTruthy();
});
"
    );
    assert!(out.notes.is_empty());
}

#[test]
fn forms_multipart_and_binary_bodies() {
    let mut request = ApiRequest::new("POST", "https://x/");
    request.body = Body::Form {
        fields: vec![Entry::new("a", "1"), Entry::new("a", "2")],
    };
    let out = playwright(&request);
    assert!(
        out.text
            .contains("    form: {\n      'a': '1',\n      'a': '2',\n    },\n"),
        "{}",
        out.text
    );
    assert_eq!(out.notes, [Note::DuplicateNames]);
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
                content_type: None,
            },
        ],
    };
    let out = playwright(&request);
    assert!(out.text.starts_with(
        "import { test, expect } from '@playwright/test';\nimport { readFileSync } from 'node:fs';\n\n"
    ));
    assert!(out.text.contains(
        "    multipart: {\n      'title': 'Logo',\n      'logo': { name: 'logo.png', mimeType: 'application/octet-stream', buffer: readFileSync('./logo.png') },\n    },\n"
    ), "{}", out.text);
    request.body = Body::Binary {
        path: "./a.bin".into(),
    };
    assert!(
        playwright(&request)
            .text
            .contains("    data: readFileSync('./a.bin'),\n")
    );
}
