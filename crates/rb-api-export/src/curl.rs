//! A request as a curl command, for the shell of the options (`curl.exe` in PowerShell, where
//! `curl` may name `Invoke-WebRequest`). The URL and the options on the first line, then a line
//! per header, the credentials and the body:
//!
//! - `--globoff` when the URL holds `{`, `}`, `[` or `]` (a variable kept, an IPv6 address);
//!   `--location` when redirects are followed, `--insecure` without the TLS check,
//!   `--max-time` for a timeout other than the default, `--head` for HEAD, `--request` for
//!   every method but GET;
//! - JSON, raw text and GraphQL as `--data-raw` (no `@` read), a form as `--data-urlencode`
//!   per field, multipart as `--form-string` for a text and `--form name=@file;type=...` for a
//!   file, a binary body as `--data-binary @file`; a raw or binary body without its type sends
//!   none (`--header Content-Type:`), as the core does.

use rb_api_model::request::DEFAULT_TIMEOUT_MS;
use rb_api_model::{ApiRequest, Body};

use crate::shape::{Credentials, graphql_payload, seconds, shape};
use crate::shell::Renderer;
use crate::word::{Word, Words};
use crate::{Note, Shell};

/// `request` as a curl command for `shell`; `batch` for a script file.
pub(crate) fn command(
    request: &ApiRequest,
    words: &mut Words,
    shell: Shell,
    batch: bool,
    notes: &mut Vec<Note>,
) -> String {
    let groups = groups(request, words, shell, notes);
    let mut renderer = Renderer::new(shell, batch);
    let text = renderer.lines(&groups);
    if renderer.altered {
        notes.push(Note::LineBreaks);
    }
    text
}

fn lit(text: &str) -> Word {
    Word::lit(text)
}

/// The words of the command, a group per line.
fn groups(
    request: &ApiRequest,
    words: &mut Words,
    shell: Shell,
    notes: &mut Vec<Note>,
) -> Vec<Vec<Word>> {
    let shape = shape(request, notes);
    let url = words.of(&shape.url);
    let program = if shell == Shell::PowerShell {
        "curl.exe"
    } else {
        "curl"
    };
    let mut first = vec![lit(program)];
    if url.has_any(&['{', '}', '[', ']']) {
        first.push(lit("--globoff"));
    }
    if request.options.follow_redirects {
        first.push(lit("--location"));
    }
    if !request.options.verify_tls {
        first.push(lit("--insecure"));
    }
    if request.options.timeout_ms != DEFAULT_TIMEOUT_MS {
        first.push(lit("--max-time"));
        first.push(lit(&seconds(request.options.timeout_ms)));
    }
    match request.method.as_str() {
        "GET" => {}
        "HEAD" => first.push(lit("--head")),
        method => {
            first.push(lit("--request"));
            first.push(lit(method));
        }
    }
    first.push(url);
    let mut groups = vec![first];
    let typed = shape
        .headers
        .iter()
        .any(|(name, _)| name.eq_ignore_ascii_case("Content-Type"));
    for (name, value) in &shape.headers {
        // `Name;` sends a header without a value; `Name:` would remove it.
        let header = if value.is_empty() {
            format!("{name};")
        } else {
            format!("{name}: {value}")
        };
        groups.push(vec![lit("--header"), words.of(&header)]);
    }
    if matches!(request.body, Body::Raw { .. } | Body::Binary { .. }) && !typed {
        groups.push(vec![lit("--header"), lit("Content-Type:")]);
    }
    match &shape.credentials {
        Some(Credentials::Basic(user, password)) => {
            groups.push(vec![lit("--user"), words.of(&format!("{user}:{password}"))]);
        }
        Some(Credentials::Digest(user, password)) => groups.push(vec![
            lit("--digest"),
            lit("--user"),
            words.of(&format!("{user}:{password}")),
        ]),
        None => {}
    }
    body(request, words, &mut groups);
    groups
}

/// A form-data value in curl's own quotes: `"` and `\` escaped.
fn form_quoted(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

fn body(request: &ApiRequest, words: &mut Words, groups: &mut Vec<Vec<Word>>) {
    match &request.body {
        Body::None => {}
        Body::Json { text } | Body::Raw { text } => {
            groups.push(vec![lit("--data-raw"), words.of(text)]);
        }
        Body::GraphQl { query, variables } => {
            groups.push(vec![
                lit("--data-raw"),
                words.of(&graphql_payload(query, variables)),
            ]);
        }
        Body::Form { fields } => {
            for field in fields.iter().filter(|field| field.enabled) {
                let pair = format!("{}={}", crate::template::encode(&field.name), field.value);
                groups.push(vec![lit("--data-urlencode"), words.of(&pair)]);
            }
        }
        Body::Multipart { parts } => {
            for part in parts {
                let typed = part
                    .content_type
                    .as_deref()
                    .map(|ty| format!(";type={ty}"))
                    .unwrap_or_default();
                let (flag, spec) = match &part.file {
                    Some(file) => {
                        let file = if file.contains([';', ',', '"']) {
                            form_quoted(file)
                        } else {
                            file.clone()
                        };
                        ("--form", format!("{}=@{file}{typed}", part.name))
                    }
                    None if typed.is_empty() => {
                        ("--form-string", format!("{}={}", part.name, part.value))
                    }
                    None => (
                        "--form",
                        format!("{}={}{typed}", part.name, form_quoted(&part.value)),
                    ),
                };
                groups.push(vec![lit(flag), words.of(&spec)]);
            }
        }
        Body::Binary { path } => {
            groups.push(vec![lit("--data-binary"), words.of(&format!("@{path}"))]);
        }
    }
}

#[cfg(test)]
#[path = "curl_tests.rs"]
mod tests;
