//! What a run leaves in rBrowser's log (RB-090-84). A failed run wrote no line - its reason was
//! only in the shell's notice and `result.json` in the project - so a diagnostics ZIP could not
//! tell why a played recording failed. Now every run that ends logs one `info` line (counts and
//! duration), and every view that failed or timed out one `warn` line: its id, label and
//! breakpoint, the step that was running, the reason, and whether the view was hidden then -
//! placed off screen by a layout mode (horizontal mode's views out of sight) and reported hidden
//! to the browser, whose hidden pages get no animation frames, which Playwright's actions wait
//! for.
//!
//! The reason is the error's first line only - the lines after an `expect` failure quote the
//! expected and the received value, a form field's text -, without colour codes, every URL in it
//! without its query and fragment, masked ([`rb_diag::redact`]) and cut to [`MAX_REASON`]
//! characters. A run that did not start logs its error, masked, at `warn`. A run that skipped
//! views the layout hid, or breakpoints no view of the grid shows, says so ([`skip_notices`]).

use rb_engine::ViewId;
use rb_ipc::{CoreToShell, Notification, NotificationLevel};

use crate::control::Core;

use super::FlowError;
use super::plan::HIDDEN_REASON;
use super::result::{RunResult, ViewStatus};

/// The notice of a run that skipped what the layout hid; `{flow}` and `{breakpoints}`.
pub const HIDDEN_NOTICE: &str = "flows.notice.hidden";
/// The setting the notice links to.
pub const HIDDEN_SETTING: &str = "automation.flows.hiddenViews";
/// The notice of a run whose flow names breakpoints no view of the grid shows; `{flow}` and
/// `{breakpoints}`.
pub const NOT_IN_GRID_NOTICE: &str = "flows.notice.not_in_grid";

/// The most characters of a reason in the log.
pub const MAX_REASON: usize = 300;
/// The reason of a failed view that carries no error.
const NO_REASON: &str = "no reason given";

/// One failed view as the log names it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failure {
    /// The view's id.
    pub view_id: u32,
    /// Its label.
    pub label: String,
    /// Its breakpoint.
    pub breakpoint: Option<String>,
    /// `failed` or `timedOut`.
    pub status: ViewStatus,
    /// The step that was running (`Step 3: hover` in a played recording).
    pub step: Option<String>,
    /// Why, as [`reason`] writes it.
    pub reason: String,
    /// How the view was shown when the run ended.
    pub shown: Shown,
}

/// Whether a view was hidden, as the log names it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Shown {
    /// A layout mode placed it outside the grid area (horizontal mode's views out of sight).
    pub off_screen: bool,
    /// The browser was told it is hidden (the throttling of views without area).
    pub reported_hidden: bool,
}

/// How `view` is shown now in `core`'s window.
pub fn shown_now(core: &Core, view: u32) -> Shown {
    let host = core.host();
    Shown {
        off_screen: host.is_off_screen(ViewId(view)),
        reported_hidden: crate::perf::throttle_state(host, ViewId(view))
            == crate::perf::ThrottleState::Hidden,
    }
}

/// The views of `result` that failed or timed out; `shown` tells how a view (by id) is shown.
pub fn failures(result: &RunResult, shown: impl Fn(u32) -> Shown) -> Vec<Failure> {
    result
        .views
        .iter()
        .filter(|view| matches!(view.status, ViewStatus::Failed | ViewStatus::TimedOut))
        .map(|view| Failure {
            view_id: view.view_id,
            label: view.label.clone(),
            breakpoint: view.breakpoint.clone(),
            status: view.status,
            step: view
                .error
                .as_ref()
                .and_then(|error| error.step.as_deref())
                .map(rb_diag::redact),
            reason: view.error.as_ref().map_or_else(
                || NO_REASON.to_owned(),
                |error| reason(error.summary.as_deref().unwrap_or(&error.message)),
            ),
            shown: shown(view.view_id),
        })
        .collect()
}

/// Logs how the run of `flow` ended: one `info` line, a `warn` line per failed view, or the
/// error of a run that did not start.
pub fn log(flow: &str, outcome: &Result<RunResult, FlowError>, shown: impl Fn(u32) -> Shown) {
    match outcome {
        Ok(result) => {
            let (passed, failed, skipped) = result.counts();
            tracing::info!(
                flow,
                run = result.run_id.as_str(),
                passed,
                failed,
                skipped,
                ms = result.duration_ms,
                "a flow run ended"
            );
            for failure in failures(result, shown) {
                tracing::warn!(
                    flow,
                    run = result.run_id.as_str(),
                    view = failure.view_id,
                    label = failure.label.as_str(),
                    breakpoint = failure.breakpoint.as_deref().unwrap_or("-"),
                    off_screen = failure.shown.off_screen,
                    reported_hidden = failure.shown.reported_hidden,
                    status = ?failure.status,
                    step = failure.step.as_deref().unwrap_or("-"),
                    reason = failure.reason.as_str(),
                    "a view of a flow run failed"
                );
            }
        }
        Err(FlowError::Untrusted) => {
            tracing::info!(flow, "a flow run was refused: the workspace is not trusted");
        }
        Err(error) => {
            tracing::warn!(
                flow,
                error = reason(&error.to_string()).as_str(),
                "a flow run did not start or broke off"
            );
        }
    }
}

/// The notices of `result`'s skips (RB-090-84): the views the layout hid, linking
/// `automation.flows.hiddenViews`, and apart from them the breakpoints the flow names that no
/// view of the grid shows - no layout setting brings those, so no link.
pub fn skip_notices(result: &RunResult) -> Vec<CoreToShell> {
    let hidden: Vec<String> = result
        .views
        .iter()
        .filter(|view| view.reason.as_deref() == Some(HIDDEN_REASON))
        .map(|view| {
            view.breakpoint
                .clone()
                .unwrap_or_else(|| view.label.clone())
        })
        .collect();
    let notice = |key: &str, names: &[String], setting: Option<&str>| {
        CoreToShell::Notification(Notification {
            level: NotificationLevel::Info,
            message_key: key.to_owned(),
            args: [
                ("flow".to_owned(), result.flow.clone()),
                ("breakpoints".to_owned(), names.join(", ")),
            ]
            .into_iter()
            .collect(),
            settings_key: setting.map(str::to_owned),
        })
    };
    let mut notices = Vec::new();
    if !hidden.is_empty() {
        notices.push(notice(HIDDEN_NOTICE, &hidden, Some(HIDDEN_SETTING)));
    }
    if !result.skipped_breakpoints.is_empty() {
        notices.push(notice(
            NOT_IN_GRID_NOTICE,
            &result.skipped_breakpoints,
            None,
        ));
    }
    notices
}

/// `message` as the log names it: its first line that is not blank, without colour codes and
/// URL queries, masked and cut to [`MAX_REASON`] characters (an ellipsis marks the cut).
pub fn reason(message: &str) -> String {
    let Some(first) = message.lines().map(str::trim).find(|line| !line.is_empty()) else {
        return NO_REASON.to_owned();
    };
    let plain = without_colours(first);
    let masked = rb_diag::redact(&without_queries(plain.trim()));
    if masked.chars().count() > MAX_REASON {
        let mut cut: String = masked.chars().take(MAX_REASON).collect();
        cut.push('\u{2026}');
        cut
    } else {
        masked
    }
}

/// `text` without ANSI escape sequences (`ESC [ ... letter`).
fn without_colours(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '\u{1b}' {
            out.push(c);
            continue;
        }
        // The sequence ends at its first letter.
        for next in chars.by_ref() {
            if next.is_ascii_alphabetic() {
                break;
            }
        }
    }
    out
}

/// `text` with every `http://` or `https://` URL cut before its query or fragment.
fn without_queries(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = ["http://", "https://"]
        .iter()
        .filter_map(|scheme| rest.find(scheme))
        .min()
    {
        let (before, url_on) = rest.split_at(start);
        out.push_str(before);
        let end = url_on
            .find(|c: char| c.is_whitespace() || matches!(c, '\'' | '"' | '`' | '<' | '>' | ')'))
            .unwrap_or(url_on.len());
        let (url, after) = url_on.split_at(end);
        let kept = url.find(['?', '#']).map_or(url, |cut| &url[..cut]);
        out.push_str(kept);
        // The sentence's `;`, `,` or `.` right after a dropped query stays.
        if kept.len() < url.len()
            && let Some(mark @ (';' | ',' | '.')) = url.chars().last()
        {
            out.push(mark);
        }
        rest = after;
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
#[path = "report_tests.rs"]
mod tests;
