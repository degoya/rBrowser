//! The HTTP transport of the plugins' `http` capability (the first plugin that fetches is
//! sitemap capture): the host has checked the URL (`http`/`https`, no credentials) and the
//! rate already. Here only `GET` and `HEAD` go out, with the plugin's `Accept` and
//! `Accept-Language` and no other header (no cookie, no authorization), a redirect is answered
//! and never followed, at most `max_response_bytes` of the body are read, and the answer
//! carries only the headers a plugin needs. The request runs on a blocking thread. No cookie
//! store: ureq's `cookies` feature is not enabled in the workspace, and a test holds that a
//! `Set-Cookie` never goes back with the next request (review finding 10 of Charge 4).

use std::time::Duration;

use async_trait::async_trait;
use rb_plugin_host::HostServices;
use rb_plugin_types::{Failure, FailureKind, HttpRequest, HttpResponse, codes};

/// How long one request may take.
pub const TIMEOUT: Duration = Duration::from_secs(30);
/// The request headers a plugin may set.
pub const REQUEST_HEADERS: [&str; 2] = ["accept", "accept-language"];
/// The answer headers a plugin sees.
pub const RESPONSE_HEADERS: [&str; 5] = [
    "location",
    "content-type",
    "content-encoding",
    "content-length",
    "last-modified",
];

/// The services the core gives every plugin: the HTTP transport.
pub struct CoreServices {
    agent: ureq::Agent,
}

impl Default for CoreServices {
    fn default() -> Self {
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(TIMEOUT))
            .max_redirects(0)
            .http_status_as_error(false)
            .user_agent(concat!(
                "rBrowser/",
                env!("CARGO_PKG_VERSION"),
                " (sitemap)"
            ))
            .build()
            .into();
        Self { agent }
    }
}

fn failed(reason: &str) -> Failure {
    Failure::coded(FailureKind::Io, codes::HTTP_FAILED).with_param("reason", reason)
}

/// The headers of `request` that go out.
pub fn outgoing(request: &HttpRequest) -> Vec<(String, String)> {
    request
        .headers
        .iter()
        .filter(|(name, value)| {
            REQUEST_HEADERS.contains(&name.to_ascii_lowercase().as_str())
                && value.len() <= 1024
                && !value.chars().any(char::is_control)
        })
        .cloned()
        .collect()
}

fn fetch_blocking(
    agent: &ureq::Agent,
    request: &HttpRequest,
    limit: u64,
) -> Result<HttpResponse, Failure> {
    let call = match request.method.to_ascii_uppercase().as_str() {
        "GET" => agent.get(&request.url),
        "HEAD" => agent.head(&request.url),
        _ => {
            return Err(
                Failure::coded(FailureKind::NotSupported, codes::HTTP_FAILED)
                    .with_param("reason", "only GET and HEAD"),
            );
        }
    };
    let call = outgoing(request)
        .into_iter()
        .fold(call, |call, (name, value)| call.header(name, value));
    let mut response = call.call().map_err(|error| failed(&error.to_string()))?;
    let status = response.status().as_u16();
    let headers = response
        .headers()
        .iter()
        .filter(|(name, _)| RESPONSE_HEADERS.contains(&name.as_str()))
        .filter_map(|(name, value)| {
            Some((name.as_str().to_owned(), value.to_str().ok()?.to_owned()))
        })
        .collect();
    let body = if request.method.eq_ignore_ascii_case("HEAD") {
        Vec::new()
    } else {
        response
            .body_mut()
            .with_config()
            .limit(limit)
            .read_to_vec()
            .map_err(|error| {
                Failure::coded(FailureKind::Limit, codes::TOO_LARGE)
                    .with_param("reason", error.to_string())
            })?
    };
    Ok(HttpResponse {
        status,
        headers,
        body,
    })
}

#[async_trait]
impl HostServices for CoreServices {
    async fn fetch(
        &self,
        request: HttpRequest,
        max_response_bytes: u64,
    ) -> Result<HttpResponse, Failure> {
        let agent = self.agent.clone();
        tokio::task::spawn_blocking(move || fetch_blocking(&agent, &request, max_response_bytes))
            .await
            .map_err(|error| failed(&error.to_string()))?
    }
}

#[cfg(test)]
#[path = "http_tests.rs"]
mod tests;
