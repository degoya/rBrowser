//! A request of the page taken into the API client (RB-096-08): what the views captured - the
//! method, the address (its secret query values already `***` in the capture), the headers as
//! sent and the post data - becomes an [`ApiRequest`] named after its method and path, in a
//! folder named after its host.
//!
//! Secrets never stay in the request: a header that carries one (`rb_util::mask`'s list and
//! every token-like name, `rb_api_import::secrets::is_secret_name`), a secret query parameter of
//! the address or the `Referer`, and a secret field of the body (`capture_body.rs`) become `{{variables}}`
//! named after the host and what they held (`api-example-test-token`); a body that cannot be
//! checked is left out and said ([`BodyMissing`]). Each [`Variable`] carries
//! the value the capture held - none where the capture masked it - for the private values; any
//! other place the same value shows up becomes the variable too. The headers only a browser or
//! the connection sets (`Host`, `Content-Length`, `Accept-Encoding`, `sec-*`, HTTP/2's `:`
//! names) are left out.
//!
//! The w31 security review (`capture_guard.rs`): a secret segment of the path is a variable as
//! well, a body that holds a file include (`< ./file`) is left out, and a request that holds a
//! `{{` the capture did not make is refused ([`Refused::Template`]).

use std::fmt;

use rb_api_import::secrets::is_secret_name;
use rb_api_model::http_parse::form_unescape;
use rb_api_model::{ApiRequest, Body, Entry};
use rb_util::mask::{MASK, SENSITIVE_PARAMS, sensitive_header, sensitive_param};

#[path = "capture_body.rs"]
mod body;
#[path = "capture_guard.rs"]
mod guard;

/// The largest post data taken as the body, in characters: a saved request's file is read up to
/// 1 MiB (`super::store::MAX_FILE_BYTES`).
pub const MAX_BODY_CHARS: usize = 512 * 1024;
/// The shortest value looked for elsewhere in the request (and masked in an answer): a shorter
/// one would replace ordinary text.
pub const MIN_SWEPT_CHARS: usize = 8;
/// The capture cuts a longer text (`rb_mcp::logs::MAX_TEXT`) and ends it with `...`.
const CAPTURE_TEXT_CHARS: usize = 2048;
/// Headers left out: the client sets them for its own connection and body.
const NOT_TAKEN: &[&str] = &[
    "accept-encoding",
    "connection",
    "content-length",
    "host",
    "keep-alive",
    "te",
    "transfer-encoding",
    "upgrade",
];

/// What the views captured of one request. No `Debug`: the headers hold the session's secrets.
pub struct Captured {
    pub method: String,
    /// The address as the capture keeps it.
    pub url: String,
    /// The headers as sent.
    pub headers: Vec<(String, String)>,
    /// The post data, when the request had one and the view still holds it.
    pub body: Option<String>,
    /// The request carried a body the view could not hand over (`Network.getRequestPostData`
    /// failed: another engine, a blob or a stream).
    pub body_unread: bool,
}

/// Why the post data of a captured request is not in its body.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BodyMissing {
    /// Over [`MAX_BODY_CHARS`].
    TooLong,
    /// It could not be checked for secrets (`capture_body.rs`), so it is not written.
    Unchecked,
    /// The view could not hand it over.
    Unread,
}

impl BodyMissing {
    /// The id the answers carry (`bodyMissing`).
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::TooLong => "tooLong",
            Self::Unchecked => "unchecked",
            Self::Unread => "unread",
        }
    }
}

/// Why a captured request is not taken.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refused {
    /// Not an `http(s)` request.
    NotHttp,
    /// It holds a `{{` of the page: on send that would resolve a secret for the captured host.
    Template,
}

impl Refused {
    /// The reason the answers carry (`skipped[].reason`).
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::NotHttp => "notHttp",
            Self::Template => "template",
        }
    }
}

/// Whether a request of `method` with `headers` as sent carried a body: its `Content-Length`
/// says so; without one a `Content-Type` or a method that sends one does.
#[must_use]
pub fn body_expected(method: &str, headers: &[(String, String)]) -> bool {
    let header = |wanted: &str| {
        headers
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(wanted))
            .map(|(_, value)| value.trim())
    };
    if let Some(length) = header("content-length") {
        return !matches!(length.parse::<u64>(), Ok(0));
    }
    header("content-type").is_some()
        || matches!(
            method.trim().to_ascii_uppercase().as_str(),
            "POST" | "PUT" | "PATCH"
        )
}

/// A variable made for a secret, with the value the capture held (none where it was masked).
pub struct Variable {
    pub name: String,
    pub value: Option<String>,
}

impl fmt::Debug for Variable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Variable")
            .field("name", &self.name)
            .field("known", &self.value.is_some())
            .finish()
    }
}

/// A captured request as the API client takes it.
#[derive(Debug)]
pub struct Converted {
    /// Titled `<METHOD> <path>`; every secret a variable.
    pub request: ApiRequest,
    /// The folder below `.rbrowser/api/`: the host and port.
    pub folder: String,
    /// The path without the query, its secrets variables as in the saved file: with the method,
    /// what a duplicate shares.
    pub path: String,
    pub variables: Vec<Variable>,
    /// Why the post data is not in the request, when it is not.
    pub body_missing: Option<BodyMissing>,
    /// The capture held the address masked or cut where no variable could take its place.
    pub incomplete: bool,
}

/// Whether header `name` carries a secret: by the lists, and by its name without `X-` as a
/// parameter's (`X-Session`, `X-Auth`, `X-Sid`), its `Id` ending no exception (`X-Session-Id`).
#[must_use]
pub fn is_secret_header(name: &str) -> bool {
    let bare = name
        .get(..2)
        .filter(|start| start.eq_ignore_ascii_case("x-"))
        .map_or(name, |_| &name[2..]);
    let compact = |text: &str| -> String {
        text.chars()
            .filter(char::is_ascii_alphanumeric)
            .map(|c| c.to_ascii_lowercase())
            .collect()
    };
    let bare_compact = compact(bare);
    let stem = bare_compact
        .strip_suffix("id")
        .filter(|stem| !stem.is_empty());
    sensitive_header(name)
        || is_secret_name(name)
        || is_secret_name(bare)
        || stem.is_some_and(is_secret_name)
        || SENSITIVE_PARAMS
            .iter()
            .any(|param| compact(param) == bare_compact)
}

/// Whether query parameter or body field `name` carries a secret.
#[must_use]
pub fn is_secret_param(name: &str) -> bool {
    sensitive_param(name) || is_secret_name(name)
}

/// `text` in lower case, words joined by `-`, at most `most` characters.
fn slug(text: &str, most: usize) -> String {
    let mut out = String::new();
    for c in text.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    let cut: String = out.trim_end_matches('-').chars().take(most).collect();
    cut.trim_end_matches('-').to_owned()
}

/// The longest folder name.
const MAX_FOLDER: usize = 80;

/// FNV-1a of `text`: a short, stable tag that tells two hosts of one slug apart.
fn tag(text: &str) -> u32 {
    text.bytes().fold(0x811c_9dc5_u32, |hash, byte| {
        (hash ^ u32::from(byte)).wrapping_mul(0x0100_0193)
    })
}

/// The folder of the requests to `authority` (`host[:port]`), and the prefix of their
/// variables: its slug when only letters, digits, `.` and `:` make it and it is not cut, else
/// the slug and a tag of the authority - `api-example.test` never shares `api.example.test`'s.
#[must_use]
pub fn folder_of(authority: &str) -> String {
    let lower = authority.to_ascii_lowercase();
    let whole = slug(&lower, usize::MAX);
    let plain = lower
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == ':');
    if plain && !whole.is_empty() && whole.len() <= MAX_FOLDER {
        return whole;
    }
    let base = match slug(&lower, MAX_FOLDER - 9) {
        base if base.is_empty() => "captured".to_owned(),
        base => base,
    };
    format!("{base}-{:08x}", tag(&lower))
}

/// The folder [`folder_of`] gave `authority` before it was tagged (`api-example.test` and
/// `api.example.test` shared one): `None` when it is the same as today's.
#[must_use]
pub fn untagged_folder_of(authority: &str) -> Option<String> {
    let old = slug(authority, MAX_FOLDER);
    (!old.is_empty() && old != folder_of(authority)).then_some(old)
}

/// The variables of one request: a name per value, the values for the sweep.
struct Names {
    prefix: String,
    variables: Vec<Variable>,
}

impl Names {
    /// `{{name}}` for `value` (named after `wanted`); a masked value gets a variable without one.
    fn variable(&mut self, wanted: &str, value: &str) -> String {
        let what = match slug(wanted, 40) {
            what if what.is_empty() => "secret".to_owned(),
            what => what,
        };
        let mut base = format!("{}-{what}", self.prefix);
        if base.starts_with(|c: char| c.is_ascii_digit()) {
            base = format!("host-{base}");
        }
        let value = (value != MASK && !value.is_empty()).then(|| value.to_owned());
        let mut name = base.clone();
        let mut n = 1;
        loop {
            match self.variables.iter().find(|known| known.name == name) {
                Some(known) if known.value == value => break,
                Some(_) => {
                    n += 1;
                    name = format!("{base}-{n}");
                }
                None => {
                    self.variables.push(Variable {
                        name: name.clone(),
                        value,
                    });
                    break;
                }
            }
        }
        format!("{{{{{name}}}}}")
    }

    /// `url` with every secret segment of its path ([`Names::path`]) and the value of every
    /// secret parameter of its query as a variable.
    fn url(&mut self, url: &str) -> String {
        let (base, query) = match url.split_once('?') {
            Some((base, query)) => (base, Some(query)),
            None => (url, None),
        };
        let start = base.find("://").map_or(0, |at| at + 3);
        let base = match base[start..].find('/') {
            Some(at) => {
                let (head, path) = base.split_at(start + at);
                format!("{head}{}", self.path(path))
            }
            None => base.to_owned(),
        };
        let Some(query) = query else {
            return base;
        };
        let pairs: Vec<String> = query
            .split('&')
            .map(|pair| match pair.split_once('=') {
                Some((name, value)) if is_secret_param(&form_unescape(name)) => {
                    format!("{name}={}", self.variable(name, value))
                }
                _ => pair.to_owned(),
            })
            .collect();
        format!("{base}?{}", pairs.join("&"))
    }

    fn header(&mut self, name: &str, value: &str) -> String {
        if name.eq_ignore_ascii_case("referer") {
            return self.url(value);
        }
        if !is_secret_header(name) {
            return value.to_owned();
        }
        let authorization = name.eq_ignore_ascii_case("authorization")
            || name.eq_ignore_ascii_case("proxy-authorization");
        match value.trim().split_once(' ') {
            Some((scheme, credential)) if authorization => {
                let wanted = if scheme.eq_ignore_ascii_case("bearer") {
                    "token"
                } else {
                    scheme
                };
                format!("{scheme} {}", self.variable(wanted, credential.trim()))
            }
            _ => self.variable(name, value.trim()),
        }
    }

    /// `text` with every known value of [`MIN_SWEPT_CHARS`] or more as its variable.
    fn sweep(&self, text: &mut String) {
        let mut known: Vec<(&str, &str)> = self
            .variables
            .iter()
            .filter_map(|variable| Some((variable.value.as_deref()?, variable.name.as_str())))
            .filter(|(value, _)| value.chars().count() >= MIN_SWEPT_CHARS)
            .collect();
        known.sort_by_key(|entry| std::cmp::Reverse(entry.0.len()));
        for (value, name) in known {
            if text.contains(value) {
                *text = text.replace(value, &format!("{{{{{name}}}}}"));
            }
        }
    }
}

/// The path of `url` (from its authority on, without the query): `/` for none.
fn path_of(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let rest = rest.split(['?', '#']).next().unwrap_or_default();
    match rest.find('/') {
        Some(at) => rest[at..].to_owned(),
        None => "/".to_owned(),
    }
}

/// `captured` as the API client takes it.
///
/// # Errors
///
/// [`Refused::NotHttp`] for anything but an `http(s)` request, [`Refused::Template`] for one
/// that holds a `{{` the capture did not make.
pub fn convert(captured: &Captured) -> Result<Converted, Refused> {
    convert_in(captured, &folder_of)
}

/// [`convert`] with the folder (and variables' prefix) of an authority chosen by `folder`.
///
/// # Errors
///
/// [`convert`]'s.
pub fn convert_in(
    captured: &Captured,
    folder: &dyn Fn(&str) -> String,
) -> Result<Converted, Refused> {
    super::grants::host_of(&captured.url).ok_or(Refused::NotHttp)?;
    let url = rb_util::url::without_user_info(&captured.url);
    let url = url.split('#').next().unwrap_or_default().to_owned();
    let authority = rb_util::url::authority_of(&url)
        .ok_or(Refused::NotHttp)?
        .to_owned();
    let mut names = Names {
        prefix: folder(&authority),
        variables: Vec::new(),
    };
    let method = captured.method.trim().to_ascii_uppercase();
    let mut request = ApiRequest::new(&method, &names.url(&url));
    request.name = rb_util::text::cut(&format!("{method} {}", path_of(&request.url)), 120);
    let mut content_type = String::new();
    for (name, value) in &captured.headers {
        let lower = name.to_ascii_lowercase();
        if lower.starts_with(':')
            || lower.starts_with("sec-")
            || NOT_TAKEN.contains(&lower.as_str())
        {
            continue;
        }
        if lower == "content-type" {
            content_type.clone_from(value);
        }
        let value = names.header(name, value);
        request.headers.push(Entry::new(name.clone(), value));
    }
    let mut body_missing = None;
    if let Some(text) = captured.body.as_deref().filter(|text| !text.is_empty()) {
        let made = names.variables.len();
        if text.chars().count() > MAX_BODY_CHARS {
            body_missing = Some(BodyMissing::TooLong);
        } else if let Some(body) = names.body(&content_type, text) {
            request.body = body;
        } else {
            names.variables.truncate(made);
            body_missing = Some(BodyMissing::Unchecked);
        }
    } else if captured.body_unread {
        body_missing = Some(BodyMissing::Unread);
    }
    names.sweep(&mut request.url);
    names.sweep(&mut request.name);
    for header in &mut request.headers {
        names.sweep(&mut header.value);
    }
    match &mut request.body {
        Body::Json { text } | Body::Raw { text } => names.sweep(text),
        Body::Form { fields } => {
            for field in fields {
                names.sweep(&mut field.value);
            }
        }
        Body::Multipart { parts } => {
            for part in parts {
                names.sweep(&mut part.value);
            }
        }
        _ => {}
    }
    if guard::foreign_template(&request, &names.variables) {
        return Err(Refused::Template);
    }
    let cut = captured.url.chars().count() > CAPTURE_TEXT_CHARS && captured.url.ends_with("...");
    let incomplete = cut || request.url.contains(MASK);
    Ok(Converted {
        path: path_of(&request.url),
        request,
        folder: names.prefix.clone(),
        variables: names.variables,
        body_missing,
        incomplete,
    })
}

#[cfg(test)]
#[path = "capture_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "capture_security_tests.rs"]
mod security_tests;
