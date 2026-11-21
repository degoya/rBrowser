//! A request as a Playwright test (`@playwright/test`, its `request` fixture), the form the
//! flows run: `request.fetch(url, options)` with method, headers, `data` (a text, or a file's
//! bytes), `form`, `multipart` (a file as `{ name, mimeType, buffer }`), `maxRedirects: 0` when
//! redirects are not followed, `ignoreHTTPSErrors` without the TLS check and `timeout` for one
//! other than the default; the test expects an `ok()` answer. Basic auth is its header;
//! digest auth is noted, as is a form or multipart name given twice (an object keeps one).

use rb_api_model::request::DEFAULT_TIMEOUT_MS;
use rb_api_model::{ApiRequest, Body};

use crate::Note;
use crate::fetch::header_entries;
use crate::js::{lit, object, repeats, string};
use crate::shape::{file_name_of, graphql_payload, shape};
use crate::word::Words;

const READ_FILE: &str = "import { readFileSync } from 'node:fs';";

/// `request` as a Playwright test.
pub(crate) fn code(request: &ApiRequest, words: &mut Words, notes: &mut Vec<Note>) -> String {
    let shape = shape(request, notes);
    let mut imports = vec!["import { test, expect } from '@playwright/test';"];
    let mut options: Vec<(String, String)> = Vec::new();
    if request.method != "GET" {
        options.push(("method".into(), lit(&request.method)));
    }
    let headers = header_entries(&shape, words, notes);
    if !headers.is_empty() {
        options.push(("headers".into(), object(&headers, 4)));
    }
    match &request.body {
        Body::None => {}
        Body::Json { text } | Body::Raw { text } => {
            options.push(("data".into(), string(&words.of(text))));
        }
        Body::GraphQl { query, variables } => options.push((
            "data".into(),
            string(&words.of(&graphql_payload(query, variables))),
        )),
        Body::Form { fields } => {
            let enabled: Vec<_> = fields.iter().filter(|field| field.enabled).collect();
            if repeats(enabled.iter().map(|field| field.name.as_str())) {
                notes.push(Note::DuplicateNames);
            }
            let entries: Vec<(String, String)> = enabled
                .iter()
                .map(|field| {
                    (
                        string(&words.of(&field.name)),
                        string(&words.of(&field.value)),
                    )
                })
                .collect();
            options.push(("form".into(), object(&entries, 4)));
        }
        Body::Multipart { parts } => {
            if repeats(parts.iter().map(|part| part.name.as_str())) {
                notes.push(Note::DuplicateNames);
            }
            let mut entries = Vec::new();
            for part in parts {
                let value = match &part.file {
                    Some(file) => {
                        imports.push(READ_FILE);
                        let mime = part
                            .content_type
                            .as_deref()
                            .unwrap_or("application/octet-stream");
                        format!(
                            "{{ name: {}, mimeType: {}, buffer: readFileSync({}) }}",
                            lit(file_name_of(file)),
                            string(&words.of(mime)),
                            string(&words.of(file))
                        )
                    }
                    None => string(&words.of(&part.value)),
                };
                entries.push((string(&words.of(&part.name)), value));
            }
            options.push(("multipart".into(), object(&entries, 4)));
        }
        Body::Binary { path } => {
            imports.push(READ_FILE);
            options.push((
                "data".into(),
                format!("readFileSync({})", string(&words.of(path))),
            ));
        }
    }
    if !request.options.follow_redirects {
        options.push(("maxRedirects".into(), "0".into()));
    }
    if !request.options.verify_tls {
        options.push(("ignoreHTTPSErrors".into(), "true".into()));
    }
    if request.options.timeout_ms != DEFAULT_TIMEOUT_MS {
        options.push(("timeout".into(), request.options.timeout_ms.to_string()));
    }
    imports.dedup();
    let title = if request.name.trim().is_empty() {
        format!("{} {}", request.method, request.url.trim())
    } else {
        request.name.trim().to_owned()
    };
    let options = if options.is_empty() {
        String::new()
    } else {
        format!(", {}", object(&options, 2))
    };
    format!(
        "{}\n\ntest({}, async ({{ request }}) => {{\n  const response = await request.fetch({}{options});\n  expect(response.ok()).toBeTruthy();\n}});\n",
        imports.join("\n"),
        lit(&title),
        string(&words.of(&shape.url)),
    )
}

#[cfg(test)]
#[path = "playwright_tests.rs"]
mod tests;
