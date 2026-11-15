//! The host's [`Runner`] (RB-080-37): a view's context over CDP - its cookies for the URL
//! (`Network.getCookies` on the view's session), its user agent (the emulation's override, else
//! `Browser.getVersion`) and its extra headers - and the sends off the async workers. A repeat
//! finds the captured request (method, URL, headers) in the views' buffers and its post data
//! with `Network.getRequestPostData`.

use std::sync::{Arc, Weak};

use rb_engine::{ViewHost as _, ViewId};
use serde_json::{Value, json};
use tokio::runtime::Handle;

use super::compose::{
    self, Answer, ComposeRequest, ComposeResult, MAX_RESULT_BODY, Runner, ViewContext,
};
use super::held::Edits;
use crate::host::Host;

/// The runner over `host`'s window.
pub struct HostRunner {
    host: Weak<Host>,
    handle: Handle,
    agent: ureq::Agent,
}

impl HostRunner {
    /// A runner for `host`, its tasks on `handle`.
    #[must_use]
    pub fn new(host: Weak<Host>, handle: Handle) -> Self {
        Self {
            host,
            handle,
            agent: compose::agent(),
        }
    }
}

/// `view`'s context for a request to `url`.
async fn view_context(host: &Host, view: ViewId, url: &str) -> ViewContext {
    let cookies = host
        .cdp_send(view, "Network.getCookies", json!({ "urls": [url] }))
        .await
        .ok()
        .and_then(|answer| {
            answer["cookies"].as_array().map(|list| {
                list.iter()
                    .filter_map(|cookie| {
                        Some((
                            cookie["name"].as_str()?.to_owned(),
                            cookie["value"].as_str()?.to_owned(),
                        ))
                    })
                    .collect::<Vec<_>>()
            })
        })
        .unwrap_or_default();
    let emulation = host.emulation(view);
    let mut user_agent = emulation
        .as_ref()
        .and_then(|emulation| emulation.user_agent.clone());
    if user_agent.is_none() {
        user_agent = host
            .cdp()
            .call("Browser.getVersion", json!({}), None)
            .await
            .ok()
            .and_then(|answer| answer["userAgent"].as_str().map(str::to_owned));
    }
    let headers = emulation
        .map(|emulation| emulation.headers.into_iter().collect())
        .unwrap_or_default();
    ViewContext {
        cookies,
        user_agent,
        headers,
    }
}

/// The captured request `request_id` of `view`: its method, URL and headers (the hop and
/// length ones left out) from the views' buffers.
fn captured(view: ViewId, request_id: &str) -> Option<ComposeRequest> {
    let entry = crate::plugins::console::request(view, request_id)?;
    Some(ComposeRequest {
        method: entry.method.clone(),
        url: entry.url.clone(),
        headers: compose::resendable(entry.request_headers()),
        body: None,
    })
}

async fn sends(
    agent: ureq::Agent,
    request: ComposeRequest,
    times: u32,
    view: Option<ViewId>,
) -> Vec<ComposeResult> {
    let mut results = Vec::with_capacity(times as usize);
    for _ in 0..times {
        let (agent, request) = (agent.clone(), request.clone());
        let result = tokio::task::spawn_blocking(move || {
            compose::send(&agent, &request, MAX_RESULT_BODY, view)
        })
        .await;
        match result {
            Ok(result) => results.push(result),
            Err(_) => break,
        }
    }
    results
}

impl Runner for HostRunner {
    fn compose(
        &self,
        view: Option<ViewId>,
        request: ComposeRequest,
        times: u32,
    ) -> Answer<Vec<ComposeResult>> {
        let host = self.host.clone();
        let agent = self.agent.clone();
        Box::pin(async move {
            let context = match (view, host.upgrade()) {
                (Some(view), Some(host)) => view_context(&host, view, &request.url).await,
                _ => ViewContext::default(),
            };
            let prepared = compose::with_context(&request, &context);
            sends(agent, prepared, times, view).await
        })
    }

    fn repeat(
        &self,
        view: ViewId,
        request_id: String,
        edits: Edits,
        times: u32,
    ) -> Answer<Result<Vec<ComposeResult>, String>> {
        let host = self.host.clone();
        let agent = self.agent.clone();
        Box::pin(async move {
            let host = host.upgrade().ok_or("the window is closed")?;
            let mut original =
                captured(view, &request_id).ok_or("the view no longer lists this request")?;
            original.body = host
                .cdp_send(
                    view,
                    "Network.getRequestPostData",
                    json!({ "requestId": request_id }),
                )
                .await
                .ok()
                .and_then(|answer: Value| {
                    answer["postData"]
                        .as_str()
                        .map(|data| data.as_bytes().to_vec())
                });
            let edited = compose::with_edits(&original, &edits);
            let context = view_context(&host, view, &edited.url).await;
            let prepared = compose::with_context(&edited, &context);
            Ok(sends(agent, prepared, times, Some(view)).await)
        })
    }

    fn spawn(&self, task: Answer<()>) {
        self.handle.spawn(task);
    }
}

/// The runner of `host`'s window.
#[must_use]
pub fn for_host(host: &Arc<Host>) -> Arc<dyn Runner> {
    Arc::new(HostRunner::new(
        Arc::downgrade(host),
        host.runtime().clone(),
    ))
}
