//! The rewrites applied to a paused request or its response (RB-080-37): header operations
//! (set, add, remove), query parameters (set, remove), the map-remote URL swap with its `Host`
//! and `Origin` headers, and the body replacement by regular expression. Plain functions over
//! plain data; [`super::fetch`] and [`super::answer`] call them.

use super::action::{BodyReplace, Endpoint, HeaderOp, QueryOp};

/// `original` with `ops` applied in order.
#[must_use]
pub fn apply_headers(original: &[(String, String)], ops: &[HeaderOp]) -> Vec<(String, String)> {
    let mut headers: Vec<(String, String)> = original.to_vec();
    for op in ops {
        match op {
            HeaderOp::Set(name, value) => {
                headers.retain(|(known, _)| !known.eq_ignore_ascii_case(name));
                headers.push((name.clone(), value.clone()));
            }
            HeaderOp::Add(name, value) => headers.push((name.clone(), value.clone())),
            HeaderOp::Remove(name) => {
                headers.retain(|(known, _)| !known.eq_ignore_ascii_case(name));
            }
        }
    }
    headers
}

/// The characters of a query value that must be escaped.
fn encode_query(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'_'
            | b'.'
            | b'~'
            | b'/'
            | b':'
            | b',' => out.push(char::from(byte)),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// `url` with its query changed by `ops`; an empty query loses its `?`.
#[must_use]
pub fn apply_query(url: &str, ops: &[QueryOp]) -> String {
    let (without_fragment, fragment) = url
        .split_once('#')
        .map_or((url, None), |(base, fragment)| (base, Some(fragment)));
    let (base, query) = without_fragment
        .split_once('?')
        .map_or((without_fragment, ""), |(base, query)| (base, query));
    let mut pairs: Vec<(String, String)> = query
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| {
            let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
            (name.to_owned(), value.to_owned())
        })
        .collect();
    for op in ops {
        match op {
            QueryOp::Set(name, value) => {
                let encoded = encode_query(value);
                match pairs.iter().position(|(known, _)| known == name) {
                    Some(first) => {
                        pairs[first].1 = encoded;
                        let mut index = 0;
                        pairs.retain(|(known, _)| {
                            let keep = known != name || index == first;
                            index += 1;
                            keep
                        });
                    }
                    None => pairs.push((name.clone(), encoded)),
                }
            }
            QueryOp::Remove(name) => pairs.retain(|(known, _)| known != name),
        }
    }
    let mut out = base.to_owned();
    if !pairs.is_empty() {
        out.push('?');
        let joined: Vec<String> = pairs
            .iter()
            .map(|(name, value)| {
                if value.is_empty() {
                    name.clone()
                } else {
                    format!("{name}={value}")
                }
            })
            .collect();
        out.push_str(&joined.join("&"));
    }
    if let Some(fragment) = fragment {
        out.push('#');
        out.push_str(fragment);
    }
    out
}

/// The scheme, host, port (explicit or the scheme's) and the rest of an `http(s)` URL.
fn split_url(url: &str) -> Option<(&str, String, u16, &str)> {
    let (scheme, rest) = url.split_once("://")?;
    if scheme != "http" && scheme != "https" {
        return None;
    }
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let (authority, tail) = rest.split_at(end);
    // `[::1]:8080` keeps its brackets; `host:port` splits at the last colon.
    let split = match authority.rfind(']') {
        Some(close) => authority[close..]
            .find(':')
            .map(|at| authority.split_at(close + at)),
        None => authority
            .rsplit_once(':')
            .map(|(host, _)| authority.split_at(host.len())),
    };
    let (host, port) = match split {
        Some((host, port)) => match port.trim_start_matches(':').parse::<u16>() {
            Ok(port) => (host, port),
            Err(_) => (authority, default_port(scheme)),
        },
        None => (authority, default_port(scheme)),
    };
    Some((scheme, host.to_ascii_lowercase(), port, tail))
}

fn default_port(scheme: &str) -> u16 {
    if scheme == "https" { 443 } else { 80 }
}

/// `url` mapped from `from` to `to`, or `None` when `from` does not cover it: the scheme, the
/// port and the path prefix when `from` names them, the host (case-blind) always; `to` replaces
/// what it names, the rest stays.
#[must_use]
pub fn map_remote(url: &str, from: &Endpoint, to: &Endpoint) -> Option<String> {
    let (scheme, host, port, tail) = split_url(url)?;
    if from
        .scheme
        .as_deref()
        .is_some_and(|wanted| wanted != scheme)
        || host != from.host
    {
        return None;
    }
    if from.port.is_some_and(|wanted| wanted != port) {
        return None;
    }
    let path_end = tail.find(['?', '#']).unwrap_or(tail.len());
    let (path, after) = tail.split_at(path_end);
    let path = if path.is_empty() { "/" } else { path };
    let rest = match &from.path {
        Some(prefix) => path
            .strip_prefix(prefix.as_str())
            .filter(|rest| rest.is_empty() || rest.starts_with('/') || prefix.ends_with('/'))?,
        None => path,
    };
    let new_scheme = to.scheme.as_deref().unwrap_or(scheme);
    let mut out = format!("{new_scheme}://{}", to.host);
    if let Some(port) = to.port {
        out.push_str(&format!(":{port}"));
    }
    match &to.path {
        Some(prefix) => {
            out.push_str(prefix);
            out.push_str(rest);
        }
        None => out.push_str(path),
    }
    out.push_str(after);
    Some(out)
}

/// The `Host` header of requests to `to`.
#[must_use]
pub fn host_header(to: &Endpoint) -> String {
    match to.port {
        Some(port) => format!("{}:{port}", to.host),
        None => to.host.clone(),
    }
}

/// The `Origin` of `url`: scheme, host and explicit port.
#[must_use]
pub fn origin_header(url: &str) -> Option<String> {
    rb_util::url::origin_of(url)
}

/// `body` with `replace` applied, when it is text and something matched.
#[must_use]
pub fn replace_body(body: &[u8], replace: &BodyReplace) -> Option<Vec<u8>> {
    let text = std::str::from_utf8(body).ok()?;
    if !replace.regex.is_match(text) {
        return None;
    }
    let replaced = replace
        .regex
        .replace_all(text, replace.replacement.as_str());
    Some(replaced.into_owned().into_bytes())
}

#[cfg(test)]
#[path = "rewrite_tests.rs"]
mod tests;
