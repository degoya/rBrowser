//! The page source a run validates (`html-validate.source`).
//!
//! **Raw** ([`served`], the default): the document as the server sent it, from the view's own
//! network - `Page.getFrameTree` names the main frame and its URL, `Page.getResourceContent`
//! answers the document the view loaded: the bytes it rendered from, with its cookies and
//! session, without a second request (which could answer differently, or not at all behind a
//! login or a POST). This is the markup, as the W3C validator reads it. The host lets
//! `getResourceContent` through for a web URL only (`cdp_policy`). When the view does not hold the
//! document, `unserved` says why in words a person can act on, or reads its body from the view's
//! network log (RB-099-06).
//!
//! **Rendered** ([`rendered`], RB-094-06): the DOM after rendering - what scripts built, and what
//! the browser's parser made of the markup - serialized in the view's isolated world by
//! [`render_script`]: the doctype and `document.documentElement.outerHTML`. Its lines are those
//! of the serialization, so its findings name the element by selector, not by line.
//!
//! Both are held to [`MAX_SOURCE_BYTES`].

use rb_plugin_api::{Failure, FailureKind, PluginHost, ViewId};
use serde_json::{Value, json};

use crate::key;

/// The largest source validated: 1 MiB. With the runner it stays far below the 4 MiB an
/// `evaluate` script may have.
pub const MAX_SOURCE_BYTES: usize = 1024 * 1024;

/// The served source of a view's page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Served {
    /// The main frame's URL, without its fragment.
    pub url: String,
    pub text: String,
}

pub(crate) fn no_source(view: ViewId, reason: &str) -> Failure {
    Failure::coded(FailureKind::NotFound, &key("errors", "no_source"))
        .with_param("view", view)
        .with_param("reason", reason)
}

/// Why the host failed: its own reason when it gives one (the browser's words), else its code.
pub(crate) fn reason(failure: &Failure) -> String {
    failure
        .param("reason")
        .filter(|reason| !reason.is_empty())
        .or(failure.code())
        .unwrap_or(failure.kind.as_str())
        .chars()
        .take(200)
        .collect()
}

fn too_large(view: ViewId, size: usize) -> Failure {
    Failure::coded(FailureKind::Limit, &key("errors", "too_large"))
        .with_param("view", view)
        .with_param("size", size.div_ceil(1024))
        .with_param("limit", MAX_SOURCE_BYTES / 1024)
}

fn is_web(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

/// The source in `answer`, `Page.getResourceContent`'s JSON.
///
/// # Errors
///
/// The answer holds no text (`errors.no_source`).
pub fn from_answer(view: ViewId, answer: &str) -> Result<String, Failure> {
    let value: Value =
        serde_json::from_str(answer).map_err(|error| no_source(view, &error.to_string()))?;
    if value["base64Encoded"].as_bool().unwrap_or(false) {
        return Err(no_source(view, "not a text document"));
    }
    value["content"]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| no_source(view, "no content"))
}

/// The main frame of the page in `view`.
struct MainFrame {
    id: String,
    /// The document's loader, the id of its request in the view's network log.
    loader: Option<String>,
    /// Without the fragment.
    url: String,
}

/// The main frame of the page in `view`.
fn main_frame(host: &mut dyn PluginHost, view: ViewId) -> Result<MainFrame, Failure> {
    let tree = host
        .cdp_send(view, "Page.getFrameTree", "{}")
        .map_err(|failure| {
            if failure.unserved_engine_in(view).is_some() {
                failure
            } else {
                no_source(view, &reason(&failure))
            }
        })?;
    let tree: Value =
        serde_json::from_str(&tree).map_err(|error| no_source(view, &error.to_string()))?;
    let frame = &tree["frameTree"]["frame"];
    let (Some(id), Some(url)) = (frame["id"].as_str(), frame["url"].as_str()) else {
        return Err(no_source(view, "no main frame"));
    };
    let url = url.split_once('#').map_or(url, |(page, _)| page).to_owned();
    if !is_web(&url) {
        return Err(
            Failure::coded(FailureKind::NotSupported, &key("errors", "not_web"))
                .with_param("view", view)
                .with_param("url", url.chars().take(200).collect::<String>()),
        );
    }
    Ok(MainFrame {
        id: id.to_owned(),
        loader: frame["loaderId"].as_str().map(str::to_owned),
        url,
    })
}

/// The served source of the page in `view`.
///
/// # Errors
///
/// The host's refusal of a view whose engine has no CDP, unchanged (the check runner shows such
/// a view skipped); `errors.not_web` for a page that is no web page; `errors.no_source` when the
/// view has no source, or why (`unserved::explain`); `errors.too_large` beyond
/// [`MAX_SOURCE_BYTES`].
pub fn served(host: &mut dyn PluginHost, view: ViewId) -> Result<Served, Failure> {
    let MainFrame { id, loader, url } = main_frame(host, view)?;
    let params = json!({ "frameId": id, "url": url }).to_string();
    let text = match host.cdp_send(view, "Page.getResourceContent", &params) {
        Ok(answer) => from_answer(view, &answer)?,
        Err(failure) => crate::unserved::explain(host, view, loader.as_deref(), &failure)?,
    };
    if text.len() > MAX_SOURCE_BYTES {
        return Err(too_large(view, text.len()));
    }
    Ok(Served { url, text })
}

/// The script that serializes the rendered DOM in the view's isolated world (which shares the
/// page's DOM, not its globals): `{"html": <doctype and document element>}`, or
/// `{"tooLarge": <characters>}` beyond [`MAX_SOURCE_BYTES`] characters, so a huge DOM never
/// crosses the bridge.
#[must_use]
pub fn render_script() -> String {
    format!(
        "(() => {{ const t = document.doctype; \
         const id = t ? (t.publicId ? ` PUBLIC \"${{t.publicId}}\"` : (t.systemId ? ' SYSTEM' : '')) \
         + (t.systemId ? ` \"${{t.systemId}}\"` : '') : ''; \
         const root = document.documentElement; \
         const html = (t ? `<!DOCTYPE ${{t.name}}${{id}}>\\n` : '') + (root ? root.outerHTML : ''); \
         return html.length > {MAX_SOURCE_BYTES} ? {{ tooLarge: html.length }} : {{ html }}; }})()"
    )
}

/// The rendered DOM of the page in `view`, serialized ([`render_script`]).
///
/// # Errors
///
/// As [`served`] for the main frame; `errors.no_source` when the view answers no DOM;
/// `errors.too_large` beyond [`MAX_SOURCE_BYTES`].
pub fn rendered(host: &mut dyn PluginHost, view: ViewId) -> Result<Served, Failure> {
    let MainFrame { url, .. } = main_frame(host, view)?;
    let answer = host
        .evaluate(view, &render_script())
        .map_err(|failure| no_source(view, &reason(&failure)))?;
    let value: Value =
        serde_json::from_str(&answer).map_err(|error| no_source(view, &error.to_string()))?;
    let value = match value {
        Value::String(inner) => {
            serde_json::from_str(&inner).map_err(|error| no_source(view, &error.to_string()))?
        }
        other => other,
    };
    if let Some(size) = value["tooLarge"].as_u64() {
        return Err(too_large(view, usize::try_from(size).unwrap_or(usize::MAX)));
    }
    let text = value["html"]
        .as_str()
        .ok_or_else(|| no_source(view, "no rendered document"))?;
    if text.len() > MAX_SOURCE_BYTES {
        return Err(too_large(view, text.len()));
    }
    Ok(Served {
        url,
        text: text.to_owned(),
    })
}

#[cfg(test)]
#[path = "source_tests.rs"]
mod tests;
