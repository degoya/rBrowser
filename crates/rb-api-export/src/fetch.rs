//! A request as `fetch` for Node 20 or later (an ES module, top-level `await`): the URL and an
//! init of method, headers, body, `redirect: 'manual'` when redirects are not followed and
//! `signal: AbortSignal.timeout(ms)` for a timeout other than the default; a form as
//! `URLSearchParams`, multipart as `FormData` with the files from `openAsBlob`, a binary body
//! from `openAsBlob`; basic auth as its header (`btoa`). `fetch` has no digest auth and checks
//! every certificate - both are noted.

use rb_api_model::request::DEFAULT_TIMEOUT_MS;
use rb_api_model::{ApiRequest, Body};

use crate::Note;
use crate::js::{lit, object, repeats, string};
use crate::shape::{Credentials, file_name_of, graphql_payload, shape};
use crate::word::Words;

/// The headers of `request` as object entries; the basic credentials as `Authorization`.
pub(crate) fn header_entries(
    shape: &crate::shape::Shape,
    words: &mut Words,
    notes: &mut Vec<Note>,
) -> Vec<(String, String)> {
    let mut entries: Vec<(String, String)> = shape
        .headers
        .iter()
        .map(|(name, value)| (lit(name), string(&words.of(value))))
        .collect();
    match &shape.credentials {
        Some(Credentials::Basic(user, password)) => entries.push((
            lit("Authorization"),
            format!(
                "'Basic ' + btoa({})",
                string(&words.of(&format!("{user}:{password}")))
            ),
        )),
        Some(Credentials::Digest(..)) => notes.push(Note::Digest),
        None => {}
    }
    let names = shape.headers.iter().map(|(name, _)| name.as_str());
    let basic = matches!(shape.credentials, Some(Credentials::Basic(..)));
    if repeats(names.chain(basic.then_some("Authorization"))) {
        notes.push(Note::DuplicateNames);
    }
    entries
}

/// `request` as a `fetch` call.
pub(crate) fn code(request: &ApiRequest, words: &mut Words, notes: &mut Vec<Note>) -> String {
    let shape = shape(request, notes);
    let mut imports = Vec::new();
    let mut before: Vec<String> = Vec::new();
    let mut init: Vec<(String, String)> = Vec::new();
    if request.method != "GET" {
        init.push(("method".into(), lit(&request.method)));
    }
    let headers = header_entries(&shape, words, notes);
    if !headers.is_empty() {
        init.push(("headers".into(), object(&headers, 2)));
    }
    match &request.body {
        Body::None => {}
        Body::Json { text } | Body::Raw { text } => {
            init.push(("body".into(), string(&words.of(text))));
        }
        Body::GraphQl { query, variables } => init.push((
            "body".into(),
            string(&words.of(&graphql_payload(query, variables))),
        )),
        Body::Form { fields } => {
            let mut rows = String::new();
            for field in fields.iter().filter(|field| field.enabled) {
                rows.push_str(&format!(
                    "    [{}, {}],\n",
                    string(&words.of(&field.name)),
                    string(&words.of(&field.value))
                ));
            }
            init.push(("body".into(), format!("new URLSearchParams([\n{rows}  ])")));
        }
        Body::Multipart { parts } => {
            before.push("const form = new FormData();".into());
            for part in parts {
                let name = string(&words.of(&part.name));
                let typed = part
                    .content_type
                    .as_deref()
                    .map(|ty| format!(", {{ type: {} }}", string(&words.of(ty))));
                let value = match (&part.file, typed) {
                    (Some(file), typed) => {
                        imports.push("import { openAsBlob } from 'node:fs';");
                        format!(
                            "await openAsBlob({}{}), {}",
                            string(&words.of(file)),
                            typed.unwrap_or_default(),
                            lit(file_name_of(file))
                        )
                    }
                    (None, Some(typed)) => {
                        format!("new Blob([{}]{typed})", string(&words.of(&part.value)))
                    }
                    (None, None) => string(&words.of(&part.value)),
                };
                before.push(format!("form.append({name}, {value});"));
            }
            init.push(("body".into(), "form".into()));
        }
        Body::Binary { path } => {
            imports.push("import { openAsBlob } from 'node:fs';");
            init.push((
                "body".into(),
                format!("await openAsBlob({})", string(&words.of(path))),
            ));
        }
    }
    if !request.options.follow_redirects {
        init.push(("redirect".into(), "'manual'".into()));
    }
    if request.options.timeout_ms != DEFAULT_TIMEOUT_MS {
        init.push((
            "signal".into(),
            format!("AbortSignal.timeout({})", request.options.timeout_ms),
        ));
    }
    if !request.options.verify_tls {
        notes.push(Note::Insecure);
    }
    let mut out = String::new();
    imports.dedup();
    for import in &imports {
        out.push_str(import);
        out.push('\n');
    }
    if !imports.is_empty() {
        out.push('\n');
    }
    for line in &before {
        out.push_str(line);
        out.push('\n');
    }
    if !before.is_empty() {
        out.push('\n');
    }
    let init = if init.is_empty() {
        String::new()
    } else {
        format!(", {}", object(&init, 0))
    };
    out.push_str(&format!(
        "const response = await fetch({}{init});\n",
        string(&words.of(&shape.url))
    ));
    out.push_str("console.log(response.status, await response.text());\n");
    out
}

#[cfg(test)]
#[path = "fetch_tests.rs"]
mod tests;
