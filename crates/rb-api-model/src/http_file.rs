//! A request as its `.http` file: one request per file below `.rbrowser/api/`, in the form VS
//! Code REST Client and the JetBrains HTTP Client run as it is. What neither knows goes into
//! `# @rb-*` comment lines they skip:
//!
//! ```text
//! ### Create user                      the title
//! # a note of the person               kept comments
//! # @rb-auth bearer                    basic and bearer: also the Authorization header below
//! # @rb-auth {"type":"apiKey",...}     the other kinds as JSON, not sent by other tools
//! # @rb-body json                      the body's kind
//! # @no-redirect                       redirects not followed (both tools know it)
//! # @rb-timeout-ms 5000                a timeout other than the default
//! # @rb-insecure                       the TLS certificate not checked
//! # @rb-off X-Debug: 1                 a disabled header
//! # @rb-off-field debug=1              a disabled form field
//! # @rb-check status 2xx               a check of the answer (RB-096-04, `crate::checks`)
//! POST {{baseUrl}}/users
//! Content-Type: application/json
//! Authorization: Bearer {{token}}
//!
//! {"name": "Ada"}
//! ```
//!
//! A form body is one line of `name=value` pairs (`%`, `&`, `=`, `+`, `#`, spaces and line
//! breaks escaped; `{{variables}}` kept), a multipart body REST Client's boundary form with
//! `< ./file` for a file, a binary body `< ./file`, a GraphQL body the query, a blank line and
//! its variables, under `X-REQUEST-TYPE: GraphQL` (REST Client's marker, never sent).

use crate::request::{ApiRequest, Auth, Body, DEFAULT_TIMEOUT_MS, Entry, Part};

/// The marker of the `# @rb-*` lines.
pub const META: &str = "@rb-";
/// REST Client's GraphQL marker header.
pub const GRAPHQL_HEADER: &str = "X-REQUEST-TYPE";
/// Its value.
pub const GRAPHQL_VALUE: &str = "GraphQL";
/// The boundary written when the request names none.
pub const BOUNDARY: &str = "rbrowser-part";

/// `text` escaped for a form body line.
#[must_use]
pub fn form_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '%' => out.push_str("%25"),
            '&' => out.push_str("%26"),
            '=' => out.push_str("%3D"),
            '+' => out.push_str("%2B"),
            '#' => out.push_str("%23"),
            ' ' => out.push_str("%20"),
            '\r' => out.push_str("%0D"),
            '\n' => out.push_str("%0A"),
            other => out.push(other),
        }
    }
    out
}

/// `text` form-encoded as a query or a form body goes out: unreserved characters as they are, a
/// space as `+`, every other byte as `%XX`.
#[must_use]
pub fn form_encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(char::from(byte));
            }
            b' ' => out.push('+'),
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

/// The boundary of a `multipart/form-data` content type, if it names one.
#[must_use]
pub fn boundary_of(content_type: &str) -> Option<String> {
    content_type.split(';').skip(1).find_map(|param| {
        let (key, value) = param.split_once('=')?;
        key.trim()
            .eq_ignore_ascii_case("boundary")
            .then(|| value.trim().trim_matches('"').to_owned())
            .filter(|boundary| !boundary.is_empty())
    })
}

/// Whether `line` reads as a file include of REST Client, the JetBrains HTTP Client or
/// [`crate::http_parse`] (`< ./file`, `<@ ./file`, `<@latin1 ./file`), or starts a path right
/// after the `<` (`<./file`, `<~/file`). Markup (`<div>`, `</div>`, `<?xml`) does not.
#[must_use]
pub fn is_include_line(line: &str) -> bool {
    let Some(rest) = line.trim_start().strip_prefix('<') else {
        return false;
    };
    let rest = match rest.strip_prefix('@') {
        Some(encoding) => {
            encoding.trim_start_matches(|c: char| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        }
        None => rest,
    };
    rest.starts_with(|c: char| c.is_whitespace() || matches!(c, '.' | '~' | '\\'))
}

/// Whether a line of `text` reads as a file include ([`is_include_line`]).
#[must_use]
pub fn holds_include(text: &str) -> bool {
    text.lines().any(is_include_line)
}

/// `text` with a `\` before the `<` of every line that would read as a file include: a text
/// part's value never becomes a file another tool or this parser sends (the w31 security review).
fn without_includes(text: &str) -> String {
    if !holds_include(text) {
        return text.to_owned();
    }
    text.split('\n')
        .map(|line| {
            if is_include_line(line) {
                line.replacen('<', "\\<", 1)
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The `.http` text of `request`.
#[must_use]
pub fn to_http(request: &ApiRequest) -> String {
    let mut out = String::new();
    out.push_str("###");
    if !request.name.trim().is_empty() {
        out.push(' ');
        out.push_str(request.name.trim());
    }
    out.push('\n');
    for comment in &request.comments {
        out.push_str("# ");
        out.push_str(comment);
        out.push('\n');
    }
    meta_lines(request, &mut out);
    out.push_str(&request.method);
    out.push(' ');
    out.push_str(request.url.trim());
    out.push('\n');
    for header in header_lines(request) {
        out.push_str(&header);
        out.push('\n');
    }
    if let Some(body) = body_text(request) {
        out.push('\n');
        out.push_str(&body);
        if !body.ends_with('\n') {
            out.push('\n');
        }
    }
    out
}

fn meta_lines(request: &ApiRequest, out: &mut String) {
    let mut meta = |line: &str| {
        out.push_str("# ");
        out.push_str(line);
        out.push('\n');
    };
    match &request.auth {
        Auth::None => {}
        Auth::Basic { .. } => meta("@rb-auth basic"),
        Auth::Bearer { .. } => meta("@rb-auth bearer"),
        other => {
            let json = serde_json::to_string(other).unwrap_or_default();
            meta(&format!("@rb-auth {json}"));
        }
    }
    if !matches!(request.body, Body::None) {
        meta(&format!("@rb-body {}", request.body.kind()));
    }
    if !request.options.follow_redirects {
        meta("@no-redirect");
    }
    if request.options.timeout_ms != DEFAULT_TIMEOUT_MS {
        meta(&format!("@rb-timeout-ms {}", request.options.timeout_ms));
    }
    if !request.options.verify_tls {
        meta("@rb-insecure");
    }
    for header in request.headers.iter().filter(|header| !header.enabled) {
        meta(&format!("@rb-off {}: {}", header.name, header.value));
    }
    if let Body::Form { fields } = &request.body {
        for field in fields.iter().filter(|field| !field.enabled) {
            meta(&format!("@rb-off-field {}", pair(field)));
        }
    }
    for check in &request.checks {
        meta(&format!("@rb-check {}", check.to_line()));
    }
}

fn pair(field: &Entry) -> String {
    format!("{}={}", form_escape(&field.name), form_escape(&field.value))
}

fn header_lines(request: &ApiRequest) -> Vec<String> {
    let mut lines: Vec<String> = request
        .headers
        .iter()
        .filter(|header| header.enabled)
        .map(|header| format!("{}: {}", header.name, header.value))
        .collect();
    match &request.auth {
        Auth::Basic { username, password } => {
            lines.push(format!("Authorization: Basic {username}:{password}"));
        }
        Auth::Bearer { token } => lines.push(format!("Authorization: Bearer {token}")),
        _ => {}
    }
    match &request.body {
        Body::Multipart { .. } if request.header("Content-Type").is_none() => {
            lines.push(format!(
                "Content-Type: multipart/form-data; boundary={BOUNDARY}"
            ));
        }
        Body::GraphQl { .. } => lines.push(format!("{GRAPHQL_HEADER}: {GRAPHQL_VALUE}")),
        _ => {}
    }
    lines
}

fn body_text(request: &ApiRequest) -> Option<String> {
    match &request.body {
        Body::None => None,
        Body::Json { text } | Body::Raw { text } => Some(text.clone()),
        Body::Form { fields } => Some(
            fields
                .iter()
                .filter(|field| field.enabled)
                .map(pair)
                .collect::<Vec<_>>()
                .join("&"),
        ),
        Body::Binary { path } => Some(format!("< {path}")),
        Body::GraphQl { query, variables } => {
            let query = query.trim_end();
            Some(if variables.trim().is_empty() {
                query.to_owned()
            } else {
                format!("{query}\n\n{}", variables.trim())
            })
        }
        Body::Multipart { parts } => {
            let boundary = request
                .header("Content-Type")
                .and_then(boundary_of)
                .unwrap_or_else(|| BOUNDARY.to_owned());
            Some(multipart_text(parts, &boundary))
        }
    }
}

fn multipart_text(parts: &[Part], boundary: &str) -> String {
    let mut out = String::new();
    for part in parts {
        out.push_str(&format!("--{boundary}\n"));
        out.push_str(&format!(
            "Content-Disposition: form-data; name=\"{}\"",
            part.name
        ));
        if let Some(file) = &part.file {
            let name = file.rsplit(['/', '\\']).next().unwrap_or(file);
            out.push_str(&format!("; filename=\"{}\"", name.replace('"', "")));
        }
        out.push('\n');
        if let Some(content_type) = &part.content_type {
            out.push_str(&format!("Content-Type: {content_type}\n"));
        }
        out.push('\n');
        match &part.file {
            Some(file) => out.push_str(&format!("< {file}\n")),
            None => {
                out.push_str(&without_includes(&part.value));
                out.push('\n');
            }
        }
    }
    out.push_str(&format!("--{boundary}--\n"));
    out
}

#[cfg(test)]
#[path = "http_file_tests.rs"]
mod tests;
