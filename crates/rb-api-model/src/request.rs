//! One API request as the panel edits it, the `.http` file stores it and the core sends it. Every
//! text may hold `{{variables}}` ([`crate::vars`]); nothing here is resolved.

use serde::{Deserialize, Serialize};

use crate::checks::{Check, MAX_CHECKS};

/// The longest URL template, in bytes.
pub const MAX_URL_BYTES: usize = 8 * 1024;
/// The most headers, form fields or multipart parts of one request.
pub const MAX_ENTRIES: usize = 200;
/// The longest body template, in bytes.
pub const MAX_BODY_BYTES: usize = 4 * 1024 * 1024;
/// The default timeout of a request, in milliseconds.
pub const DEFAULT_TIMEOUT_MS: u64 = 30_000;
/// The longest timeout a request may ask for, in milliseconds.
pub const MAX_TIMEOUT_MS: u64 = 600_000;

/// A request.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApiRequest {
    /// The title (`### <name>` in the file); empty for an unnamed draft.
    #[serde(default)]
    pub name: String,
    /// The method, an HTTP token in upper case (`GET`, `POST`, `PROPFIND`).
    pub method: String,
    /// The URL template, the query included.
    pub url: String,
    /// The headers, in order; a disabled one is kept but not sent.
    #[serde(default)]
    pub headers: Vec<Entry>,
    /// The body.
    #[serde(default)]
    pub body: Body,
    /// How the request authenticates.
    #[serde(default)]
    pub auth: Auth,
    /// Timeout, redirects, TLS.
    #[serde(default)]
    pub options: Options,
    /// The file's own comment lines (without `#`), kept so a hand-written note survives a save.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub comments: Vec<String>,
    /// What the answer must show (RB-096-04, `# @rb-check` in the file).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub checks: Vec<Check>,
}

/// A name and a value: a header or a form field.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Entry {
    pub name: String,
    #[serde(default)]
    pub value: String,
    /// Sent (`true`, the default) or kept for later.
    #[serde(default = "enabled")]
    pub enabled: bool,
}

const fn enabled() -> bool {
    true
}

impl Entry {
    /// An enabled entry.
    #[must_use]
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
            enabled: true,
        }
    }
}

/// One part of a `multipart/form-data` body: a text value or a file.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Part {
    pub name: String,
    /// The text of a value part; empty for a file part.
    #[serde(default)]
    pub value: String,
    /// A file part: its path relative to the request's file (`./logo.png`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    /// The part's `Content-Type`, when it names one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
}

/// The body of a request.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Body {
    /// No body.
    #[default]
    None,
    /// JSON text, checked before it is sent.
    Json { text: String },
    /// `application/x-www-form-urlencoded`: the fields, not encoded.
    Form { fields: Vec<Entry> },
    /// `multipart/form-data`.
    Multipart { parts: Vec<Part> },
    /// Text as it is; the `Content-Type` header names its type.
    Raw { text: String },
    /// The bytes of a file, its path relative to the request's file.
    Binary { path: String },
    /// A GraphQL query and its variables (JSON text, may be empty), sent as JSON.
    #[serde(rename = "graphql")]
    GraphQl {
        query: String,
        #[serde(default)]
        variables: String,
    },
}

impl Body {
    /// The kind's id: `none`, `json`, `form`, `multipart`, `raw`, `binary`, `graphql`.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Json { .. } => "json",
            Self::Form { .. } => "form",
            Self::Multipart { .. } => "multipart",
            Self::Raw { .. } => "raw",
            Self::Binary { .. } => "binary",
            Self::GraphQl { .. } => "graphql",
        }
    }

    /// The `Content-Type` sent when the request names none; `None` for a raw text or a file,
    /// whose type only the person knows, and for no body. A multipart body's boundary is the
    /// sender's.
    #[must_use]
    pub const fn default_content_type(&self) -> Option<&'static str> {
        match self {
            Self::Json { .. } | Self::GraphQl { .. } => Some("application/json"),
            Self::Form { .. } => Some("application/x-www-form-urlencoded"),
            Self::Multipart { .. } => Some("multipart/form-data"),
            Self::None | Self::Raw { .. } | Self::Binary { .. } => None,
        }
    }
}

/// Where an API key goes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum KeyPlace {
    #[default]
    Header,
    Query,
}

/// The grant of an OAuth 2 token.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OAuthGrant {
    #[default]
    ClientCredentials,
    /// Authorization code with PKCE (S256).
    AuthorizationCode,
}

/// How a request authenticates. Every text is a template; a secret belongs in a variable that
/// resolves from `RB_SECRET_*` or `config.local.json`, never in the file.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Auth {
    /// None, or the headers say it.
    #[default]
    None,
    Basic {
        username: String,
        #[serde(default)]
        password: String,
    },
    Bearer {
        token: String,
    },
    ApiKey {
        name: String,
        value: String,
        #[serde(default, rename = "in")]
        place: KeyPlace,
    },
    Digest {
        username: String,
        #[serde(default)]
        password: String,
    },
    #[serde(rename = "oauth2")]
    OAuth2 {
        #[serde(default)]
        grant: OAuthGrant,
        token_url: String,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        auth_url: String,
        client_id: String,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        client_secret: String,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        scope: String,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        redirect_uri: String,
    },
}

impl Auth {
    /// The kind's id: `none`, `basic`, `bearer`, `apiKey`, `digest`, `oauth2`.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Basic { .. } => "basic",
            Self::Bearer { .. } => "bearer",
            Self::ApiKey { .. } => "apiKey",
            Self::Digest { .. } => "digest",
            Self::OAuth2 { .. } => "oauth2",
        }
    }
}

/// How a request is sent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Options {
    /// The whole exchange's timeout, in milliseconds.
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
    /// Follow redirects (up to ten) or answer the first.
    #[serde(default = "enabled")]
    pub follow_redirects: bool,
    /// Check the server's TLS certificate.
    #[serde(default = "enabled")]
    pub verify_tls: bool,
}

const fn default_timeout() -> u64 {
    DEFAULT_TIMEOUT_MS
}

impl Default for Options {
    fn default() -> Self {
        Self {
            timeout_ms: DEFAULT_TIMEOUT_MS,
            follow_redirects: true,
            verify_tls: true,
        }
    }
}

/// Why a request cannot be stored or sent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Invalid {
    /// The method is no HTTP token.
    Method,
    /// The URL is empty, too long or holds a line break.
    Url,
    /// A header name is no HTTP token, or a value holds a line break.
    Header(String),
    /// Too many headers, fields or parts.
    TooMany,
    /// The body is over [`MAX_BODY_BYTES`].
    BodyTooLarge,
    /// The timeout is 0 or over [`MAX_TIMEOUT_MS`].
    Timeout,
    /// A form field or part name, or a file path, holds a line break; or a text part's value
    /// reads as a file include (`< ./file`).
    Field(String),
    /// Too many checks, or check `n` (from 1) is not of its form (RB-096-04).
    Check(usize),
}

impl Invalid {
    /// The reason's id, for a failure parameter.
    #[must_use]
    pub fn reason(&self) -> String {
        match self {
            Self::Method => "method".to_owned(),
            Self::Url => "url".to_owned(),
            Self::Header(name) => format!("header {name}"),
            Self::TooMany => "too many entries".to_owned(),
            Self::BodyTooLarge => "body too large".to_owned(),
            Self::Timeout => "timeout".to_owned(),
            Self::Field(name) => format!("field {name}"),
            Self::Check(number) => format!("check {number}"),
        }
    }
}

/// Whether `text` is an HTTP token (RFC 9110 `tchar`).
#[must_use]
pub fn is_token(text: &str) -> bool {
    !text.is_empty()
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "!#$%&'*+-.^_`|~".contains(c))
}

fn one_line(text: &str) -> bool {
    !text.contains(['\r', '\n'])
}

impl ApiRequest {
    /// A request of `method` to `url`, nothing else set.
    #[must_use]
    pub fn new(method: &str, url: &str) -> Self {
        Self {
            method: method.to_owned(),
            url: url.to_owned(),
            ..Self::default()
        }
    }

    /// Checks what a file and a sender rely on: a token method, a one-line URL, token header
    /// names with one-line values, the limits.
    ///
    /// # Errors
    ///
    /// The first problem found.
    pub fn check(&self) -> Result<(), Invalid> {
        if !one_line(&self.name) || self.name.len() > 200 {
            return Err(Invalid::Field(String::new()));
        }
        if !is_token(&self.method) || self.method != self.method.to_ascii_uppercase() {
            return Err(Invalid::Method);
        }
        let url = self.url.trim();
        if url.is_empty() || self.url.len() > MAX_URL_BYTES || !one_line(&self.url) {
            return Err(Invalid::Url);
        }
        if self.headers.len() > MAX_ENTRIES {
            return Err(Invalid::TooMany);
        }
        for header in &self.headers {
            if !is_token(&header.name) || !one_line(&header.value) {
                return Err(Invalid::Header(header.name.clone()));
            }
        }
        if self.options.timeout_ms == 0 || self.options.timeout_ms > MAX_TIMEOUT_MS {
            return Err(Invalid::Timeout);
        }
        if self.checks.len() > MAX_CHECKS {
            return Err(Invalid::Check(MAX_CHECKS + 1));
        }
        if let Some(at) = self
            .checks
            .iter()
            .position(|check| check.validate().is_err())
        {
            return Err(Invalid::Check(at + 1));
        }
        self.check_body()
    }

    fn check_body(&self) -> Result<(), Invalid> {
        let size = match &self.body {
            Body::None => 0,
            Body::Json { text } | Body::Raw { text } => text.len(),
            Body::GraphQl { query, variables } => query.len() + variables.len(),
            Body::Binary { path } => {
                if !one_line(path) || path.trim().is_empty() {
                    return Err(Invalid::Field(path.clone()));
                }
                0
            }
            Body::Form { fields } => {
                if fields.len() > MAX_ENTRIES {
                    return Err(Invalid::TooMany);
                }
                if let Some(bad) = fields.iter().find(|field| !one_line(&field.name)) {
                    return Err(Invalid::Field(bad.name.clone()));
                }
                fields.iter().map(|f| f.name.len() + f.value.len()).sum()
            }
            Body::Multipart { parts } => {
                if parts.len() > MAX_ENTRIES {
                    return Err(Invalid::TooMany);
                }
                let bad = parts.iter().find(|part| {
                    part.name.trim().is_empty()
                        || part.name.contains('"')
                        || !one_line(&part.name)
                        || part.file.as_deref().is_some_and(|file| !one_line(file))
                        || (part.file.is_none() && crate::http_file::holds_include(&part.value))
                        || part.content_type.as_deref().is_some_and(|ty| !one_line(ty))
                });
                if let Some(bad) = bad {
                    return Err(Invalid::Field(bad.name.clone()));
                }
                parts.iter().map(|p| p.name.len() + p.value.len()).sum()
            }
        };
        if size > MAX_BODY_BYTES {
            return Err(Invalid::BodyTooLarge);
        }
        Ok(())
    }

    /// The value of the first enabled header `name` (case-insensitive).
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|header| header.enabled && header.name.eq_ignore_ascii_case(name))
            .map(|header| header.value.as_str())
    }
}

#[cfg(test)]
#[path = "request_tests.rs"]
mod tests;
