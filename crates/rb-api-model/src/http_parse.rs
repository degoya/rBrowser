//! A `.http` file read back into a request ([`crate::http_file`] has the form). It reads what
//! rBrowser writes and what a person or another tool wrote: `# @name` as the title when `###`
//! names none, `//` comments, a request line without a method (`GET`), `?`/`&` query lines under
//! it, comments between headers, a body kind inferred from `Content-Type` when no `@rb-body`
//! names it. An `Authorization` header becomes the auth only under `# @rb-auth basic|bearer`.
//! [`split_requests`] cuts a file of several requests for the importers (RB-096-02).

use crate::checks::Check;
use crate::http_file::{GRAPHQL_HEADER, GRAPHQL_VALUE, META, boundary_of};
use crate::request::{ApiRequest, Auth, Body, Entry, Part, is_token};

/// Why a file does not read, and on which line (1-based; 0 for the whole file).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    pub line: usize,
    pub reason: &'static str,
}

fn error(line: usize, reason: &'static str) -> ParseError {
    ParseError { line, reason }
}

/// The requests of a file of several, each from its `###` line on (text before the first one
/// that holds a request counts as one too).
#[must_use]
pub fn split_requests(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut current = String::new();
    for line in text.lines() {
        if line.trim_start().starts_with("###") && has_request(&current) {
            out.push(std::mem::take(&mut current));
        }
        current.push_str(line);
        current.push('\n');
    }
    if has_request(&current) {
        out.push(current);
    }
    out
}

fn has_request(text: &str) -> bool {
    text.lines().any(|line| {
        let line = line.trim();
        !line.is_empty() && !is_comment(line)
    })
}

fn is_comment(line: &str) -> bool {
    line.starts_with('#') || line.starts_with("//")
}

/// The comment's text without its marker.
fn comment_text(line: &str) -> &str {
    line.strip_prefix("//")
        .or_else(|| line.strip_prefix('#'))
        .unwrap_or(line)
        .trim()
}

/// What the comment lines say.
#[derive(Default)]
struct Meta {
    name: Option<String>,
    auth: Option<String>,
    body: Option<String>,
    no_redirect: bool,
    timeout_ms: Option<u64>,
    insecure: bool,
    off_headers: Vec<Entry>,
    off_fields: Vec<Entry>,
    comments: Vec<String>,
    checks: Vec<Check>,
}

impl Meta {
    fn take(&mut self, number: usize, line: &str) -> Result<(), ParseError> {
        let text = comment_text(line);
        if let Some(name) = text.strip_prefix("@name ") {
            self.name.get_or_insert_with(|| name.trim().to_owned());
        } else if text == "@no-redirect" {
            self.no_redirect = true;
        } else if let Some(rest) = text.strip_prefix(META) {
            self.take_own(number, rest)?;
        } else if !text.is_empty() {
            self.comments.push(text.to_owned());
        }
        Ok(())
    }

    fn take_own(&mut self, number: usize, rest: &str) -> Result<(), ParseError> {
        let (key, value) = rest.split_once(' ').unwrap_or((rest, ""));
        let value = value.trim();
        match key {
            "auth" => self.auth = Some(value.to_owned()),
            "body" => self.body = Some(value.to_owned()),
            "insecure" => self.insecure = true,
            "timeout-ms" => {
                let ms = value.parse().map_err(|_| error(number, "timeout"))?;
                self.timeout_ms = Some(ms);
            }
            "off" => {
                let (name, value) = value.split_once(':').ok_or(error(number, "header"))?;
                self.off_headers.push(Entry {
                    name: name.trim().to_owned(),
                    value: value.trim().to_owned(),
                    enabled: false,
                });
            }
            "off-field" => {
                let mut field = field_of(value);
                field.enabled = false;
                self.off_fields.push(field);
            }
            "check" => {
                let check = Check::from_line(value).map_err(|reason| error(number, reason))?;
                self.checks.push(check);
            }
            _ => self.comments.push(format!("{META}{rest}")),
        }
        Ok(())
    }
}

/// `%XX` and `+` decoded; a broken escape stays as written.
#[must_use]
pub fn form_unescape(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
                match hex.and_then(|hex| u8::from_str_radix(hex, 16).ok()) {
                    Some(byte) => {
                        out.push(byte);
                        i += 3;
                        continue;
                    }
                    None => out.push(b'%'),
                }
            }
            other => out.push(other),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn field_of(pair: &str) -> Entry {
    let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
    Entry::new(form_unescape(name.trim()), form_unescape(value.trim()))
}

/// The one request of `text`.
///
/// # Errors
///
/// No request line, a second request, a method that is no token, an `@rb-*` value that does not
/// read, a header line without `:`.
pub fn parse_http(text: &str) -> Result<ApiRequest, ParseError> {
    if split_requests(text).len() > 1 {
        return Err(error(0, "more than one request"));
    }
    let lines: Vec<&str> = text.lines().collect();
    let mut meta = Meta::default();
    let mut index = 0;
    let mut request = None;
    while index < lines.len() {
        let line = lines[index].trim();
        index += 1;
        if line.is_empty() {
            continue;
        }
        if let Some(title) = line.strip_prefix("###") {
            if !title.trim().is_empty() {
                meta.name = Some(title.trim().to_owned());
            }
            continue;
        }
        if is_comment(line) {
            meta.take(index, line)?;
            continue;
        }
        request = Some(request_line(index, line)?);
        break;
    }
    let (method, mut url) = request.ok_or(error(0, "no request line"))?;
    while index < lines.len() {
        let line = lines[index].trim();
        if !(line.starts_with('?') || line.starts_with('&')) {
            break;
        }
        url.push_str(line);
        index += 1;
    }
    let mut headers = Vec::new();
    while index < lines.len() {
        let line = lines[index].trim();
        index += 1;
        if line.is_empty() {
            break;
        }
        if is_comment(line) {
            meta.take(index, line)?;
            continue;
        }
        let (name, value) = line.split_once(':').ok_or(error(index, "header"))?;
        if !is_token(name.trim()) {
            return Err(error(index, "header"));
        }
        headers.push(Entry::new(name.trim(), value.trim()));
    }
    let body_lines = &lines[index.min(lines.len())..];
    let body = body_lines.join("\n").trim_end().to_owned();
    build(meta, method, url, headers, &body)
}

fn request_line(number: usize, line: &str) -> Result<(String, String), ParseError> {
    let mut parts = line.split_whitespace();
    let first = parts.next().unwrap_or_default();
    if first.contains("://") || first.starts_with("{{") || first.starts_with('/') {
        return Ok(("GET".to_owned(), first.to_owned()));
    }
    if !is_token(first) {
        return Err(error(number, "method"));
    }
    let url = parts.next().ok_or(error(number, "url"))?;
    Ok((first.to_ascii_uppercase(), url.to_owned()))
}

fn build(
    meta: Meta,
    method: String,
    url: String,
    mut headers: Vec<Entry>,
    body: &str,
) -> Result<ApiRequest, ParseError> {
    let auth = match meta.auth.as_deref() {
        None | Some("") => Auth::None,
        Some("basic" | "bearer") => header_auth(&mut headers, meta.auth.as_deref()),
        Some(json) => serde_json::from_str(json).map_err(|_| error(0, "auth"))?,
    };
    let graphql = take_header(&mut headers, GRAPHQL_HEADER)
        .is_some_and(|value| value.eq_ignore_ascii_case(GRAPHQL_VALUE));
    let kind = match meta.body.as_deref() {
        Some(kind) => kind.to_owned(),
        None if body.is_empty() => "none".to_owned(),
        None if graphql => "graphql".to_owned(),
        None => inferred(&headers, body).to_owned(),
    };
    let content_type = headers
        .iter()
        .find(|header| header.name.eq_ignore_ascii_case("Content-Type"))
        .map(|header| header.value.clone());
    let body = match kind.as_str() {
        "none" => Body::None,
        "json" => Body::Json { text: body.into() },
        "raw" => Body::Raw { text: body.into() },
        "form" => {
            let line: String = body.lines().map(str::trim).collect();
            let mut fields: Vec<Entry> = line
                .split('&')
                .filter(|pair| !pair.is_empty())
                .map(field_of)
                .collect();
            fields.extend(meta.off_fields);
            Body::Form { fields }
        }
        "binary" => Body::Binary {
            path: body
                .trim()
                .strip_prefix('<')
                .unwrap_or(body)
                .trim()
                .to_owned(),
        },
        "graphql" => graphql_body(body),
        "multipart" => {
            let boundary = content_type.as_deref().and_then(boundary_of);
            Body::Multipart {
                parts: parts_of(body, boundary.as_deref()).ok_or(error(0, "multipart"))?,
            }
        }
        _ => return Err(error(0, "body kind")),
    };
    headers.extend(meta.off_headers);
    let mut request = ApiRequest {
        name: meta.name.unwrap_or_default(),
        method,
        url,
        headers,
        body,
        auth,
        comments: meta.comments,
        checks: meta.checks,
        ..ApiRequest::default()
    };
    request.options.follow_redirects = !meta.no_redirect;
    request.options.verify_tls = !meta.insecure;
    if let Some(ms) = meta.timeout_ms {
        request.options.timeout_ms = ms;
    }
    Ok(request)
}

fn take_header(headers: &mut Vec<Entry>, name: &str) -> Option<String> {
    let at = headers
        .iter()
        .position(|header| header.name.eq_ignore_ascii_case(name))?;
    Some(headers.remove(at).value)
}

fn header_auth(headers: &mut Vec<Entry>, kind: Option<&str>) -> Auth {
    let Some(at) = headers
        .iter()
        .position(|header| header.name.eq_ignore_ascii_case("Authorization"))
    else {
        return Auth::None;
    };
    let value = headers[at].value.clone();
    let (scheme, rest) = value.split_once(' ').unwrap_or((value.as_str(), ""));
    let rest = rest.trim();
    let auth = match kind {
        Some("bearer") if scheme.eq_ignore_ascii_case("Bearer") => Auth::Bearer {
            token: rest.to_owned(),
        },
        Some("basic") if scheme.eq_ignore_ascii_case("Basic") => {
            match rest.split_once(':').or_else(|| rest.split_once(' ')) {
                Some((username, password)) => Auth::Basic {
                    username: username.to_owned(),
                    password: password.to_owned(),
                },
                None => return Auth::None,
            }
        }
        _ => return Auth::None,
    };
    headers.remove(at);
    auth
}

fn inferred(headers: &[Entry], body: &str) -> &'static str {
    let content_type = headers
        .iter()
        .find(|header| header.name.eq_ignore_ascii_case("Content-Type"))
        .map(|header| header.value.to_ascii_lowercase())
        .unwrap_or_default();
    if content_type.contains("json") {
        "json"
    } else if content_type.starts_with("application/x-www-form-urlencoded") {
        "form"
    } else if content_type.starts_with("multipart/form-data") {
        "multipart"
    } else if body.lines().count() == 1 && body.starts_with("< ") {
        "binary"
    } else {
        "raw"
    }
}

fn graphql_body(body: &str) -> Body {
    if let Some((query, variables)) = body.rsplit_once("\n\n")
        && serde_json::from_str::<serde_json::Value>(variables.trim()).is_ok_and(|v| v.is_object())
    {
        return Body::GraphQl {
            query: query.trim_end().to_owned(),
            variables: variables.trim().to_owned(),
        };
    }
    Body::GraphQl {
        query: body.to_owned(),
        variables: String::new(),
    }
}

fn parts_of(body: &str, boundary: Option<&str>) -> Option<Vec<Part>> {
    let boundary = boundary?;
    let open = format!("--{boundary}");
    let mut parts = Vec::new();
    for section in body.split(open.as_str()).skip(1) {
        if section.starts_with("--") {
            break;
        }
        let section = section.trim_start_matches(['\r', '\n']);
        let (head, value) = section.split_once("\n\n").unwrap_or((section, ""));
        let mut part = Part {
            name: String::new(),
            value: String::new(),
            file: None,
            content_type: None,
        };
        for line in head.lines() {
            let (key, rest) = line.split_once(':')?;
            if key.trim().eq_ignore_ascii_case("Content-Type") {
                part.content_type = Some(rest.trim().to_owned());
            } else if key.trim().eq_ignore_ascii_case("Content-Disposition") {
                part.name = param(rest, "name")?;
            }
        }
        let value = value.trim_end_matches(['\r', '\n']);
        match value.trim().strip_prefix("< ") {
            Some(file) if !value.trim().contains('\n') => part.file = Some(file.trim().to_owned()),
            _ => part.value = value.to_owned(),
        }
        parts.push(part);
    }
    Some(parts)
}

fn param(disposition: &str, key: &str) -> Option<String> {
    disposition.split(';').skip(1).find_map(|item| {
        let (name, value) = item.split_once('=')?;
        (name.trim() == key).then(|| value.trim().trim_matches('"').to_owned())
    })
}

#[cfg(test)]
#[path = "http_parse_tests.rs"]
mod tests;
