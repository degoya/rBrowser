//! Every request of every view in the host's capture (`$console`, at most 200 per view), each
//! instance on its own (RB-096-06): what the endpoint analysis counts. A request is the current
//! page's when it started after the view's last new document (`navigatedAt`). The response
//! facts (waiting time, caching, compression) and the decoded size come from the host's
//! capture; a view of another engine has none.

use rb_plugin_api::ViewId;
use serde_json::Value;

use crate::context::FEED_KEY;

/// What the response said about speed, caching and compression.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Response {
    pub ttfb_ms: Option<f64>,
    /// `Content-Encoding` (`br`, `gzip`); `None` without one.
    pub encoding: Option<String>,
    pub cache_control: Option<String>,
    pub expires: bool,
    pub etag: bool,
    pub last_modified: bool,
}

impl Response {
    fn read(value: &Value) -> Option<Self> {
        value.is_object().then(|| Self {
            ttfb_ms: value["ttfbMs"].as_f64(),
            encoding: value["encoding"].as_str().map(str::to_owned),
            cache_control: value["cacheControl"].as_str().map(str::to_owned),
            expires: value["expires"] == Value::Bool(true),
            etag: value["etag"] == Value::Bool(true),
            last_modified: value["lastModified"] == Value::Bool(true),
        })
    }

    /// Nothing lets a cache keep or revalidate it: no `Cache-Control`, `Expires`, `ETag` or
    /// `Last-Modified`.
    #[must_use]
    pub fn uncached(&self) -> bool {
        self.cache_control.is_none() && !self.expires && !self.etag && !self.last_modified
    }
}

/// One request of one view.
#[derive(Clone, Debug, PartialEq)]
pub struct Call {
    pub view: ViewId,
    /// The host's request id, for the body and the list.
    pub id: String,
    pub method: String,
    pub url: String,
    pub resource_type: String,
    pub status: Option<u64>,
    pub failed: bool,
    pub from_cache: bool,
    pub mime_type: Option<String>,
    /// Bytes received (headers and encoded body).
    pub size: Option<f64>,
    /// The body's bytes decoded.
    pub decoded_size: Option<f64>,
    pub duration_ms: Option<f64>,
    /// Milliseconds since the epoch.
    pub time: f64,
    /// Started after the view's last new document.
    pub current: bool,
    pub response: Option<Response>,
}

impl Call {
    /// The body's size: decoded when known, else what came over the wire.
    #[must_use]
    pub fn body_size(&self) -> Option<f64> {
        self.decoded_size.or(self.size)
    }
}

/// Whether a response of `mime` is worth compressing: text, JSON, JavaScript, XML and SVG.
#[must_use]
pub fn compressible(mime: Option<&str>) -> bool {
    let Some(mime) = mime.map(str::to_ascii_lowercase) else {
        return false;
    };
    mime.starts_with("text/")
        || ["json", "javascript", "xml", "graphql", "csv"]
            .iter()
            .any(|kind| mime.contains(kind))
}

/// The calls of `args`' capture, view by view, oldest first.
#[must_use]
pub fn calls(args: &Value) -> Vec<Call> {
    let mut out = Vec::new();
    for view in args[FEED_KEY]["views"].as_array().into_iter().flatten() {
        let Some(id) = view["view"]
            .as_u64()
            .and_then(|id| ViewId::try_from(id).ok())
        else {
            continue;
        };
        let navigated = view["navigatedAt"].as_f64();
        for entry in view["network"].as_array().into_iter().flatten() {
            let (Some(url), Some(method)) = (entry["url"].as_str(), entry["method"].as_str())
            else {
                continue;
            };
            let time = entry["time"].as_f64().unwrap_or(0.0);
            out.push(Call {
                view: id,
                id: entry["id"].as_str().unwrap_or_default().to_owned(),
                method: method.to_owned(),
                url: url.split_once('#').map_or(url, |(base, _)| base).to_owned(),
                resource_type: entry["resourceType"].as_str().unwrap_or("Other").to_owned(),
                status: entry["status"].as_u64(),
                failed: entry["failed"].is_string(),
                from_cache: entry["fromCache"] == Value::Bool(true),
                mime_type: entry["mimeType"].as_str().map(str::to_owned),
                size: entry["size"].as_f64(),
                decoded_size: entry["decodedSize"].as_f64(),
                duration_ms: entry["durationMs"].as_f64(),
                time,
                current: navigated.is_none_or(|at| time > at),
                response: Response::read(&entry["response"]),
            });
        }
    }
    out
}

#[cfg(test)]
#[path = "calls_tests.rs"]
mod tests;
