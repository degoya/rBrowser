//! What every reader hands its requests and environments to: the [`Collector`] makes them fit
//! the model and the store - a one-line title, a folder path of safe names at most
//! [`MAX_FOLDER_DEPTH`] deep, the URL without spaces, every literal secret a placeholder
//! ([`crate::secrets`]), the dynamic variables rBrowser does not know reported - and drops what
//! still does not hold ([`ApiRequest::check`]) with the reason.

use rb_api_model::paths::MAX_DEPTH;
use rb_api_model::vars::is_name;
use rb_api_model::{ApiRequest, Auth, Body};
use serde::Serialize;

use crate::detect::Format;
use crate::report::{NoteKind, Report, cut};
use crate::secrets::{Secrets, is_secret_name};

/// The deepest folder an imported request lands in, below the import's own folder.
pub const MAX_FOLDER_DEPTH: usize = MAX_DEPTH - 1;
/// The most requests one import takes.
pub const MAX_REQUESTS: usize = 1000;
/// The most environments one import takes.
pub const MAX_ENVIRONMENTS: usize = 50;
/// The dynamic variables the core resolves (`crates/rbrowser/src/api_client/resolve.rs`).
pub const KNOWN_DYNAMIC: &[&str] = &["$guid", "$timestamp", "$processEnv", "$secret"];

/// A request and the folder it goes to (`""`, `users`, `users/admin`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Imported {
    pub folder: String,
    pub request: ApiRequest,
}

/// A variable of an environment; a secret's value is already its placeholder.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Variable {
    pub name: String,
    pub value: String,
}

/// An environment: `$shared` for the variables every one shares.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Environment {
    pub name: String,
    pub variables: Vec<Variable>,
}

/// The result of one import.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Import {
    /// The formats read, in order.
    pub formats: Vec<Format>,
    /// The collection's name, when a file names one.
    pub collection: Option<String>,
    pub requests: Vec<Imported>,
    pub environments: Vec<Environment>,
    pub report: Report,
}

/// Gathers one import.
#[derive(Debug, Default)]
pub struct Collector {
    formats: Vec<Format>,
    collection: Option<String>,
    requests: Vec<Imported>,
    environments: Vec<Environment>,
    report: Report,
    secrets: Secrets,
}

/// `name` as a folder's name: lower case, `-` between words, at most 60 characters; `folder`
/// when nothing is left.
#[must_use]
pub fn folder_name(name: &str) -> String {
    if !name.chars().any(|c| c.is_ascii_alphanumeric()) {
        return "folder".to_owned();
    }
    let file = rb_api_model::paths::file_name(name);
    file.strip_suffix(rb_api_model::paths::EXTENSION)
        .unwrap_or(&file)
        .to_owned()
}

/// `url` with its spaces and line breaks escaped, so it stays one token of the request line;
/// a variable's `{{ name }}` written `{{name}}`.
#[must_use]
pub fn clean_url(url: &str) -> String {
    let mut out = String::with_capacity(url.len());
    let mut rest = url.trim();
    while !rest.is_empty() {
        let (text, variable) = match rest.find("{{") {
            Some(open) => match rest[open..].find("}}") {
                Some(close) => (&rest[..open], Some(&rest[open + 2..open + close])),
                None => (rest, None),
            },
            None => (rest, None),
        };
        escape_into(&mut out, text);
        match variable {
            Some(inner) => {
                out.push_str("{{");
                escape_into(&mut out, inner.trim());
                out.push_str("}}");
                rest = &rest[text.len() + inner.len() + 4..];
            }
            None => rest = "",
        }
    }
    out
}

fn escape_into(out: &mut String, text: &str) {
    for c in text.chars() {
        match c {
            ' ' => out.push_str("%20"),
            '\t' => out.push_str("%09"),
            '\r' | '\n' => {}
            other => out.push(other),
        }
    }
}

impl Collector {
    /// Notes that `format` was read.
    pub fn format(&mut self, format: Format) {
        if !self.formats.contains(&format) {
            self.formats.push(format);
        }
    }

    /// The collection's name; the first one named stays.
    pub fn collection(&mut self, name: &str) {
        let name = cut(name, 120);
        if self.collection.is_none() && !name.is_empty() {
            self.collection = Some(name);
        }
    }

    /// Something of `at` rBrowser has no counterpart for; the rest was taken.
    pub fn reported(&mut self, at: &str, code: &str, detail: &str) {
        self.report.note(NoteKind::Reported, code, at, detail);
    }

    /// Something of `at` left out, for the reason `code` names.
    pub fn dropped(&mut self, at: &str, code: &str, detail: &str) {
        self.report.note(NoteKind::Dropped, code, at, detail);
    }

    /// The requests taken so far.
    #[must_use]
    pub fn count(&self) -> usize {
        self.requests.len()
    }

    /// Takes `request` into the folder `folders` (the source's names, outermost first).
    pub fn request(&mut self, folders: &[String], mut request: ApiRequest) {
        request.method = request.method.trim().to_ascii_uppercase();
        if request.method.is_empty() {
            request.method = "GET".to_owned();
        }
        let dynamic = unknown_dynamic(&request);
        request.url = clean_url(&request.url);
        request.name = title_of(&request);
        let at = request.name.clone();
        if self.requests.len() >= MAX_REQUESTS {
            self.dropped(&at, "limit", &MAX_REQUESTS.to_string());
            return;
        }
        request.comments = request
            .comments
            .iter()
            .flat_map(|comment| comment.lines().map(|line| cut(line, 500)))
            .filter(|line| !line.is_empty())
            .take(50)
            .collect();
        for header in &mut request.headers {
            header.name = header.name.trim().to_owned();
            header.value = header.value.replace(['\r', '\n'], " ").trim().to_owned();
        }
        request.headers.retain(|header| !header.name.is_empty());
        self.secrets.scrub(&mut request, &at);
        for name in dynamic {
            self.reported(&at, "variable", &name);
        }
        let slots: Vec<(String, String, bool)> = self
            .secrets
            .take_url_slots()
            .into_iter()
            .map(|name| {
                let value = format!("{{{{$secret {name}}}}}");
                (name, value, false)
            })
            .collect();
        if !slots.is_empty() {
            self.environment(rb_api_model::env::SHARED, &slots);
        }
        if let Err(invalid) = request.check() {
            self.dropped(&at, "invalid", &invalid.reason());
            return;
        }
        let folder = self.folder(folders, &at);
        self.requests.push(Imported { folder, request });
    }

    fn folder(&mut self, folders: &[String], at: &str) -> String {
        let mut names: Vec<String> = folders
            .iter()
            .filter(|name| !name.trim().is_empty())
            .map(|name| folder_name(name))
            .collect();
        if names.len() > MAX_FOLDER_DEPTH {
            let rest = names.split_off(MAX_FOLDER_DEPTH - 1).join("-");
            names.push(folder_name(&rest));
            self.reported(at, "folder", &names.join("/"));
        }
        names.join("/")
    }

    /// Takes an environment: `variables` as (name, value, secret by the source); a value of a
    /// secret - by the source or by its name - becomes its placeholder.
    pub fn environment(&mut self, name: &str, variables: &[(String, String, bool)]) {
        let name = if name.trim() == rb_api_model::env::SHARED {
            rb_api_model::env::SHARED.to_owned()
        } else {
            cut(name, 64)
        };
        let shown = if name.is_empty() {
            "environment".to_owned()
        } else {
            name.clone()
        };
        if name.is_empty() {
            self.dropped(&shown, "environment", "no name");
            return;
        }
        let at = format!("environment {shown}");
        let mut taken: Vec<Variable> = Vec::new();
        for (variable, value, secret) in variables {
            let variable = variable.trim();
            if !is_name(variable) {
                self.dropped(&at, "environment", variable);
                continue;
            }
            let value = if *secret && value.trim().is_empty() {
                self.secrets.slot(&at, variable)
            } else if *secret || is_secret_name(variable) {
                self.secrets.value(&at, variable, value)
            } else {
                value.clone()
            };
            if taken.len() >= rb_api_model::env::MAX_ENTRIES {
                self.dropped(&at, "limit", variable);
                continue;
            }
            taken.retain(|known| known.name != variable);
            taken.push(Variable {
                name: variable.to_owned(),
                value,
            });
        }
        let full = self.environments.len() >= MAX_ENVIRONMENTS;
        match self.environments.iter_mut().find(|e| e.name == name) {
            Some(known) => {
                for variable in taken {
                    if !known.variables.iter().any(|v| v.name == variable.name) {
                        known.variables.push(variable);
                    }
                }
            }
            None if full => {
                self.dropped(&at, "limit", &MAX_ENVIRONMENTS.to_string());
            }
            None => self.environments.push(Environment {
                name,
                variables: taken,
            }),
        }
    }

    /// The import.
    #[must_use]
    pub fn finish(self) -> Import {
        let mut report = self.report;
        report.imported = self.requests.len();
        report.environments = self.environments.len();
        report.secrets = self.secrets.notes().to_vec();
        Import {
            formats: self.formats,
            collection: self.collection,
            requests: self.requests,
            environments: self.environments,
            report,
        }
    }
}

/// The title a request is taken under: its own, else its method and path.
#[must_use]
pub fn title_of(request: &ApiRequest) -> String {
    if request.name.trim().is_empty() {
        let method = request.method.trim().to_ascii_uppercase();
        cut(
            &format!("{method} {}", path_of(&clean_url(&request.url))),
            200,
        )
    } else {
        cut(&request.name, 200)
    }
}

/// The path of `url` for a title: no scheme, no host, no query.
fn path_of(url: &str) -> &str {
    let rest = match url.split_once("://") {
        Some((_, rest)) => rest.find('/').map_or("/", |at| &rest[at..]),
        None => url,
    };
    rest.split(['?', '#']).next().unwrap_or(rest)
}

/// The dynamic variables (`{{$name}}`) of `request` the core does not resolve, each once.
fn unknown_dynamic(request: &ApiRequest) -> Vec<String> {
    let mut texts: Vec<&str> = vec![request.url.as_str()];
    texts.extend(request.headers.iter().map(|h| h.value.as_str()));
    match &request.body {
        Body::Json { text } | Body::Raw { text } => texts.push(text),
        Body::GraphQl { query, variables } => texts.extend([query.as_str(), variables.as_str()]),
        Body::Form { fields } => texts.extend(fields.iter().map(|f| f.value.as_str())),
        Body::Multipart { parts } => texts.extend(parts.iter().map(|p| p.value.as_str())),
        Body::None | Body::Binary { .. } => {}
    }
    match &request.auth {
        Auth::Basic { username, password } | Auth::Digest { username, password } => {
            texts.extend([username.as_str(), password.as_str()]);
        }
        Auth::Bearer { token } => texts.push(token),
        Auth::ApiKey { value, .. } => texts.push(value),
        Auth::OAuth2 { client_secret, .. } => texts.push(client_secret),
        Auth::None => {}
    }
    let mut out: Vec<String> = Vec::new();
    for text in texts {
        let mut rest = text;
        while let Some(open) = rest.find("{{") {
            let inner = &rest[open + 2..];
            let Some(close) = inner.find("}}") else {
                break;
            };
            let name = inner[..close].split_whitespace().next().unwrap_or("");
            if name.starts_with('$')
                && !KNOWN_DYNAMIC.contains(&name)
                && !out.iter().any(|n| n == name)
            {
                out.push(name.to_owned());
            }
            rest = &inner[close + 2..];
        }
    }
    out
}

#[cfg(test)]
#[path = "collect_tests.rs"]
mod tests;
