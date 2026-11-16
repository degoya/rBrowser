//! Compose and Repeat (RB-080-37): a request sent from the core with `ureq`, from the context of
//! the view that saw it - its cookies for the URL, its user agent and extra headers - without
//! touching the page. The answer is one result with status, headers, body and time; "Repeat
//! 10x" sends them one after another. The pure parts are here: the request with its context,
//! the edits, the one send and the result's JSON; the host's context ([`Runner`]) is
//! [`super::runner`], the CDP side of it.

use std::future::Future;
use std::pin::Pin;
use std::time::{Duration, Instant};

use base64::Engine as _;
use rb_engine::ViewId;
use serde_json::{Value, json};

use super::held::Edits;
use super::mask;

/// The most bytes of a response body kept.
pub const MAX_RESULT_BODY: usize = 256 * 1024;
/// The most results `$network.composed` keeps.
pub const MAX_COMPOSED: usize = 50;
/// How long one send may take in all.
const SEND_TIMEOUT: Duration = Duration::from_secs(30);
/// A request to send.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComposeRequest {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<Vec<u8>>,
}

/// What a view lends a request: its cookies for the URL, its user agent, its extra headers.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ViewContext {
    pub cookies: Vec<(String, String)>,
    pub user_agent: Option<String>,
    pub headers: Vec<(String, String)>,
}

/// What one send answered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComposeResult {
    pub method: String,
    pub url: String,
    /// The view whose context was used.
    pub view: Option<ViewId>,
    /// The status; none when the request did not get an answer.
    pub status: Option<u16>,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    pub body_truncated: bool,
    /// How long the send took.
    pub time_ms: u64,
    /// Milliseconds since the Unix epoch when it was sent.
    pub at: u64,
    pub error: Option<String>,
}

impl ComposeResult {
    /// The result as the panel and MCP see it; `masked` hides cookies and tokens.
    #[must_use]
    pub fn to_json(&self, masked: bool) -> Value {
        let headers = if masked {
            mask::mask_headers(&self.headers)
        } else {
            self.headers.clone()
        };
        let url = if masked {
            mask::mask_url(&self.url)
        } else {
            self.url.clone()
        };
        let mut entry = json!({
            "method": self.method,
            "url": url,
            "view": self.view.map(|view| view.0),
            "status": self.status,
            "headers": headers.iter().map(|(name, value)| json!([name, value])).collect::<Vec<_>>(),
            "bodyTruncated": self.body_truncated,
            "timeMs": self.time_ms,
            "at": self.at,
        });
        match std::str::from_utf8(&self.body) {
            Ok(text) => {
                entry["body"] = json!(text);
                entry["bodyBase64"] = json!(false);
            }
            Err(_) => {
                entry["body"] = json!(base64::engine::general_purpose::STANDARD.encode(&self.body));
                entry["bodyBase64"] = json!(true);
            }
        }
        if let Some(error) = &self.error {
            entry["error"] = json!(error);
        }
        entry
    }
}

/// The `Cookie` header of `cookies`.
#[must_use]
pub fn cookie_header(cookies: &[(String, String)]) -> String {
    cookies
        .iter()
        .map(|(name, value)| format!("{name}={value}"))
        .collect::<Vec<_>>()
        .join("; ")
}

fn has_header(headers: &[(String, String)], name: &str) -> bool {
    headers
        .iter()
        .any(|(known, _)| known.eq_ignore_ascii_case(name))
}

/// `request` with `context`'s cookies, user agent and extra headers added where the request
/// does not set them itself.
#[must_use]
pub fn with_context(request: &ComposeRequest, context: &ViewContext) -> ComposeRequest {
    let mut request = request.clone();
    if !context.cookies.is_empty() && !has_header(&request.headers, "cookie") {
        request
            .headers
            .push(("Cookie".to_owned(), cookie_header(&context.cookies)));
    }
    if let Some(agent) = &context.user_agent
        && !has_header(&request.headers, "user-agent")
    {
        request
            .headers
            .push(("User-Agent".to_owned(), agent.clone()));
    }
    for (name, value) in &context.headers {
        if !has_header(&request.headers, name) {
            request.headers.push((name.clone(), value.clone()));
        }
    }
    request
}

/// Header names a repeat does not send again: the client sets them for the new connection and
/// body.
const NOT_RESENT: &[&str] = &[
    "connection",
    "content-length",
    "host",
    "keep-alive",
    "transfer-encoding",
];

/// `headers` of a captured request as a repeat sends them: without the hop and length headers.
#[must_use]
pub fn resendable(headers: &[(String, String)]) -> Vec<(String, String)> {
    headers
        .iter()
        .filter(|(name, _)| {
            !NOT_RESENT
                .iter()
                .any(|gone| name.eq_ignore_ascii_case(gone))
        })
        .cloned()
        .collect()
}

/// `request` with `edits` applied (a repeat's changes). When the edits move the request to
/// another origin and bring no headers of their own, the captured secrets (cookie,
/// authorization, tokens) and `origin`/`referer` stay behind: they belong to the first host.
#[must_use]
pub fn with_edits(request: &ComposeRequest, edits: &Edits) -> ComposeRequest {
    let url = edits.url.clone().unwrap_or_else(|| request.url.clone());
    let moved = rb_util::url::origin_of(&url) != rb_util::url::origin_of(&request.url);
    let headers = match &edits.headers {
        Some(headers) => headers.clone(),
        None if moved => request
            .headers
            .iter()
            .filter(|(name, _)| {
                !mask::sensitive_header(name)
                    && !name.eq_ignore_ascii_case("origin")
                    && !name.eq_ignore_ascii_case("referer")
            })
            .cloned()
            .collect(),
        None => request.headers.clone(),
    };
    ComposeRequest {
        method: edits
            .method
            .clone()
            .unwrap_or_else(|| request.method.clone()),
        url,
        headers,
        body: edits.body.clone().or_else(|| request.body.clone()),
    }
}

/// The client every send uses: no `https` restriction (a local API is `http`), errors as
/// statuses, and no redirect followed - the request carries the view's cookies in an explicit
/// `Cookie` header, which must not travel to another host; a 3xx is answered as it came, with
/// its `location`.
#[must_use]
pub fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .max_redirects(0)
        .timeout_global(Some(SEND_TIMEOUT))
        .http_status_as_error(false)
        .user_agent(format!("rBrowser/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .into()
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| {
            u64::try_from(since.as_millis()).unwrap_or(u64::MAX)
        })
}

/// Sends `request` once and reads at most `max_body` bytes of the answer. Blocking: call it off
/// the async workers.
#[must_use]
pub fn send(
    agent: &ureq::Agent,
    request: &ComposeRequest,
    max_body: usize,
    view: Option<ViewId>,
) -> ComposeResult {
    let at = now_ms();
    let started = Instant::now();
    let mut result = ComposeResult {
        method: request.method.clone(),
        url: request.url.clone(),
        view,
        status: None,
        headers: Vec::new(),
        body: Vec::new(),
        body_truncated: false,
        time_ms: 0,
        at,
        error: None,
    };
    let outcome =
        build(request).and_then(|built| agent.run(built).map_err(|error| error.to_string()));
    match outcome {
        Ok(response) => {
            result.status = Some(response.status().as_u16());
            result.headers = response
                .headers()
                .iter()
                .map(|(name, value)| {
                    (
                        name.as_str().to_owned(),
                        String::from_utf8_lossy(value.as_bytes()).into_owned(),
                    )
                })
                .collect();
            use std::io::Read as _;
            let reader = response.into_body().into_reader();
            let mut body = Vec::new();
            let limit = u64::try_from(max_body)
                .unwrap_or(u64::MAX)
                .saturating_add(1);
            if let Err(error) = reader.take(limit).read_to_end(&mut body) {
                result.error = Some(error.to_string());
            }
            if body.len() > max_body {
                body.truncate(max_body);
                result.body_truncated = true;
            }
            result.body = body;
        }
        Err(error) => result.error = Some(error),
    }
    result.time_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    result
}

fn build(request: &ComposeRequest) -> Result<ureq::http::Request<Vec<u8>>, String> {
    let mut builder = ureq::http::Request::builder()
        .method(request.method.as_str())
        .uri(request.url.as_str());
    for (name, value) in &request.headers {
        builder = builder.header(name.as_str(), value.as_str());
    }
    builder
        .body(request.body.clone().unwrap_or_default())
        .map_err(|error| error.to_string())
}

/// A future the runner answers with.
pub type Answer<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

/// The host's side of compose and repeat: a view's context over CDP and the sends off the
/// async workers. Tests run without one.
pub trait Runner: Send + Sync {
    /// Sends `request` `times` in a row from `view`'s context (none: no cookies, the engine's
    /// user agent).
    fn compose(
        &self,
        view: Option<ViewId>,
        request: ComposeRequest,
        times: u32,
    ) -> Answer<Vec<ComposeResult>>;

    /// Sends captured request `request_id` of `view` again with `edits`, `times` in a row.
    fn repeat(
        &self,
        view: ViewId,
        request_id: String,
        edits: Edits,
        times: u32,
    ) -> Answer<Result<Vec<ComposeResult>, String>>;

    /// Runs `task` detached on the host's runtime.
    fn spawn(&self, task: Answer<()>);
}

#[cfg(test)]
#[path = "compose_tests.rs"]
mod tests;
