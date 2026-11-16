//! What a matching request gets (RB-070-02, RB-080-37): `mock`, `block`, `rewrite` - the
//! request's URL and headers, its query, the response's headers and body - `mapLocal` (a file
//! below a folder the person allowed) and `mapRemote` (another host). Parsed here from a rule's
//! `action`; applied by [`super::fetch`], [`super::rewrite`] and [`super::local`].

use base64::Engine as _;
use regex::{Regex, RegexBuilder};
use serde_json::Value;

use super::rule::{
    MAX_BODY_BASE64, MAX_DELAY_MS, MAX_HEADER_VALUE, MAX_HEADERS, MAX_PATTERN_CHARS,
};

/// The largest compiled regular expression of a body replacement.
const REGEX_SIZE_LIMIT: usize = 1024 * 1024;
/// The longest replacement text.
const MAX_REPLACEMENT: usize = 64 * 1024;
/// The longest path of a map-local folder or file.
const MAX_PATH: usize = 1024;

/// One change of a header list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HeaderOp {
    /// Replace every header of that name (case-blind) by one, or add it.
    Set(String, String),
    /// Add one, whatever is there.
    Add(String, String),
    /// Drop every header of that name.
    Remove(String),
}

impl HeaderOp {
    /// The header's name.
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Self::Set(name, _) | Self::Add(name, _) | Self::Remove(name) => name,
        }
    }
}

/// One change of a URL's query.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QueryOp {
    /// Set the parameter (the first of that name replaced, the others dropped; added when none).
    Set(String, String),
    /// Drop every parameter of that name.
    Remove(String),
}

/// A replacement in the response body.
#[derive(Clone, Debug)]
pub struct BodyReplace {
    pub regex: Regex,
    /// `$1` and `$name` as `regex` reads them.
    pub replacement: String,
}

impl PartialEq for BodyReplace {
    fn eq(&self, other: &Self) -> bool {
        self.regex.as_str() == other.regex.as_str() && self.replacement == other.replacement
    }
}

impl Eq for BodyReplace {}

/// `Fetch.continueRequest` with changes, and for the response parts `Fetch.fulfillRequest` at
/// the response stage.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Rewrite {
    pub url: Option<String>,
    pub headers: Vec<HeaderOp>,
    pub response_headers: Vec<HeaderOp>,
    pub query: Vec<QueryOp>,
    pub body: Option<BodyReplace>,
}

impl Rewrite {
    /// Whether it changes the request (URL, query or request headers).
    #[must_use]
    pub fn acts_on_request(&self) -> bool {
        self.url.is_some() || !self.headers.is_empty() || !self.query.is_empty()
    }

    /// Whether it changes the response (headers or body): it needs the response stage.
    #[must_use]
    pub fn acts_on_response(&self) -> bool {
        !self.response_headers.is_empty() || self.body.is_some()
    }
}

/// A folder (the URL's path below it) or one file served instead of the request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MapLocal {
    /// As the rule names it: absolute, or relative to the project; resolved and bounded by
    /// [`super::local`] before the rule applies.
    pub folder: Option<String>,
    pub file: Option<String>,
    /// The MIME type; by extension when none.
    pub mime: Option<String>,
    /// The canonical path of the folder or file, once [`super::state`] resolved it against
    /// the allowed roots; a rule without it does not apply.
    pub resolved: Option<std::path::PathBuf>,
}

/// One side of a map-remote rule.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Endpoint {
    pub scheme: Option<String>,
    pub host: String,
    pub port: Option<u16>,
    /// A path prefix starting with `/`.
    pub path: Option<String>,
}

/// What a matching request gets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    /// A response of the host's: `Fetch.fulfillRequest` after `delay_ms`.
    Mock {
        status: u16,
        headers: Vec<(String, String)>,
        body_base64: String,
        delay_ms: u64,
    },
    /// `Fetch.failRequest` with `BlockedByClient`.
    Block,
    /// Changes to the request and/or the response.
    Rewrite(Rewrite),
    /// A local file instead of the server's answer.
    MapLocal(MapLocal),
    /// Host, port and path prefix swapped; `Host` and `Origin` follow.
    MapRemote { from: Endpoint, to: Endpoint },
}

/// The action of a rule's `action` value.
///
/// # Errors
///
/// What does not read, in a few words.
pub fn parse(raw: &Value) -> Result<Action, &'static str> {
    match raw["type"].as_str() {
        Some("block") => Ok(Action::Block),
        Some("mock") => mock(raw),
        Some("rewrite") => rewrite(raw),
        Some("mapLocal") => map_local(raw),
        Some("mapRemote") => Ok(Action::MapRemote {
            from: endpoint(&raw["from"])?,
            to: endpoint(&raw["to"])?,
        }),
        _ => Err("bad action"),
    }
}

fn mock(raw: &Value) -> Result<Action, &'static str> {
    let status = raw["status"]
        .as_u64()
        .filter(|status| (100..=599).contains(status))
        .and_then(|status| u16::try_from(status).ok())
        .ok_or("bad status")?;
    let body_base64 = match &raw["bodyBase64"] {
        Value::Null => String::new(),
        Value::String(body) if body.len() <= MAX_BODY_BASE64 => {
            base64::engine::general_purpose::STANDARD
                .decode(body)
                .map_err(|_| "bad bodyBase64")?;
            body.clone()
        }
        _ => return Err("bad bodyBase64"),
    };
    let delay_ms = match &raw["delayMs"] {
        Value::Null => 0,
        value => value
            .as_u64()
            .filter(|delay| *delay <= MAX_DELAY_MS)
            .ok_or("bad delayMs")?,
    };
    Ok(Action::Mock {
        status,
        headers: headers(&raw["headers"])?,
        body_base64,
        delay_ms,
    })
}

fn rewrite(raw: &Value) -> Result<Action, &'static str> {
    let url = match &raw["url"] {
        Value::Null => None,
        Value::String(url) if web_url(url) => Some(url.clone()),
        _ => return Err("bad rewrite url"),
    };
    let mut ops: Vec<HeaderOp> = headers(&raw["headers"])?
        .into_iter()
        .map(|(name, value)| HeaderOp::Set(name, value))
        .collect();
    ops.extend(header_ops(&raw["requestHeaders"])?);
    let rewrite = Rewrite {
        url,
        headers: ops,
        response_headers: header_ops(&raw["responseHeaders"])?,
        query: query_ops(&raw["query"])?,
        body: body_replace(&raw["body"])?,
    };
    if !rewrite.acts_on_request() && !rewrite.acts_on_response() {
        return Err("rewrite changes nothing");
    }
    Ok(Action::Rewrite(rewrite))
}

fn map_local(raw: &Value) -> Result<Action, &'static str> {
    let path = |key: &str| match &raw[key] {
        Value::Null => Ok(None),
        Value::String(path)
            if !path.is_empty()
                && path.len() <= MAX_PATH
                && !path.chars().any(char::is_control) =>
        {
            Ok(Some(path.clone()))
        }
        _ => Err("bad mapLocal path"),
    };
    let (folder, file) = (path("folder")?, path("file")?);
    if folder.is_some() == file.is_some() {
        return Err("mapLocal needs one of folder, file");
    }
    let mime = match &raw["mime"] {
        Value::Null => None,
        Value::String(mime) if mime_type(mime) => Some(mime.clone()),
        _ => return Err("bad mime"),
    };
    Ok(Action::MapLocal(MapLocal {
        folder,
        file,
        mime,
        resolved: None,
    }))
}

fn endpoint(raw: &Value) -> Result<Endpoint, &'static str> {
    if !raw.is_object() {
        return Err("bad endpoint");
    }
    let scheme = match &raw["scheme"] {
        Value::Null => None,
        Value::String(scheme) if scheme == "http" || scheme == "https" => Some(scheme.clone()),
        _ => return Err("bad scheme"),
    };
    let host = raw["host"]
        .as_str()
        .filter(|host| host_name(host))
        .ok_or("bad host")?
        .to_ascii_lowercase();
    let port = match &raw["port"] {
        Value::Null => None,
        value => Some(
            value
                .as_u64()
                .and_then(|port| u16::try_from(port).ok())
                .filter(|port| *port > 0)
                .ok_or("bad port")?,
        ),
    };
    let path = match &raw["path"] {
        Value::Null => None,
        Value::String(path)
            if path.starts_with('/')
                && path.len() <= MAX_PATTERN_CHARS
                && !path.chars().any(|c| c.is_whitespace() || c.is_control()) =>
        {
            Some(path.clone())
        }
        _ => return Err("bad path"),
    };
    Ok(Endpoint {
        scheme,
        host,
        port,
        path,
    })
}

/// `[[name, value], ...]`: token names, values without line breaks.
pub(super) fn headers(raw: &Value) -> Result<Vec<(String, String)>, &'static str> {
    let list = match raw {
        Value::Null => return Ok(Vec::new()),
        Value::Array(list) if list.len() <= MAX_HEADERS => list,
        _ => return Err("bad headers"),
    };
    list.iter()
        .map(|pair| match pair.as_array().map(Vec::as_slice) {
            Some([Value::String(name), Value::String(value)])
                if header_name(name) && header_value(value) =>
            {
                Ok((name.clone(), value.clone()))
            }
            _ => Err("bad header"),
        })
        .collect()
}

/// `[{ "op": "set" | "add" | "remove", "name", "value"? }, ...]`.
fn header_ops(raw: &Value) -> Result<Vec<HeaderOp>, &'static str> {
    let list = match raw {
        Value::Null => return Ok(Vec::new()),
        Value::Array(list) if list.len() <= MAX_HEADERS => list,
        _ => return Err("bad header operations"),
    };
    list.iter()
        .map(|entry| {
            let name = entry["name"]
                .as_str()
                .filter(|name| header_name(name))
                .ok_or("bad header name")?;
            let value = || {
                entry["value"]
                    .as_str()
                    .filter(|value| header_value(value))
                    .ok_or("bad header value")
            };
            match entry["op"].as_str() {
                Some("set") => Ok(HeaderOp::Set(name.to_owned(), value()?.to_owned())),
                Some("add") => Ok(HeaderOp::Add(name.to_owned(), value()?.to_owned())),
                Some("remove") => Ok(HeaderOp::Remove(name.to_owned())),
                _ => Err("bad header op"),
            }
        })
        .collect()
}

/// `[{ "op": "set" | "remove", "name", "value"? }, ...]`.
fn query_ops(raw: &Value) -> Result<Vec<QueryOp>, &'static str> {
    let list = match raw {
        Value::Null => return Ok(Vec::new()),
        Value::Array(list) if list.len() <= MAX_HEADERS => list,
        _ => return Err("bad query operations"),
    };
    let plain = |text: &str| {
        !text.is_empty() && text.len() <= MAX_HEADER_VALUE && !text.chars().any(char::is_control)
    };
    list.iter()
        .map(|entry| {
            let name = entry["name"]
                .as_str()
                .filter(|name| plain(name))
                .ok_or("bad query name")?;
            match entry["op"].as_str() {
                Some("set") => {
                    let value = entry["value"]
                        .as_str()
                        .filter(|value| value.len() <= MAX_HEADER_VALUE && !has_control(value))
                        .ok_or("bad query value")?;
                    Ok(QueryOp::Set(name.to_owned(), value.to_owned()))
                }
                Some("remove") => Ok(QueryOp::Remove(name.to_owned())),
                _ => Err("bad query op"),
            }
        })
        .collect()
}

/// `{ "regex", "replacement" }`.
fn body_replace(raw: &Value) -> Result<Option<BodyReplace>, &'static str> {
    if raw.is_null() {
        return Ok(None);
    }
    let pattern = raw["regex"]
        .as_str()
        .filter(|pattern| !pattern.is_empty() && pattern.chars().count() <= MAX_PATTERN_CHARS)
        .ok_or("bad body regex")?;
    let replacement = raw["replacement"]
        .as_str()
        .filter(|text| text.len() <= MAX_REPLACEMENT)
        .ok_or("bad body replacement")?;
    let regex = RegexBuilder::new(pattern)
        .size_limit(REGEX_SIZE_LIMIT)
        .build()
        .map_err(|_| "bad body regex")?;
    Ok(Some(BodyReplace {
        regex,
        replacement: replacement.to_owned(),
    }))
}

pub(super) fn header_name(name: &str) -> bool {
    (1..=128).contains(&name.len())
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "!#$%&'*+-.^_`|~".contains(c))
}

pub(super) fn header_value(value: &str) -> bool {
    value.len() <= MAX_HEADER_VALUE && !has_control(value)
}

fn has_control(text: &str) -> bool {
    text.chars().any(|c| c.is_control())
}

/// `type/subtype`, printable ASCII.
fn mime_type(mime: &str) -> bool {
    (3..=128).contains(&mime.len())
        && mime.contains('/')
        && mime
            .chars()
            .all(|c| c.is_ascii_graphic() && c != '"' && c != '\\')
}

/// A host name or address: letters, digits, `.`, `-`, `:` (IPv6 in brackets), no whitespace.
fn host_name(host: &str) -> bool {
    (1..=253).contains(&host.len())
        && host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-.:[]_".contains(c))
}

/// An `http(s)` URL without whitespace or control characters.
pub(super) fn web_url(url: &str) -> bool {
    (url.starts_with("http://") || url.starts_with("https://"))
        && url.chars().count() <= MAX_PATTERN_CHARS
        && !url.chars().any(|c| c.is_whitespace() || c.is_control())
}

#[cfg(test)]
#[path = "action_tests.rs"]
mod tests;
