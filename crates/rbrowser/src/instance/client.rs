//! A second call's side: find the instance of the project in its instance file, send the
//! request with the file's nonce, read the answer.

use std::path::Path;
use std::time::Duration;

use serde_json::Value;
use tokio::io::AsyncWriteExt as _;

use super::protocol::{self, Envelope, MAX_REPLY, Refusal, Request};
use super::{channel, file, server};

/// How long connecting may take before the instance counts as gone.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
/// How long the instance may take to answer (a reload of every view is quick to start); a
/// screenshot gets its budget on top ([`reply_timeout`]).
const REPLY_TIMEOUT: Duration = Duration::from_secs(10);

/// What became of a hand-over.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The instance took the request.
    Handed,
    /// The instance carried out the request and answered this (`screenshot`).
    Answered(Value),
    /// No instance runs for the project (no file, or nobody listens behind it).
    NotRunning,
    /// The instance refused the request; with the reason when a valid request failed.
    Refused(Refusal, Option<String>),
    /// The instance was reached but did not answer properly.
    Failed(String),
}

/// Hands `request` to the running instance of the project at `root`, if there is one.
pub fn hand_over(local: &Path, root: &Path, request: Request) -> Outcome {
    match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime.block_on(hand_over_async(local, root, request)),
        Err(error) => Outcome::Failed(error.to_string()),
    }
}

/// [`hand_over`] inside a runtime.
pub async fn hand_over_async(local: &Path, root: &Path, request: Request) -> Outcome {
    let Some(instance) = file::read(&file::file_path(local, root), local)
        .filter(|instance| file::same_project(&instance.project, root))
    else {
        return Outcome::NotRunning;
    };
    let Ok(Ok(mut conn)) =
        tokio::time::timeout(CONNECT_TIMEOUT, channel::connect(&instance.endpoint)).await
    else {
        return Outcome::NotRunning;
    };
    if matches!(
        request,
        Request::Open {
            background: false,
            ..
        }
    ) {
        channel::allow_foreground(instance.pid);
    }
    let timeout = reply_timeout(&request);
    let line = protocol::encode(&Envelope::new(&instance.nonce, request));
    if let Err(error) = async {
        conn.write_all(&line).await?;
        conn.flush().await
    }
    .await
    {
        return Outcome::Failed(error.to_string());
    }
    let mut reader = tokio::io::BufReader::new(conn);
    match server::read_line_within(&mut reader, MAX_REPLY, timeout)
        .await
        .and_then(|line| protocol::decode_reply(&line))
    {
        Some(reply) if reply.ok => reply.answer.map_or(Outcome::Handed, Outcome::Answered),
        Some(reply) => Outcome::Refused(reply.error.unwrap_or(Refusal::Failed), reply.message),
        None => Outcome::Failed("the running instance did not answer".to_owned()),
    }
}

/// How long the answer to `request` may take: [`REPLY_TIMEOUT`], for a screenshot on top of the
/// instance's own budget (its delay and a margin).
pub fn reply_timeout(request: &Request) -> Duration {
    match request {
        Request::Screenshot { args, .. } => protocol::screenshot_budget(args) + REPLY_TIMEOUT,
        Request::Open { .. } | Request::Reload { .. } | Request::FlowRun { .. } => REPLY_TIMEOUT,
        Request::SitemapCapture { .. } => crate::sitemap::app::DISCOVER_BUDGET + REPLY_TIMEOUT,
    }
}

#[cfg(test)]
#[path = "client_tests.rs"]
mod tests;
