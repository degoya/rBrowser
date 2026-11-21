//! `.http` files of VS Code REST Client and the JetBrains HTTP Client, several requests per file
//! (`###` between them), and JetBrains' environment files.
//!
//! Mapped: each request through rBrowser's own reader (`rb_api_model::http_parse`), the file's
//! variables (`@name = value`) as `$shared` variables, `http-client.env.json` as environments
//! and `http-client.private.env.json` as environments of secrets; a file's requests go into a
//! folder named after it. Reported (and taken out of the request): JetBrains' pre-request
//! scripts (`< {% %}`, `< ./script.js` before the request line), response handlers
//! (`> {% %}`, `> ./handler.js`), output to a file (`>>`, `>>!`) and response references
//! (`<>`); dropped: a request that does not read, with its line.

use rb_api_model::http_parse::{parse_http, split_requests};
use rb_api_model::vars::is_name;
use serde_json::Value;

use crate::Source;
use crate::collect::{Collector, title_of};
use crate::json::as_text;

/// The JetBrains environment file, checked in.
pub const ENV_FILE: &str = "http-client.env.json";
/// The JetBrains environment file of private values.
pub const PRIVATE_ENV_FILE: &str = "http-client.private.env.json";

/// Reads an `.http` file or a JetBrains environment file.
///
/// # Errors
///
/// An environment file that is no JSON object.
pub fn read(source: &Source, out: &mut Collector) -> Result<(), String> {
    let file = source
        .name
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(&source.name)
        .to_ascii_lowercase();
    if file == ENV_FILE || file == PRIVATE_ENV_FILE {
        return environments(&source.text, file == PRIVATE_ENV_FILE, out);
    }
    let stem = file
        .rsplit_once('.')
        .map_or(file.as_str(), |(stem, _)| stem)
        .to_owned();
    if !stem.is_empty() {
        out.collection(&stem);
    }
    let folder: Vec<String> = if stem.is_empty() {
        Vec::new()
    } else {
        vec![stem]
    };
    let (text, variables) = file_variables(&source.text);
    if !variables.is_empty() {
        out.environment(rb_api_model::env::SHARED, &variables);
    }
    for (index, segment) in split_requests(&text).iter().enumerate() {
        let (clean, notes) = handlers(segment);
        match parse_http(&clean) {
            Ok(request) => {
                let at = title_of(&request);
                for (code, detail) in notes {
                    out.reported(&at, code, detail);
                }
                out.request(&folder, request);
            }
            Err(error) => out.dropped(
                &format!("request {}", index + 1),
                "invalid",
                &format!("line {}: {}", error.line, error.reason),
            ),
        }
    }
    Ok(())
}

fn environments(text: &str, private: bool, out: &mut Collector) -> Result<(), String> {
    let value: Value = serde_json::from_str(text).map_err(|error| format!("not JSON: {error}"))?;
    let Value::Object(environments) = value else {
        return Err("not an object of environments".to_owned());
    };
    for (name, variables) in &environments {
        let Value::Object(variables) = variables else {
            out.dropped(
                &format!("environment {name}"),
                "environment",
                "not an object",
            );
            continue;
        };
        let variables: Vec<(String, String, bool)> = variables
            .iter()
            .map(|(key, value)| (key.clone(), as_text(value).into_owned(), private))
            .collect();
        out.environment(name, &variables);
    }
    Ok(())
}

/// `text` without its `@name = value` lines, and those variables.
fn file_variables(text: &str) -> (String, Vec<(String, String, bool)>) {
    let mut kept = String::with_capacity(text.len());
    let mut variables = Vec::new();
    for line in text.lines() {
        let variable = line
            .trim()
            .strip_prefix('@')
            .and_then(|rest| rest.split_once('='))
            .filter(|(name, _)| is_name(name.trim()));
        match variable {
            Some((name, value)) => {
                variables.push((name.trim().to_owned(), value.trim().to_owned(), false));
            }
            None => {
                kept.push_str(line);
                kept.push('\n');
            }
        }
    }
    (kept, variables)
}

/// `segment` without JetBrains' scripts, handlers and redirects, and what they were.
fn handlers(segment: &str) -> (String, Vec<(&'static str, &'static str)>) {
    let mut kept = String::with_capacity(segment.len());
    let mut notes: Vec<(&'static str, &'static str)> = Vec::new();
    let mut note = |code: &'static str, detail: &'static str| {
        if !notes.contains(&(code, detail)) {
            notes.push((code, detail));
        }
    };
    let mut before_request = true;
    let mut lines = segment.lines();
    while let Some(line) = lines.next() {
        let trimmed = line.trim();
        let script = trimmed.starts_with("> {%") || trimmed.starts_with("< {%");
        if script {
            note(
                "script",
                if trimmed.starts_with('<') {
                    "pre-request"
                } else {
                    "response handler"
                },
            );
            let mut closed = trimmed.len() > 4 && trimmed.ends_with("%}");
            while !closed {
                match lines.next() {
                    Some(next) => closed = next.trim_end().ends_with("%}"),
                    None => break,
                }
            }
            continue;
        }
        if trimmed.starts_with(">>") {
            note("setting", "output to a file");
            continue;
        }
        if trimmed.starts_with("<>") {
            note("setting", "response reference");
            continue;
        }
        if trimmed.starts_with("> ") {
            note("script", "response handler");
            continue;
        }
        if before_request && trimmed.starts_with("< ") {
            note("script", "pre-request");
            continue;
        }
        let comment = trimmed.starts_with('#') || trimmed.starts_with("//");
        if !trimmed.is_empty() && !comment {
            before_request = false;
        }
        kept.push_str(line);
        kept.push('\n');
    }
    (kept, notes)
}

#[cfg(test)]
#[path = "http_files_tests.rs"]
mod tests;
