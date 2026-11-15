//! One paused request's answer (RB-080-37), on a task of its own: the rules' one call (a mock
//! after its delay, a block, a rewrite, map remote), a local file ([`super::local`]), a rewritten
//! response (the body read with `Fetch.getResponseBody`, headers and body changed, fulfilled), or
//! a hold - the request waits in [`super::held::Held`] for the person's continue or abort, and
//! goes on unchanged after `breakpointTimeoutMs`. Every path ends in a CDP call, with the
//! fallbacks that keep a request from staying paused.

use std::sync::Arc;
use std::time::Duration;

use base64::Engine as _;
use rb_engine::ViewId;
use serde_json::{Value, json};

use super::debug::Verdict;
use super::document::Stage;
use super::fetch::{self, Decision, Outcome, Paused};
use super::held::{self, HeldRequest, HeldResponse, Resolution};
use super::local::{self, LocalError};
use super::rewrite;
use super::state::Rules;
use crate::cdp::CdpRouter;

/// The most bytes of a local file served.
pub const MAX_LOCAL_BYTES: u64 = 32 * 1024 * 1024;

/// Answers one paused request of `view`'s `session` on a task of its own.
pub(super) fn answer(
    rules: &Arc<Rules>,
    cdp: &Arc<CdpRouter>,
    view: ViewId,
    session: String,
    params: &Value,
) {
    let Some(request) = Paused::of(params) else {
        return;
    };
    let verdict = rules.verdict(view, &request);
    let rules = Arc::clone(rules);
    let cdp = Arc::clone(cdp);
    tokio::spawn(async move {
        match verdict {
            Verdict::Outcome(Outcome::Call(decision)) => {
                call_with_fallbacks(&cdp, &session, &decision).await;
            }
            Verdict::Outcome(Outcome::Local {
                root, file, mime, ..
            }) => {
                let decision = local_decision(&request, root, file, mime).await;
                call_with_fallbacks(&cdp, &session, &decision).await;
            }
            Verdict::Outcome(Outcome::Response {
                rule,
                status,
                response_headers,
                headers,
                body,
                ..
            }) => {
                let max = rules.settings().max_body_bytes;
                let decision = response_decision(
                    &cdp,
                    &session,
                    &request,
                    status,
                    &response_headers,
                    &headers,
                    body.as_ref(),
                    max,
                )
                .await;
                if decision.rule.is_none()
                    && let Some(plugin) = rules.owner_of(&rule)
                {
                    rules.note_skipped(&plugin, &rule);
                }
                call_with_fallbacks(&cdp, &session, &decision).await;
            }
            Verdict::Hold(breakpoint) => {
                hold(&rules, &cdp, view, session, request, breakpoint).await;
            }
        }
    });
}

/// Sends `decision` after its delay; when it fails, its fallbacks in order.
async fn call_with_fallbacks(cdp: &CdpRouter, session: &str, decision: &Decision) {
    if !decision.delay.is_zero() {
        tokio::time::sleep(decision.delay).await;
    }
    let Err(error) = cdp
        .call(decision.method, decision.params.clone(), Some(session))
        .await
    else {
        return;
    };
    tracing::debug!(%error, method = decision.method, "a paused request was not answered");
    for (method, params) in fetch::fallbacks(decision) {
        match cdp.call(method, params, Some(session)).await {
            Ok(_) => return,
            Err(error) => tracing::debug!(%error, method, "a fallback answer failed"),
        }
    }
}

fn fulfill(request_id: &str, status: u16, headers: &[(String, String)], body: &[u8]) -> Value {
    json!({
        "requestId": request_id,
        "responseCode": status,
        "responseHeaders": headers.iter().map(|(name, value)| json!({ "name": name, "value": value })).collect::<Vec<_>>(),
        "body": base64::engine::general_purpose::STANDARD.encode(body),
    })
}

fn decision(rule: &str, method: &'static str, params: Value) -> Decision {
    Decision {
        rule: Some(rule.to_owned()),
        delay: Duration::ZERO,
        method,
        params,
    }
}

/// A local file as `Fetch.fulfillRequest`: 200 with the file, 404 without one, 403 for a path
/// that would leave the folder.
async fn local_decision(
    request: &Paused,
    root: std::path::PathBuf,
    file: Option<std::path::PathBuf>,
    mime: Option<String>,
) -> Decision {
    let found = match file {
        Some(file) => Ok(file),
        None => local::file_for(&root, &request.url),
    };
    let (status, mime, body) = match found {
        Ok(path) => match read_bounded(&path).await {
            Ok(bytes) => (
                200,
                mime.unwrap_or_else(|| local::mime_of(&path).to_owned()),
                bytes,
            ),
            Err(reason) => (500, "text/plain".to_owned(), reason.into_bytes()),
        },
        Err(LocalError::NotFound) => (
            404,
            "text/plain".to_owned(),
            b"rBrowser: no such file below the mapped folder".to_vec(),
        ),
        Err(LocalError::Outside) => (
            403,
            "text/plain".to_owned(),
            b"rBrowser: the path leaves the mapped folder".to_vec(),
        ),
    };
    let mut headers = vec![("content-type".to_owned(), mime)];
    if let Some((_, origin)) = request
        .headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("origin"))
    {
        headers.push(("access-control-allow-origin".to_owned(), origin.clone()));
        headers.push((
            "access-control-allow-credentials".to_owned(),
            "true".to_owned(),
        ));
    }
    decision(
        "mapLocal",
        "Fetch.fulfillRequest",
        fulfill(&request.request_id, status, &headers, &body),
    )
}

async fn read_bounded(path: &std::path::Path) -> Result<Vec<u8>, String> {
    let size = tokio::fs::metadata(path)
        .await
        .map_err(|error| error.to_string())?
        .len();
    if size > MAX_LOCAL_BYTES {
        return Err(format!(
            "rBrowser: the file is larger than {MAX_LOCAL_BYTES} bytes"
        ));
    }
    tokio::fs::read(path)
        .await
        .map_err(|error| error.to_string())
}

/// The response's body, decoded, when the view still holds it.
async fn response_body(cdp: &CdpRouter, session: &str, request_id: &str) -> Option<Vec<u8>> {
    let answer = cdp
        .call(
            "Fetch.getResponseBody",
            json!({ "requestId": request_id }),
            Some(session),
        )
        .await
        .ok()?;
    let body = answer["body"].as_str()?;
    if answer["base64Encoded"] == Value::Bool(true) {
        base64::engine::general_purpose::STANDARD.decode(body).ok()
    } else {
        Some(body.as_bytes().to_vec())
    }
}

/// Headers of a response whose body is fulfilled decoded: the encoding and length no longer
/// hold.
/// A rewritten response: `Fetch.fulfillRequest` with the changed body, or
/// `Fetch.continueResponse` with the changed headers alone; a body over `max_body` stays as it
/// is (the decision then names no rule, so the caller counts it skipped).
#[allow(
    clippy::too_many_arguments,
    reason = "the parts of one response rewrite, called once"
)]
async fn response_decision(
    cdp: &CdpRouter,
    session: &str,
    request: &Paused,
    status: u16,
    original: &[(String, String)],
    ops: &[super::action::HeaderOp],
    replace: Option<&super::action::BodyReplace>,
    max_body: usize,
) -> Decision {
    let headers = rewrite::apply_headers(original, ops);
    let id = &request.request_id;
    let mut continue_response = json!({ "requestId": id });
    if !ops.is_empty() {
        continue_response["responseHeaders"] = headers
            .iter()
            .map(|(name, value)| json!({ "name": name, "value": value }))
            .collect();
    }
    let Some(replace) = replace else {
        return decision("rewrite", "Fetch.continueResponse", continue_response);
    };
    let Some(body) = response_body(cdp, session, id).await else {
        return decision("rewrite", "Fetch.continueResponse", continue_response);
    };
    if body.len() > max_body {
        return Decision {
            rule: None,
            ..decision("rewrite", "Fetch.continueResponse", continue_response)
        };
    }
    match rewrite::replace_body(&body, replace) {
        Some(replaced) => decision(
            "rewrite",
            "Fetch.fulfillRequest",
            fulfill(id, status, &held::without_encoding(&headers), &replaced),
        ),
        None => decision("rewrite", "Fetch.continueResponse", continue_response),
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| {
            u64::try_from(since.as_millis()).unwrap_or(u64::MAX)
        })
}

/// Holds `request` until the person decides or the timeout runs out, then answers.
async fn hold(
    rules: &Arc<Rules>,
    cdp: &Arc<CdpRouter>,
    view: ViewId,
    session: String,
    request: Paused,
    breakpoint: String,
) {
    let settings = rules.settings();
    let (body, body_truncated) = match request.stage {
        Stage::Request => (request.post_data.clone(), false),
        Stage::Response => match response_body(cdp, &session, &request.request_id).await {
            Some(body) if body.len() > settings.max_body_bytes => (None, true),
            body => (body, false),
        },
    };
    let held = HeldRequest {
        id: String::new(),
        key: String::new(),
        view,
        session: session.clone(),
        request_id: request.request_id.clone(),
        stage: request.stage,
        method: request.method.clone(),
        url: request.url.clone(),
        headers: request.headers.clone(),
        body,
        body_truncated,
        resource_type: request.resource_type.clone(),
        breakpoint,
        since: now_ms(),
        response: (request.stage == Stage::Response).then(|| HeldResponse {
            status: request.response_status.unwrap_or(200),
            headers: request.response_headers.clone(),
        }),
    };
    let (id, decided) = rules.held().hold(held.clone());
    let resolution = if settings.timeout_ms == 0 {
        decided.await.ok()
    } else {
        tokio::time::timeout(Duration::from_millis(settings.timeout_ms), decided)
            .await
            .ok()
            .and_then(Result::ok)
    };
    rules.held().remove(&id);
    let (method, params) = match resolution {
        Some(Resolution::Continue(edits)) => {
            held::continue_params(&held, &held::unmask_edits(&held, &edits))
        }
        Some(Resolution::Abort) => held::abort_params(&held),
        None => {
            let unchanged = request.unchanged();
            (unchanged.method, unchanged.params)
        }
    };
    let decision = Decision {
        rule: None,
        delay: Duration::ZERO,
        method,
        params,
    };
    call_with_fallbacks(cdp, &session, &decision).await;
}
