//! What every language sends alike, as the core's sender does (`api_client::send::prepare`):
//! the enabled headers, the bearer token and a header API key as headers, a query API key on
//! the URL, `application/json` for a JSON or GraphQL body without a `Content-Type`, no
//! multipart type of the request's own (each tool writes the boundary it sends); basic and
//! digest credentials apart, for each language's own way.

use rb_api_model::{ApiRequest, Auth, Body, KeyPlace};

use crate::Note;
use crate::template::encode;

/// User and password of basic or digest auth.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Credentials {
    Basic(String, String),
    Digest(String, String),
}

/// A request's URL, headers and credentials as they go out.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Shape {
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub credentials: Option<Credentials>,
}

/// The shape of `request`; an OAuth 2 auth adds [`Note::OAuth2`].
pub(crate) fn shape(request: &ApiRequest, notes: &mut Vec<Note>) -> Shape {
    let multipart = matches!(request.body, Body::Multipart { .. });
    let mut headers: Vec<(String, String)> = request
        .headers
        .iter()
        .filter(|header| header.enabled)
        .filter(|header| {
            !(multipart
                && header.name.eq_ignore_ascii_case("Content-Type")
                && header
                    .value
                    .trim_start()
                    .to_ascii_lowercase()
                    .starts_with("multipart/"))
        })
        .map(|header| (header.name.clone(), header.value.clone()))
        .collect();
    let mut url = request.url.trim().to_owned();
    let mut credentials = None;
    match &request.auth {
        Auth::None => {}
        Auth::Basic { username, password } => {
            credentials = Some(Credentials::Basic(username.clone(), password.clone()));
        }
        Auth::Digest { username, password } => {
            credentials = Some(Credentials::Digest(username.clone(), password.clone()));
        }
        Auth::Bearer { token } => headers.push(("Authorization".into(), format!("Bearer {token}"))),
        Auth::ApiKey {
            name,
            value,
            place: KeyPlace::Header,
        } => headers.push((name.clone(), value.clone())),
        Auth::ApiKey {
            name,
            value,
            place: KeyPlace::Query,
        } => url = with_query(&url, &format!("{}={}", encode(name), encode(value))),
        Auth::OAuth2 { .. } => notes.push(Note::OAuth2),
    }
    let named = headers
        .iter()
        .any(|(name, _)| name.eq_ignore_ascii_case("Content-Type"));
    if !named && matches!(request.body, Body::Json { .. } | Body::GraphQl { .. }) {
        headers.push(("Content-Type".into(), "application/json".into()));
    }
    Shape {
        url,
        headers,
        credentials,
    }
}

/// `url` with `pair` added to its query, before a fragment.
pub(crate) fn with_query(url: &str, pair: &str) -> String {
    let (base, fragment) = url
        .split_once('#')
        .map_or((url, None), |(b, f)| (b, Some(f)));
    let joiner = if base.contains('?') { '&' } else { '?' };
    let mut out = format!("{base}{joiner}{pair}");
    if let Some(fragment) = fragment {
        out.push('#');
        out.push_str(fragment);
    }
    out
}

/// A GraphQL body as the core sends it: `{"query": ..., "variables": ...}`, the variables' text
/// as written (it may hold variables, so it is not parsed here).
pub(crate) fn graphql_payload(query: &str, variables: &str) -> String {
    let query = serde_json::Value::String(query.to_owned()).to_string();
    if variables.trim().is_empty() {
        format!("{{\"query\":{query}}}")
    } else {
        format!("{{\"query\":{query},\"variables\":{}}}", variables.trim())
    }
}

/// The file name of a path (`./img/logo.png` -> `logo.png`).
pub(crate) fn file_name_of(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

/// A timeout in seconds as a tool reads it: `5`, `2.5`.
pub(crate) fn seconds(ms: u64) -> String {
    if ms.is_multiple_of(1000) {
        (ms / 1000).to_string()
    } else {
        let text = format!("{:.3}", ms as f64 / 1000.0);
        text.trim_end_matches('0').to_owned()
    }
}

#[cfg(test)]
#[path = "shape_tests.rs"]
mod tests;
