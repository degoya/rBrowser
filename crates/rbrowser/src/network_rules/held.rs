//! The requests a breakpoint holds (RB-080-37): each with what the person may edit, numbered
//! `h1`, `h2`, ... and keyed by stage, method and URL so "continue for all views" finds the same
//! request in every view. The task that paused the request waits on a channel of its own; a
//! resolution reaches it there, and the task's timeout (`breakpointTimeoutMs`) lets the request go
//! on unchanged so the grid never hangs. The CDP answers of a continue and an abort are made here.

use std::collections::BTreeMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use base64::Engine as _;
use rb_engine::ViewId;
use serde_json::{Value, json};
use tokio::sync::oneshot;

use super::document::Stage;
use crate::cdp::lock;

/// What a person changed before the request goes on.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Edits {
    pub method: Option<String>,
    pub url: Option<String>,
    /// The whole header list when given.
    pub headers: Option<Vec<(String, String)>>,
    pub body: Option<Vec<u8>>,
    /// The response's status (response stage only).
    pub status: Option<u16>,
}

/// The response's part of a request held at the response stage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeldResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
}

/// One held request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeldRequest {
    /// `h<n>`, given by [`Held::hold`].
    pub id: String,
    /// `<stage> <method> <url>`, given by [`Held::hold`].
    pub key: String,
    pub view: ViewId,
    /// The view's CDP session.
    pub session: String,
    /// CDP's interception id.
    pub request_id: String,
    pub stage: Stage,
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    /// The request's post data, or the response's body at the response stage.
    pub body: Option<Vec<u8>>,
    /// The body was larger than the limit and is not held.
    pub body_truncated: bool,
    pub resource_type: String,
    /// The breakpoint that held it.
    pub breakpoint: String,
    /// Milliseconds since the Unix epoch.
    pub since: u64,
    pub response: Option<HeldResponse>,
}

impl HeldRequest {
    /// The entry of `$network.held`: the body as text, or base64 with `bodyBase64`, cut to
    /// `max_body` bytes.
    #[must_use]
    pub fn to_json(&self, max_body: usize) -> Value {
        let mut entry = json!({
            "id": self.id,
            "key": self.key,
            "view": self.view.0,
            "stage": self.stage.name(),
            "method": self.method,
            "url": self.url,
            "headers": self.headers.iter().map(|(name, value)| json!([name, value])).collect::<Vec<_>>(),
            "resourceType": self.resource_type,
            "breakpoint": self.breakpoint,
            "since": self.since,
            "bodyTruncated": self.body_truncated,
        });
        if let Some(body) = &self.body {
            let shown = &body[..body.len().min(max_body)];
            if shown.len() < body.len() {
                entry["bodyTruncated"] = json!(true);
            }
            match std::str::from_utf8(shown) {
                Ok(text) => {
                    entry["body"] = json!(text);
                    entry["bodyBase64"] = json!(false);
                }
                Err(_) => {
                    entry["body"] = json!(base64::engine::general_purpose::STANDARD.encode(shown));
                    entry["bodyBase64"] = json!(true);
                }
            }
        }
        if let Some(response) = &self.response {
            entry["response"] = json!({
                "status": response.status,
                "headers": response.headers.iter().map(|(name, value)| json!([name, value])).collect::<Vec<_>>(),
            });
        }
        entry
    }
}

/// What a person decided.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Resolution {
    Continue(Edits),
    Abort,
}

/// Why a resolution was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeldError {
    /// No request of that id is held.
    Unknown,
}

impl std::fmt::Display for HeldError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unknown => f.write_str("no request of that id is held"),
        }
    }
}

struct Entry {
    request: HeldRequest,
    decide: oneshot::Sender<Resolution>,
}

/// The held requests of one window.
#[derive(Default)]
pub struct Held {
    inner: Mutex<BTreeMap<String, Entry>>,
    next: AtomicU64,
}

impl Held {
    /// Holds `request` (its `id` and `key` set here); the receiver gets the person's resolution.
    pub fn hold(&self, mut request: HeldRequest) -> (String, oneshot::Receiver<Resolution>) {
        let id = format!("h{}", self.next.fetch_add(1, Ordering::Relaxed) + 1);
        request.id.clone_from(&id);
        request.key = format!(
            "{} {} {}",
            request.stage.name(),
            request.method,
            request.url
        );
        let (decide, receiver) = oneshot::channel();
        lock(&self.inner).insert(id.clone(), Entry { request, decide });
        (id, receiver)
    }

    /// Every held request, oldest first.
    #[must_use]
    pub fn list(&self) -> Vec<HeldRequest> {
        let inner = lock(&self.inner);
        let mut list: Vec<&Entry> = inner.values().collect();
        list.sort_by_key(|entry| entry.request.since);
        list.into_iter()
            .map(|entry| entry.request.clone())
            .collect()
    }

    /// How many are held.
    #[must_use]
    pub fn len(&self) -> usize {
        lock(&self.inner).len()
    }

    /// Whether none is held.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Resolves request `id` - with `all_views` every held request of its key - and answers how
    /// many.
    ///
    /// # Errors
    ///
    /// [`HeldError::Unknown`] when no request of that id is held.
    pub fn resolve(
        &self,
        id: &str,
        all_views: bool,
        resolution: Resolution,
    ) -> Result<usize, HeldError> {
        let mut inner = lock(&self.inner);
        let key = inner
            .get(id)
            .map(|entry| entry.request.key.clone())
            .ok_or(HeldError::Unknown)?;
        let ids: Vec<String> = inner
            .iter()
            .filter(|(known, entry)| *known == id || (all_views && entry.request.key == key))
            .map(|(id, _)| id.clone())
            .collect();
        let mut resolved = 0;
        for id in ids {
            if let Some(entry) = inner.remove(&id) {
                // A task that stopped waiting is gone with its receiver; nothing to tell.
                let _ = entry.decide.send(resolution.clone());
                resolved += 1;
            }
        }
        Ok(resolved)
    }

    /// Forgets request `id` (its task finished).
    pub fn remove(&self, id: &str) {
        lock(&self.inner).remove(id);
    }

    /// Forgets every request: their tasks get no resolution and let them go on.
    pub fn clear(&self) {
        lock(&self.inner).clear();
    }
}

fn header_entries(headers: &[(String, String)]) -> Value {
    Value::Array(
        headers
            .iter()
            .map(|(name, value)| json!({ "name": name, "value": value }))
            .collect(),
    )
}

/// The CDP call that lets `request` go on with `edits`: at the request stage
/// `Fetch.continueRequest` with the changed parts; at the response stage `Fetch.fulfillRequest`
/// when the body changed, else `Fetch.continueResponse` with a changed status or headers.
#[must_use]
pub fn continue_params(request: &HeldRequest, edits: &Edits) -> (&'static str, Value) {
    let id = json!(request.request_id);
    let engine = base64::engine::general_purpose::STANDARD;
    match request.stage {
        Stage::Request => {
            let mut params = json!({ "requestId": id });
            if let Some(method) = &edits.method {
                params["method"] = json!(method);
            }
            if let Some(url) = &edits.url {
                params["url"] = json!(url);
            }
            if let Some(headers) = &edits.headers {
                params["headers"] = header_entries(headers);
            }
            if let Some(body) = &edits.body {
                params["postData"] = json!(engine.encode(body));
            }
            ("Fetch.continueRequest", params)
        }
        Stage::Response => {
            let response = request.response.as_ref();
            let status = edits
                .status
                .or_else(|| response.map(|response| response.status))
                .unwrap_or(200);
            let headers = edits.headers.clone().unwrap_or_else(|| {
                response
                    .map(|response| response.headers.clone())
                    .unwrap_or_default()
            });
            if let Some(body) = &edits.body {
                // The held body is the decoded one: the original's encoding and length no
                // longer describe it.
                return (
                    "Fetch.fulfillRequest",
                    json!({ "requestId": id, "responseCode": status,
                            "responseHeaders": header_entries(&without_encoding(&headers)),
                            "body": engine.encode(body) }),
                );
            }
            let mut params = json!({ "requestId": id });
            if edits.status.is_some() {
                params["responseCode"] = json!(status);
            }
            if edits.headers.is_some() {
                params["responseHeaders"] = header_entries(&headers);
            }
            ("Fetch.continueResponse", params)
        }
    }
}

/// `headers` without `content-encoding`, `content-length` and `transfer-encoding`: a body
/// fulfilled whole and decoded carries none of them.
#[must_use]
pub fn without_encoding(headers: &[(String, String)]) -> Vec<(String, String)> {
    headers
        .iter()
        .filter(|(name, _)| {
            !["content-encoding", "content-length", "transfer-encoding"]
                .iter()
                .any(|gone| name.eq_ignore_ascii_case(gone))
        })
        .cloned()
        .collect()
}

/// `edits` with every masked value put back: a header value of [`super::mask::MASK`] means
/// "as it was" (the request's headers at the request stage, the response's at the response
/// stage), and so does a masked query parameter of the URL. The panel and MCP see the held
/// request masked and may send it back as they saw it. Edits that move the request to another
/// origin unmask nothing.
#[must_use]
pub fn unmask_edits(request: &HeldRequest, edits: &Edits) -> Edits {
    let mask = super::mask::MASK;
    // A request sent on to another origin takes no secret of the first one with it: the
    // masked values stay masks.
    if edits
        .url
        .as_ref()
        .is_some_and(|url| rb_util::url::origin_of(url) != rb_util::url::origin_of(&request.url))
    {
        return edits.clone();
    }
    let original = match (request.stage, &request.response) {
        (Stage::Response, Some(response)) => &response.headers,
        _ => &request.headers,
    };
    // By position: the n-th header of a name sent back masked is the n-th original of that
    // name (two `set-cookie` headers stay two different cookies).
    let headers = edits.headers.as_ref().map(|headers| {
        headers
            .iter()
            .enumerate()
            .map(|(at, (name, value))| {
                if value != mask {
                    return (name.clone(), value.clone());
                }
                let nth = headers[..at]
                    .iter()
                    .filter(|(known, _)| known.eq_ignore_ascii_case(name))
                    .count();
                original
                    .iter()
                    .filter(|(known, _)| known.eq_ignore_ascii_case(name))
                    .nth(nth)
                    .map_or_else(
                        || (name.clone(), value.clone()),
                        |(_, kept)| (name.clone(), kept.clone()),
                    )
            })
            .collect()
    });
    let url = edits.url.as_ref().map(|url| {
        if !url.contains(mask) {
            return url.clone();
        }
        let originals: Vec<(String, String)> = request
            .url
            .split_once('?')
            .map(|(_, query)| query.split('#').next().unwrap_or_default())
            .unwrap_or_default()
            .split('&')
            .filter_map(|pair| pair.split_once('='))
            .map(|(name, value)| (name.to_owned(), value.to_owned()))
            .collect();
        let (base, rest) = match url.split_once('?') {
            Some(split) => split,
            None => return url.clone(),
        };
        let (query, fragment) = rest
            .split_once('#')
            .map_or((rest, None), |(query, fragment)| (query, Some(fragment)));
        let pairs: Vec<String> = query
            .split('&')
            .map(|pair| match pair.split_once('=') {
                Some((name, value)) if value == mask => originals
                    .iter()
                    .find(|(known, _)| known == name)
                    .map_or_else(|| pair.to_owned(), |(_, kept)| format!("{name}={kept}")),
                _ => pair.to_owned(),
            })
            .collect();
        let mut out = format!("{base}?{}", pairs.join("&"));
        if let Some(fragment) = fragment {
            out.push('#');
            out.push_str(fragment);
        }
        out
    });
    Edits {
        method: edits.method.clone(),
        url,
        headers,
        body: edits.body.clone(),
        status: edits.status,
    }
}

/// The CDP call that fails `request` as aborted.
#[must_use]
pub fn abort_params(request: &HeldRequest) -> (&'static str, Value) {
    (
        "Fetch.failRequest",
        json!({ "requestId": request.request_id, "errorReason": "Aborted" }),
    )
}

#[cfg(test)]
#[path = "held_tests.rs"]
mod tests;
