//! What several readers share: `:name` and `{name}` path parameters as variables, a text body by
//! its language or `Content-Type`, a file a body names, query parameters onto a URL.

use rb_api_model::vars::is_name;
use rb_api_model::{Body, Entry};

use crate::collect::Collector;

/// `url` with each `:name` path segment as the value given for it, or as `{{name}}`.
#[must_use]
pub fn path_params(url: &str, values: &[(String, String)]) -> String {
    let (base, query) = match url.find('?') {
        Some(at) => url.split_at(at),
        None => (url, ""),
    };
    let (prefix, path) = match base.find("://") {
        Some(at) => {
            let rest = &base[at + 3..];
            match rest.find('/') {
                Some(slash) => base.split_at(at + 3 + slash),
                None => (base, ""),
            }
        }
        None => ("", base),
    };
    let path: Vec<String> = path
        .split('/')
        .map(|segment| match segment.strip_prefix(':') {
            Some(name) if is_name(name) => values
                .iter()
                .find(|(key, value)| key == name && !value.is_empty())
                .map_or_else(|| format!("{{{{{name}}}}}"), |(_, value)| value.clone()),
            _ => segment.to_owned(),
        })
        .collect();
    format!("{prefix}{}{query}", path.join("/"))
}

/// `url` with each `{name}` of OpenAPI's form as `{{name}}`; `{{...}}` stays.
#[must_use]
pub fn brace_params(url: &str) -> String {
    let mut out = String::with_capacity(url.len() + 8);
    let mut rest = url;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open..];
        if after.starts_with("{{") {
            let end = after.find("}}").map_or(after.len(), |close| close + 2);
            out.push_str(&after[..end]);
            rest = &after[end..];
            continue;
        }
        match after.find('}') {
            Some(close) if is_name(&after[1..close]) => {
                out.push_str("{{");
                out.push_str(&after[1..close]);
                out.push_str("}}");
                rest = &after[close + 1..];
            }
            _ => {
                out.push('{');
                rest = &after[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// The `Content-Type` of `headers`, lower case.
#[must_use]
pub fn content_type(headers: &[Entry]) -> String {
    headers
        .iter()
        .find(|header| header.name.eq_ignore_ascii_case("Content-Type"))
        .map(|header| header.value.to_ascii_lowercase())
        .unwrap_or_default()
}

/// A text body: JSON when `language` or the `Content-Type` says so, else raw - with the
/// `Content-Type` the language names added when the headers name none.
pub fn text_body(text: String, language: &str, headers: &mut Vec<Entry>) -> Body {
    if text.trim().is_empty() {
        return Body::None;
    }
    let language = language.to_ascii_lowercase();
    let named = content_type(headers);
    if language == "json" || (language.is_empty() && named.contains("json")) {
        return Body::Json { text };
    }
    let implied = match language.as_str() {
        "xml" => Some("application/xml"),
        "html" => Some("text/html"),
        "text" => Some("text/plain"),
        "javascript" => Some("application/javascript"),
        _ => None,
    };
    if let Some(implied) = implied
        && named.is_empty()
    {
        headers.push(Entry::new("Content-Type", implied));
    }
    Body::Raw { text }
}

/// A file a body names, as `./<file name>` beside the request's file - the import copies no
/// file; the report says which to put there.
pub fn local_file(path: &str, at: &str, out: &mut Collector) -> String {
    let name = path
        .rsplit(['/', '\\'])
        .find(|part| !part.trim().is_empty())
        .unwrap_or("file")
        .trim();
    let name: String = name
        .chars()
        .filter(|c| !c.is_control() && *c != ':')
        .collect();
    let name = if name.is_empty() {
        "file".to_owned()
    } else {
        name
    };
    out.reported(at, "file", &name);
    format!("./{name}")
}

/// `url` with `pairs` appended to its query (none of them encoded: they are templates).
#[must_use]
pub fn with_query(url: &str, pairs: &[(String, String)]) -> String {
    if pairs.is_empty() {
        return url.to_owned();
    }
    let joined: Vec<String> = pairs
        .iter()
        .map(|(name, value)| format!("{name}={value}"))
        .collect();
    let glue = if url.contains('?') { '&' } else { '?' };
    format!("{url}{glue}{}", joined.join("&"))
}

#[cfg(test)]
#[path = "common_tests.rs"]
mod tests;
