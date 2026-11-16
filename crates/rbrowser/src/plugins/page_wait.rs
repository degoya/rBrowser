//! How long the host waits for a page request (RB-080-66): as long as the page does
//! ([`PAGE_TIMEOUT_MILLISECONDS`]), then the request is given up - it leaves the plugin's queue
//! instead of keeping the next ones waiting, and counts as the plugin's failure, so a plugin that
//! hangs is suspended in the end (review point 5). Refusals for a full queue are logged within
//! the page log's gate, with the note when further ones are held (review point 7).

use std::time::Duration;

use rb_engine::ViewId;
use rb_plugin_host::{MAX_PENDING, NO_ANSWER, PAGE_TIMEOUT_MILLISECONDS};
use rb_plugin_types::Failure;
use serde_json::Value;

use super::page_log::{Admit, LOG_BURST, admit};
use super::runtime::Component;

/// A page request of `plugin` in `view` refused: too many wait.
pub fn note_busy(plugin: &str, view: ViewId) {
    match admit(&format!("{plugin}#busy")) {
        Admit::Write => tracing::warn!(
            plugin_id = plugin,
            %view,
            waiting = MAX_PENDING,
            "page requests of a plugin refused: too many wait"
        ),
        Admit::FirstHeld => tracing::info!(
            plugin_id = plugin,
            burst = LOG_BURST,
            "a plugin's page requests are refused more often; further refusals this minute are not logged"
        ),
        Admit::Held => {}
    }
}

/// `component`'s answer to `message`, or [`NO_ANSWER`] when none came within the page's time.
///
/// # Errors
///
/// [`NO_ANSWER`]: the request was given up and counted as the plugin's failure.
pub async fn within_page_time(
    component: &dyn Component,
    plugin: &str,
    view: ViewId,
    message: &Value,
) -> Result<Result<String, Failure>, String> {
    let wait = Duration::from_millis(u64::from(PAGE_TIMEOUT_MILLISECONDS));
    match tokio::time::timeout(wait, component.page_message(view.0, message)).await {
        Ok(raw) => Ok(raw),
        Err(_) => {
            component.gave_up();
            tracing::warn!(
                plugin_id = plugin,
                %view,
                "a page request of a plugin got no answer in time; given up"
            );
            Err(NO_ANSWER.to_owned())
        }
    }
}
