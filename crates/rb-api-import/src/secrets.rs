//! Secrets kept out of the imported files: a literal token, password, API key, cookie or client
//! secret - in an auth, a header, the URL's user info or query, a form field, a JSON body, an
//! environment value - becomes `{{$secret <name>}}`. The core resolves that from the process
//! variable `RB_SECRET_<NAME>` (`crates/rbrowser/src/api_client/resolve.rs`); a private value of
//! the same name (`api-client.secrets`) serves an environment's variable too. A URL takes no
//! space (the request line), so a URL's secret becomes `{{<name>}}` and the import sets that
//! variable to `{{$secret <name>}}` in `$shared`. The report names each placeholder and the
//! variable, never the value; a value already a template (`{{token}}`) stays as it is.

use std::fmt;

use rb_api_model::{ApiRequest, Auth, Body, Entry};
use serde_json::Value;

use crate::report::{SecretNote, cut};

/// The prefix of the process variable a placeholder reads.
pub const SECRET_PREFIX: &str = "RB_SECRET_";
/// The deepest JSON body scrubbed.
const MAX_JSON_DEPTH: usize = 64;

/// Whether a header, parameter, field or variable called `name` holds a secret.
#[must_use]
pub fn is_secret_name(name: &str) -> bool {
    let compact: String = name
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect();
    let neighbour = [
        "url",
        "uri",
        "type",
        "endpoint",
        "name",
        "id",
        "prefix",
        "placement",
    ]
    .iter()
    .any(|end| compact.ends_with(end));
    if neighbour {
        return false;
    }
    let parts = [
        "token",
        "secret",
        "password",
        "passwd",
        "apikey",
        "authorization",
        "cookie",
        "credential",
        "privatekey",
        "accesskey",
        "signature",
    ];
    let whole = [
        "key", "auth", "pass", "pwd", "sig", "jwt", "session", "bearer",
    ];
    parts.iter().any(|part| compact.contains(part)) || whole.contains(&compact.as_str())
}

/// Whether `value` is nothing but `{{...}}` variables.
#[must_use]
pub fn is_template(value: &str) -> bool {
    let mut rest = value.trim();
    if rest.is_empty() {
        return false;
    }
    while let Some(open) = rest.find("{{") {
        if !rest[..open].trim().is_empty() {
            return false;
        }
        let Some(close) = rest[open..].find("}}") else {
            return false;
        };
        rest = &rest[open + close + 2..];
    }
    rest.trim().is_empty()
}

/// A placeholder's name from `wanted`: lower case, `-` between words, at most 40 characters,
/// `secret` when nothing is left.
#[must_use]
pub fn placeholder_name(wanted: &str) -> String {
    let mut name = String::new();
    for c in wanted.chars() {
        if c.is_ascii_alphanumeric() {
            name.push(c.to_ascii_lowercase());
        } else if !name.ends_with('-') {
            name.push('-');
        }
    }
    let name: String = name.trim_matches('-').chars().take(40).collect();
    let name = name.trim_end_matches('-');
    if name.is_empty() {
        "secret".to_owned()
    } else if name.starts_with(|c: char| c.is_ascii_digit()) {
        format!("secret-{name}")
    } else {
        name.to_owned()
    }
}

/// The process variable `{{$secret name}}` reads.
#[must_use]
pub fn variable_of(name: &str) -> String {
    let upper: String = name
        .chars()
        .map(|c| {
            if c == '-' || c == '.' {
                '_'
            } else {
                c.to_ascii_uppercase()
            }
        })
        .collect();
    format!("{SECRET_PREFIX}{upper}")
}

/// The placeholders of one import: each name stands for one value (a second value under the same
/// name gets `-2`, `-3`, ...). The values live only here, for that comparison.
#[derive(Default)]
pub struct Secrets {
    taken: Vec<(String, String)>,
    notes: Vec<SecretNote>,
    /// The placeholders a URL got as `{{name}}`: a request line takes no space, so the URL names
    /// a variable that the import sets to `{{$secret name}}` (`$shared`).
    url_slots: Vec<String>,
}

impl fmt::Debug for Secrets {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Secrets")
            .field("notes", &self.notes)
            .finish_non_exhaustive()
    }
}

impl Secrets {
    /// The placeholders made, in order.
    #[must_use]
    pub fn notes(&self) -> &[SecretNote] {
        &self.notes
    }

    /// `value` (found at `at`, named after `wanted`) as a placeholder; an empty value and a
    /// template stay as they are.
    pub fn value(&mut self, at: &str, wanted: &str, value: &str) -> String {
        if value.trim().is_empty() || is_template(value) {
            return value.to_owned();
        }
        let base = placeholder_name(wanted);
        let mut name = base.clone();
        let mut n = 1;
        loop {
            match self.taken.iter().find(|(known, _)| *known == name) {
                Some((_, held)) if held == value => break,
                Some(_) => {
                    n += 1;
                    name = format!("{base}-{n}");
                }
                None => {
                    self.taken.push((name.clone(), value.to_owned()));
                    break;
                }
            }
        }
        let note = SecretNote {
            at: cut(at, 200),
            variable: variable_of(&name),
            name: name.clone(),
        };
        if !self.notes.contains(&note) {
            self.notes.push(note);
        }
        format!("{{{{$secret {name}}}}}")
    }

    /// A placeholder for a secret whose value the source does not hold (a Bruno `vars:secret`,
    /// an empty Postman secret): `{{$secret <name>}}`, noted like the others.
    pub fn slot(&mut self, at: &str, wanted: &str) -> String {
        let name = placeholder_name(wanted);
        let note = SecretNote {
            at: cut(at, 200),
            variable: variable_of(&name),
            name: name.clone(),
        };
        if !self.notes.contains(&note) {
            self.notes.push(note);
        }
        format!("{{{{$secret {name}}}}}")
    }

    /// The variables URLs got since the last call, each to be set to `{{$secret <name>}}`.
    pub fn take_url_slots(&mut self) -> Vec<String> {
        std::mem::take(&mut self.url_slots)
    }

    /// [`Secrets::value`] for a URL: `{{name}}` instead of `{{$secret name}}`, the name kept for
    /// [`Secrets::take_url_slots`].
    fn url_value(&mut self, at: &str, wanted: &str, value: &str) -> String {
        let placeholder = self.value(at, wanted, value);
        let Some(name) = placeholder
            .strip_prefix("{{$secret ")
            .and_then(|rest| rest.strip_suffix("}}"))
        else {
            return placeholder;
        };
        let name = name.to_owned();
        if !self.url_slots.contains(&name) {
            self.url_slots.push(name.clone());
        }
        format!("{{{{{name}}}}}")
    }

    /// Every literal secret of `request` (called `at` in the report) as a placeholder.
    pub fn scrub(&mut self, request: &mut ApiRequest, at: &str) {
        request.url = self.url(at, &request.url);
        for header in &mut request.headers {
            self.header(at, header);
        }
        self.auth(at, &mut request.auth);
        match &mut request.body {
            Body::Form { fields } => {
                for field in fields.iter_mut().filter(|f| is_secret_name(&f.name)) {
                    field.value = self.value(at, &field.name, &field.value);
                }
            }
            Body::Multipart { parts } => {
                for part in parts
                    .iter_mut()
                    .filter(|p| p.file.is_none() && is_secret_name(&p.name))
                {
                    part.value = self.value(at, &part.name, &part.value);
                }
            }
            Body::Json { text } => {
                if let Some(clean) = self.json(at, text) {
                    *text = clean;
                }
            }
            Body::GraphQl { variables, .. } => {
                if let Some(clean) = self.json(at, variables) {
                    *variables = clean;
                }
            }
            Body::None | Body::Raw { .. } | Body::Binary { .. } => {}
        }
    }

    fn header(&mut self, at: &str, header: &mut Entry) {
        if !is_secret_name(&header.name) {
            return;
        }
        let authorization = header.name.eq_ignore_ascii_case("Authorization")
            || header.name.eq_ignore_ascii_case("Proxy-Authorization");
        let value = header.value.trim().to_owned();
        if authorization && let Some((scheme, credential)) = value.split_once(' ') {
            let wanted = if scheme.eq_ignore_ascii_case("Bearer") {
                "token".to_owned()
            } else {
                scheme.to_owned()
            };
            header.value = format!("{scheme} {}", self.value(at, &wanted, credential.trim()));
        } else {
            header.value = self.value(at, &header.name, &value);
        }
    }

    fn auth(&mut self, at: &str, auth: &mut Auth) {
        match auth {
            Auth::Basic { password, .. } | Auth::Digest { password, .. } => {
                *password = self.value(at, "password", password);
            }
            Auth::Bearer { token } => *token = self.value(at, "token", token),
            Auth::ApiKey { name, value, .. } => *value = self.value(at, name, value),
            Auth::OAuth2 { client_secret, .. } => {
                *client_secret = self.value(at, "client-secret", client_secret);
            }
            Auth::None => {}
        }
    }

    /// `url` with the password of its user info and the secret query values as placeholders.
    fn url(&mut self, at: &str, url: &str) -> String {
        let (base, query) = match url.split_once('?') {
            Some((base, query)) => (base, Some(query)),
            None => (url, None),
        };
        let mut out = String::with_capacity(url.len());
        match base.split_once("://") {
            Some((scheme, rest)) => {
                let authority_end = rest.find('/').unwrap_or(rest.len());
                let (authority, path) = rest.split_at(authority_end);
                out.push_str(scheme);
                out.push_str("://");
                match authority.rsplit_once('@') {
                    Some((info, host)) => {
                        match info.split_once(':') {
                            Some((user, password)) => {
                                out.push_str(user);
                                out.push(':');
                                out.push_str(&self.url_value(at, "password", password));
                            }
                            None => out.push_str(info),
                        }
                        out.push('@');
                        out.push_str(host);
                    }
                    None => out.push_str(authority),
                }
                out.push_str(path);
            }
            None => out.push_str(base),
        }
        if let Some(query) = query {
            out.push('?');
            let pairs: Vec<String> = query
                .split('&')
                .map(|pair| match pair.split_once('=') {
                    Some((name, value)) if is_secret_name(name) => {
                        format!("{name}={}", self.url_value(at, name, value))
                    }
                    _ => pair.to_owned(),
                })
                .collect();
            out.push_str(&pairs.join("&"));
        }
        out
    }

    /// A JSON text with its secret string values as placeholders, pretty again; `None` when it
    /// does not parse or holds none.
    fn json(&mut self, at: &str, text: &str) -> Option<String> {
        let mut value: Value = serde_json::from_str(text).ok()?;
        let changed = self.json_value(at, &mut value, 0);
        if changed {
            serde_json::to_string_pretty(&value).ok()
        } else {
            None
        }
    }

    fn json_value(&mut self, at: &str, value: &mut Value, depth: usize) -> bool {
        if depth > MAX_JSON_DEPTH {
            return false;
        }
        let mut changed = false;
        match value {
            Value::Object(map) => {
                for (key, item) in map.iter_mut() {
                    if let Value::String(text) = item
                        && is_secret_name(key)
                    {
                        let clean = self.value(at, key, text);
                        if clean != *text {
                            *text = clean;
                            changed = true;
                        }
                    } else {
                        changed |= self.json_value(at, item, depth + 1);
                    }
                }
            }
            Value::Array(items) => {
                for item in items {
                    changed |= self.json_value(at, item, depth + 1);
                }
            }
            _ => {}
        }
        changed
    }
}

#[cfg(test)]
#[path = "secrets_tests.rs"]
mod tests;
