//! One consent probe (RB-094-09) over a [`ProbeDriver`]: open a fresh session, load the page and
//! wait for it to go quiet, capture (requests, cookies, the read collector's storage keys and
//! banner), click "Reject all" with the plugin's reject collector when it asks for that step,
//! wait again and capture once more - then discard the session, whatever happened before. The
//! open and the captures are bounded by [`PROBE_DEADLINE`], the discard by [`DISCARD_DEADLINE`];
//! a discard that does not finish, and a probe its caller drops mid-way (an MCP cancel), leave
//! the session to [`ProbeDriver::abandon`]. The driver over CDP and a windowless browser is
//! `super::cdp`; tests drive a fake.
//!
//! A [`ProbeError`] is a fixed sentence, never page text, so a failure can be logged and
//! answered without anything the page said or set.

use std::fmt;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::Value;

use super::args::ProbeRequest;
use super::shape::{Capture, Phase, banner_of, cookies_of, keys_of};

/// The longest a probe may take from the open to the last capture; the session is discarded
/// after it all the same.
pub const PROBE_DEADLINE: Duration = Duration::from_secs(60);
/// The longest the discard may take before the session is abandoned instead.
pub const DISCARD_DEADLINE: Duration = Duration::from_secs(10);

/// Why a probe failed: a fixed sentence of the host's own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProbeError(pub &'static str);

impl fmt::Display for ProbeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

/// The probe session's browser, as the probe drives it.
#[async_trait]
pub trait ProbeDriver: Send {
    /// Opens a fresh session of its own with one windowless page, apart from every view's
    /// session; whether it held no cookie.
    async fn open(&mut self) -> Result<bool, ProbeError>;
    /// Loads `url` and waits for its `load` and for the network to go quiet (bounded).
    async fn load(&mut self, url: &str) -> Result<(), ProbeError>;
    /// Waits until no request came for a while (bounded), after a click.
    async fn settle(&mut self);
    /// Runs `expression` in an isolated world of the page's main frame; its value.
    async fn evaluate(&mut self, expression: &str) -> Result<Value, ProbeError>;
    /// The `Network.requestWillBeSent` parameters of the page since the last call.
    fn take_requests(&mut self) -> Vec<Value>;
    /// Every cookie of the session, a `Network.getAllCookies` answer.
    async fn cookies(&mut self) -> Result<Value, ProbeError>;
    /// Deletes the session's cookies and closes its browser; whether both happened.
    async fn discard(&mut self) -> bool;
    /// Lets go of the session at once, without waiting: the browser closed, the collection
    /// stopped. For a probe dropped mid-way or a discard that did not finish; nothing after a
    /// discard that did.
    fn abandon(&mut self);
}

/// Abandons the driver's session when dropped before [`Discarding::done`].
struct Discarding<'a> {
    driver: &'a mut dyn ProbeDriver,
    done: bool,
}

impl Drop for Discarding<'_> {
    fn drop(&mut self) {
        if !self.done {
            self.driver.abandon();
        }
    }
}

async fn open_and_capture(
    driver: &mut dyn ProbeDriver,
    request: &ProbeRequest,
    now: u64,
) -> Result<Capture, ProbeError> {
    let fresh = driver.open().await?;
    capture(driver, request, now, fresh).await
}

fn requests_of(driver: &mut dyn ProbeDriver, phase: &mut Phase) {
    for params in driver.take_requests() {
        phase.add_request(&params);
    }
}

async fn capture(
    driver: &mut dyn ProbeDriver,
    request: &ProbeRequest,
    now: u64,
    fresh: bool,
) -> Result<Capture, ProbeError> {
    driver.load(&request.url).await?;
    let read = driver.evaluate(&request.read).await?;
    let mut before = Phase {
        cookies: cookies_of(&driver.cookies().await?),
        storage: keys_of(&read),
        ..Phase::default()
    };
    requests_of(driver, &mut before);
    let mut clicked = None;
    let mut after_reject = None;
    if let Some(reject) = &request.reject {
        let answer = driver.evaluate(reject).await?;
        if answer["clicked"] == Value::Bool(true) {
            driver.settle().await;
            let read = driver.evaluate(&request.read).await?;
            let mut after = Phase {
                storage: keys_of(&read),
                ..Phase::default()
            };
            requests_of(driver, &mut after);
            after.cookies = cookies_of(&driver.cookies().await?);
            after_reject = Some(after);
        }
        clicked = Some(answer);
    }
    Ok(Capture {
        url: request.url.clone(),
        captured_at: now,
        fresh,
        discarded: false,
        banner: banner_of(&read, clicked.as_ref()),
        before,
        after_reject,
    })
}

/// Runs `request` over `driver` at `now` (seconds since the epoch): its capture, the session
/// discarded in every case - abandoned when the discard takes over [`DISCARD_DEADLINE`] or the
/// future is dropped before it ends.
///
/// # Errors
///
/// The driver's [`ProbeError`], or the probe took longer than [`PROBE_DEADLINE`].
pub async fn probe(
    driver: &mut dyn ProbeDriver,
    request: &ProbeRequest,
    now: u64,
) -> Result<Capture, ProbeError> {
    let mut guard = Discarding {
        driver,
        done: false,
    };
    let captured = tokio::time::timeout(
        PROBE_DEADLINE,
        open_and_capture(&mut *guard.driver, request, now),
    )
    .await
    .unwrap_or(Err(ProbeError("the probe took too long")));
    let discarded = match tokio::time::timeout(DISCARD_DEADLINE, guard.driver.discard()).await {
        Ok(discarded) => {
            guard.done = true;
            discarded
        }
        // Dropping the guard abandons the session.
        Err(_) => false,
    };
    drop(guard);
    let mut capture = captured?;
    capture.discarded = discarded;
    Ok(capture)
}

#[cfg(test)]
#[path = "run_tests.rs"]
mod tests;
