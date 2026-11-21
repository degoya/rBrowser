//! A request as an HTTPie command (`http`, 3.0 or later), for the shell of the options: the
//! options, the method and the URL on the first line, then a request item per line - a header
//! `Name:value` (`Name;` without a value, `Content-Type:` to send none), a form field
//! `name=value` (`\` before `:`, `=` and `@` of the name), a file part `name@file;type=...`, a
//! binary body `@file` - the credentials and the body: JSON, raw text and GraphQL as `--raw`,
//! a form with `--form`, multipart with `--multipart`.

use rb_api_model::request::DEFAULT_TIMEOUT_MS;
use rb_api_model::{ApiRequest, Body};

use crate::shape::{Credentials, graphql_payload, seconds, shape};
use crate::shell::Renderer;
use crate::word::{Word, Words};
use crate::{Note, Shell};

/// `request` as an HTTPie command for `shell`.
pub(crate) fn command(
    request: &ApiRequest,
    words: &mut Words,
    shell: Shell,
    notes: &mut Vec<Note>,
) -> String {
    let groups = groups(request, words, notes);
    let mut renderer = Renderer::new(shell, false);
    let text = renderer.lines(&groups);
    if renderer.altered {
        notes.push(Note::LineBreaks);
    }
    text
}

/// A field name with HTTPie's separators escaped.
fn item_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for c in name.chars() {
        if "\\:=@".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

fn groups(request: &ApiRequest, words: &mut Words, notes: &mut Vec<Note>) -> Vec<Vec<Word>> {
    let shape = shape(request, notes);
    let mut first = vec![Word::lit("http")];
    if request.options.follow_redirects {
        first.push(Word::lit("--follow"));
    }
    if !request.options.verify_tls {
        first.push(Word::lit("--verify=no"));
    }
    if request.options.timeout_ms != DEFAULT_TIMEOUT_MS {
        first.push(Word::lit(&format!(
            "--timeout={}",
            seconds(request.options.timeout_ms)
        )));
    }
    match request.body {
        Body::Form { .. } => first.push(Word::lit("--form")),
        Body::Multipart { .. } => first.push(Word::lit("--multipart")),
        _ => {}
    }
    first.push(Word::lit(&request.method));
    first.push(words.of(&shape.url));
    let mut groups = vec![first];
    let typed = shape
        .headers
        .iter()
        .any(|(name, _)| name.eq_ignore_ascii_case("Content-Type"));
    for (name, value) in &shape.headers {
        let item = if value.is_empty() {
            format!("{name};")
        } else {
            format!("{name}:{value}")
        };
        groups.push(vec![words.of(&item)]);
    }
    if matches!(request.body, Body::Raw { .. } | Body::Binary { .. }) && !typed {
        groups.push(vec![Word::lit("Content-Type:")]);
    }
    match &request.body {
        Body::None => {}
        Body::Json { text } | Body::Raw { text } => {
            groups.push(vec![Word::lit("--raw"), words.of(text)]);
        }
        Body::GraphQl { query, variables } => groups.push(vec![
            Word::lit("--raw"),
            words.of(&graphql_payload(query, variables)),
        ]),
        Body::Form { fields } => {
            for field in fields.iter().filter(|field| field.enabled) {
                let item = format!("{}={}", item_name(&field.name), field.value);
                groups.push(vec![words.of(&item)]);
            }
        }
        Body::Multipart { parts } => {
            for part in parts {
                let item = match &part.file {
                    Some(file) => {
                        let typed = part
                            .content_type
                            .as_deref()
                            .map(|ty| format!(";type={ty}"))
                            .unwrap_or_default();
                        format!("{}@{file}{typed}", item_name(&part.name))
                    }
                    None => format!("{}={}", item_name(&part.name), part.value),
                };
                groups.push(vec![words.of(&item)]);
            }
        }
        Body::Binary { path } => groups.push(vec![words.of(&format!("@{path}"))]),
    }
    match &shape.credentials {
        Some(Credentials::Basic(user, password)) => groups.push(vec![
            Word::lit("--auth"),
            words.of(&format!("{user}:{password}")),
        ]),
        Some(Credentials::Digest(user, password)) => groups.push(vec![
            Word::lit("--auth-type=digest"),
            Word::lit("--auth"),
            words.of(&format!("{user}:{password}")),
        ]),
        None => {}
    }
    groups
}

#[cfg(test)]
#[path = "httpie_tests.rs"]
mod tests;
