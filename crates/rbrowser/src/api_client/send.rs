//! Sending a resolved request from the core (RB-096-01): no CORS, the proxy of the environment
//! (`HTTPS_PROXY`, `HTTP_PROXY`, `NO_PROXY`; ureq's), the request's timeout, redirects and TLS
//! check, the phases timed ([`super::timing`]). [`prepare`] turns the model into bytes - the
//! auth into its header or query, the body into its encoding and default `Content-Type` - and
//! [`send_blocking`] runs it on a blocking thread's time, the body read up to a limit. ureq
//! follows no redirect: [`next_hop`] gives the next request as a browser makes it (303, and 301
//! or 302 after a `POST`, become a `GET` without a body; the `Authorization` and `Cookie`
//! headers stay behind when the host changes), and the caller holds its host to the grants.
//! Digest and OAuth 2 are modelled and stored, not sent yet: they answer
//! `auth_not_supported`.

use std::io::Read as _;
use std::sync::Arc;
use std::time::{Duration, Instant};

use base64::Engine as _;
pub use rb_api_model::http_file::form_encode;
use rb_api_model::http_file::{GRAPHQL_HEADER, boundary_of};
use rb_api_model::mask::mask;
use rb_api_model::{ApiRequest, Auth, Body, KeyPlace, Options};
use rb_plugin_types::{Failure, FailureKind};
use serde_json::{Value, json};

use super::error;
use super::timing::{Marks, Phases, TimedResolver, connector};

/// The boundary of a multipart body the request names none for.
pub const BOUNDARY: &str = "rbrowser-api-client-boundary";
/// The most redirects followed.
pub const MAX_REDIRECTS: u32 = 10;

/// A request ready to go.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Prepared {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    pub options: Options,
    /// The headers (lower case) that carry a credential besides `Authorization`,
    /// `Proxy-Authorization` and `Cookie`: the API key's, every one holding a secret. None of
    /// them follows a redirect to another origin.
    pub credentials: Vec<String>,
}

/// What came back.
#[derive(Clone, Debug, PartialEq)]
pub struct Exchange {
    pub status: u16,
    pub status_text: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    pub truncated: bool,
    pub phases: Phases,
}

/// The Basic pair as it goes out (base64 of `user:password`) when a secret is in it: masked as a
/// secret of its own, since no form of the secret alone is in the header. Empty for any other
/// auth.
#[must_use]
pub fn derived_secrets(request: &ApiRequest, secrets: &[String]) -> Vec<String> {
    let Auth::Basic { username, password } = &request.auth else {
        return Vec::new();
    };
    let holds = |text: &str| secrets.iter().any(|secret| text.contains(secret.as_str()));
    if !holds(username) && !holds(password) {
        return Vec::new();
    }
    vec![base64::engine::general_purpose::STANDARD.encode(format!("{username}:{password}"))]
}

fn invalid(name: &str, reason: impl ToString) -> Failure {
    error(FailureKind::BadData, name).with_param("reason", reason)
}

/// `resolved` (every variable in) as bytes; `file` reads a body's or a part's file.
///
/// # Errors
///
/// Not `http(s)`, a JSON body or GraphQL variables that do not parse, an auth not sent yet, a
/// file that does not read.
pub fn prepare(
    resolved: &ApiRequest,
    file: &dyn Fn(&str) -> Result<Vec<u8>, Failure>,
) -> Result<Prepared, Failure> {
    let mut url = resolved.url.trim().to_owned();
    let lowered = url.to_ascii_lowercase();
    if !(lowered.starts_with("http://") || lowered.starts_with("https://")) {
        return Err(invalid("invalid_request", "only http and https"));
    }
    let mut headers: Vec<(String, String)> = resolved
        .headers
        .iter()
        .filter(|header| header.enabled && !header.name.eq_ignore_ascii_case(GRAPHQL_HEADER))
        .map(|header| (header.name.clone(), header.value.clone()))
        .collect();
    let mut credentials = Vec::new();
    match &resolved.auth {
        Auth::None => {}
        Auth::Basic { username, password } => {
            let pair =
                base64::engine::general_purpose::STANDARD.encode(format!("{username}:{password}"));
            headers.push(("Authorization".to_owned(), format!("Basic {pair}")));
        }
        Auth::Bearer { token } => {
            headers.push(("Authorization".to_owned(), format!("Bearer {token}")))
        }
        Auth::ApiKey {
            name,
            value,
            place: KeyPlace::Header,
        } => {
            credentials.push(name.to_ascii_lowercase());
            headers.push((name.clone(), value.clone()));
        }
        Auth::ApiKey {
            name,
            value,
            place: KeyPlace::Query,
        } => {
            let (base, fragment) = url
                .split_once('#')
                .map_or((url.as_str(), None), |(b, f)| (b, Some(f)));
            let joiner = if base.contains('?') { '&' } else { '?' };
            let mut next = format!("{base}{joiner}{}={}", form_encode(name), form_encode(value));
            if let Some(fragment) = fragment {
                next.push('#');
                next.push_str(fragment);
            }
            url = next;
        }
        Auth::Digest { .. } | Auth::OAuth2 { .. } => {
            return Err(error(FailureKind::NotSupported, "auth_not_supported")
                .with_param("auth", resolved.auth.kind()));
        }
    }
    let has_type = headers
        .iter()
        .any(|(name, _)| name.eq_ignore_ascii_case("Content-Type"));
    let (body, content_type) = body_of(resolved, &mut headers, file)?;
    if !has_type && let Some(content_type) = content_type {
        headers.push(("Content-Type".to_owned(), content_type));
    }
    Ok(Prepared {
        method: resolved.method.clone(),
        url,
        headers,
        body,
        options: resolved.options,
        credentials,
    })
}

/// Marks every header of `prepared` whose value holds one of `secrets` as a credential.
pub fn mark_secret_headers(prepared: &mut Prepared, secrets: &[String]) {
    for (name, value) in &prepared.headers {
        let lower = name.to_ascii_lowercase();
        let holds = secrets
            .iter()
            .any(|secret| !secret.is_empty() && value.contains(secret.as_str()));
        if holds && !prepared.credentials.contains(&lower) {
            prepared.credentials.push(lower);
        }
    }
}

/// The body's bytes and the `Content-Type` it wants when the request names none.
fn body_of(
    resolved: &ApiRequest,
    headers: &mut Vec<(String, String)>,
    file: &dyn Fn(&str) -> Result<Vec<u8>, Failure>,
) -> Result<(Vec<u8>, Option<String>), Failure> {
    let default = resolved.body.default_content_type().map(str::to_owned);
    Ok(match &resolved.body {
        Body::None => (Vec::new(), None),
        Body::Json { text } => {
            serde_json::from_str::<Value>(text)
                .map_err(|e| invalid("invalid_json", e.to_string()))?;
            (text.clone().into_bytes(), default)
        }
        Body::Raw { text } => (text.clone().into_bytes(), None),
        Body::Binary { path } => (file(path)?, None),
        Body::Form { fields } => {
            let line: Vec<String> = fields
                .iter()
                .filter(|field| field.enabled)
                .map(|field| format!("{}={}", form_encode(&field.name), form_encode(&field.value)))
                .collect();
            (line.join("&").into_bytes(), default)
        }
        Body::GraphQl { query, variables } => {
            let mut payload = json!({ "query": query });
            if !variables.trim().is_empty() {
                let parsed: Value = serde_json::from_str(variables)
                    .map_err(|e| invalid("invalid_json", e.to_string()))?;
                payload["variables"] = parsed;
            }
            (payload.to_string().into_bytes(), default)
        }
        Body::Multipart { parts } => {
            let named = headers
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case("Content-Type"))
                .and_then(|(_, value)| boundary_of(value));
            let boundary = named.clone().unwrap_or_else(|| BOUNDARY.to_owned());
            if named.is_none() {
                // A multipart type without its boundary is replaced by one that names it.
                headers.retain(|(name, _)| !name.eq_ignore_ascii_case("Content-Type"));
                headers.push((
                    "Content-Type".to_owned(),
                    format!("multipart/form-data; boundary={boundary}"),
                ));
            }
            let mut bytes = Vec::new();
            for part in parts {
                bytes.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
                let mut disposition =
                    format!("Content-Disposition: form-data; name=\"{}\"", part.name);
                if let Some(path) = &part.file {
                    let name = path
                        .rsplit(['/', '\\'])
                        .next()
                        .unwrap_or(path)
                        .replace('"', "");
                    disposition.push_str(&format!("; filename=\"{name}\""));
                }
                bytes.extend_from_slice(disposition.as_bytes());
                bytes.extend_from_slice(b"\r\n");
                if let Some(content_type) = &part.content_type {
                    bytes.extend_from_slice(format!("Content-Type: {content_type}\r\n").as_bytes());
                }
                bytes.extend_from_slice(b"\r\n");
                match &part.file {
                    Some(path) => bytes.extend_from_slice(&file(path)?),
                    None => bytes.extend_from_slice(part.value.as_bytes()),
                }
                bytes.extend_from_slice(b"\r\n");
            }
            bytes.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
            (bytes, None)
        }
    })
}

/// Where a send goes out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    /// Through the proxy the environment names (`HTTPS_PROXY`, `HTTP_PROXY`, `NO_PROXY`).
    Environment,
    /// Straight to the host (the tests' local server).
    Direct,
}

fn agent(options: &Options, marks: &Arc<Marks>, route: Route) -> ureq::Agent {
    let proxy = match route {
        Route::Environment => ureq::Proxy::try_from_env(),
        Route::Direct => None,
    };
    let tls = ureq::tls::TlsConfig::builder()
        .disable_verification(!options.verify_tls)
        .build();
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_millis(options.timeout_ms)))
        // Redirects are followed by [`next_hop`], each new host held to the grants.
        .max_redirects(0)
        .http_status_as_error(false)
        .proxy(proxy)
        .tls_config(tls)
        .user_agent(concat!(
            "rBrowser/",
            env!("CARGO_PKG_VERSION"),
            " (API client)"
        ))
        .build();
    ureq::Agent::with_parts(
        config,
        connector(marks),
        TimedResolver::new(Arc::clone(marks)),
    )
}

/// The request a redirect answer leads to, when `prepared` follows redirects and `exchange`
/// is one with a `Location` that reads as an `http(s)` URL.
#[must_use]
pub fn next_hop(prepared: &Prepared, exchange: &Exchange) -> Option<Prepared> {
    if !prepared.options.follow_redirects || !matches!(exchange.status, 301 | 302 | 303 | 307 | 308)
    {
        return None;
    }
    let location = exchange
        .headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("location"))
        .map(|(_, value)| value.trim())?;
    let next = url::Url::parse(&prepared.url).ok()?.join(location).ok()?;
    if !matches!(next.scheme(), "http" | "https") {
        return None;
    }
    let mut hop = prepared.clone();
    // The origin: scheme, host and port - https down to http on the same host is another one.
    let same_origin = url::Url::parse(&prepared.url)
        .ok()
        .is_some_and(|before| before.origin() == next.origin());
    hop.url = next.to_string();
    let to_get = exchange.status == 303
        || (matches!(exchange.status, 301 | 302) && prepared.method.eq_ignore_ascii_case("POST"));
    if to_get {
        hop.method = "GET".to_owned();
        hop.body.clear();
        hop.headers.retain(|(name, _)| {
            !name.eq_ignore_ascii_case("content-type")
                && !name.eq_ignore_ascii_case("content-length")
        });
    }
    if !same_origin {
        hop.headers.retain(|(name, _)| {
            let lower = name.to_ascii_lowercase();
            !matches!(
                lower.as_str(),
                "authorization" | "proxy-authorization" | "cookie"
            ) && !prepared.credentials.contains(&lower)
        });
    }
    Some(hop)
}

/// Sends `prepared` by `route`, the body read up to `limit` bytes; `secrets` are masked in a
/// failure's reason. Blocks.
///
/// # Errors
///
/// `send_failed` with ureq's reason (masked): no such host, refused, timed out, TLS.
pub fn send_blocking(
    prepared: &Prepared,
    limit: u64,
    secrets: &[String],
    route: Route,
) -> Result<Exchange, Failure> {
    let failed = |reason: String| {
        error(FailureKind::Io, "send_failed")
            .with_param("reason", rb_util::text::cut(&mask(&reason, secrets), 300))
    };
    let mut builder = ureq::http::Request::builder()
        .method(prepared.method.as_str())
        .uri(prepared.url.as_str());
    for (name, value) in &prepared.headers {
        builder = builder.header(name.as_str(), value.as_str());
    }
    let request = builder
        .body(prepared.body.clone())
        .map_err(|e| invalid("invalid_request", mask(&e.to_string(), secrets)))?;
    let marks = Arc::new(Marks::default());
    let agent = agent(&prepared.options, &marks, route);
    marks.start();
    let mut response = agent.run(request).map_err(|e| failed(e.to_string()))?;
    let headers_at = Instant::now();
    let status = response.status();
    let headers = response
        .headers()
        .iter()
        .map(|(name, value)| {
            (
                name.as_str().to_owned(),
                String::from_utf8_lossy(value.as_bytes()).into_owned(),
            )
        })
        .collect();
    let mut body = Vec::new();
    response
        .body_mut()
        .as_reader()
        .take(limit.saturating_add(1))
        .read_to_end(&mut body)
        .map_err(|e| failed(e.to_string()))?;
    let truncated = u64::try_from(body.len()).unwrap_or(u64::MAX) > limit;
    body.truncate(usize::try_from(limit).unwrap_or(usize::MAX));
    let phases = marks.times().phases(headers_at, Instant::now());
    Ok(Exchange {
        status: status.as_u16(),
        status_text: status.canonical_reason().unwrap_or_default().to_owned(),
        headers,
        body,
        truncated,
        phases,
    })
}

/// The answer the panel and MCP get: status, phases, size, headers, the body as text or base64,
/// every secret masked.
#[must_use]
pub fn answer(prepared: &Prepared, exchange: &Exchange, secrets: &[String]) -> Value {
    let (body, encoding) = match std::str::from_utf8(&exchange.body) {
        Ok(text) => (text.to_owned(), "text"),
        Err(_) => (
            base64::engine::general_purpose::STANDARD.encode(&exchange.body),
            "base64",
        ),
    };
    let content_type = exchange
        .headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("content-type"))
        .map(|(_, value)| value.clone());
    let phases = exchange.phases;
    let value = json!({
        "method": prepared.method,
        "url": prepared.url,
        "status": exchange.status,
        "statusText": exchange.status_text,
        "headers": exchange.headers.iter().map(|(n, v)| json!([n, v])).collect::<Vec<_>>(),
        "contentType": content_type,
        "body": body,
        "bodyEncoding": encoding,
        "size": exchange.body.len(),
        "truncated": exchange.truncated,
        "timings": {
            "dns": phases.dns,
            "connect": phases.connect,
            "tls": phases.tls,
            "firstByte": phases.first_byte,
            "total": phases.total,
        },
    });
    rb_api_model::mask::mask_json(&value, secrets)
}

#[cfg(test)]
#[path = "send_tests.rs"]
mod tests;
