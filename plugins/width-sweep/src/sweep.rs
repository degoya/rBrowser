//! The sweep over one view: the coarse pass, then the refinement between the widths whose issues
//! differ ([`crate::plan`]), each width set through the host's width lease ([`SET`]) and looked
//! at ([`crate::probe`]), `host.report-progress` after each. It ends at the last width, at
//! Escape pressed in the view, or when the lease was ended from outside (the `cancel` call: the
//! next width is refused as `plugins.error.cancelled`). However it ends - a failure included -
//! the lease is ended ([`CLEAR`]), so the view has its own width and sync again; the host ends
//! it too when the call ends, should this call not get that far.

use rb_plugin_api::{Failure, FailureKind, PluginHost, ViewId};
use serde_json::json;

use crate::plan::{self, Samples, Span};
use crate::probe::{self, CLEAR, SET};

/// The most widths the refinement adds: past it the ranges keep the coarse step's precision.
pub const MAX_REFINE: usize = 2000;

/// What to sweep.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Request {
    pub view: ViewId,
    pub span: Span,
    pub kinds: Vec<String>,
}

/// How a sweep ended.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum End {
    /// Every width was looked at.
    #[default]
    Done,
    /// The lease was ended from outside: the `cancel` command or tool.
    Cancelled,
    /// Escape was pressed in the view.
    Escaped,
}

/// What a sweep saw.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Outcome {
    pub end: End,
    pub samples: Samples,
    /// The widths looked at.
    pub probes: u32,
    /// The widths at which the page had another width than the one set (a host layout took the
    /// view back in between, or a mobile page without a viewport meta tag).
    pub mismatched: u32,
}

/// Sweeps `request.view` and ends the lease, whatever happened.
///
/// # Errors
///
/// A width the host refused for another reason than a cancel, or the page's failure.
pub fn sweep(host: &mut dyn PluginHost, request: &Request) -> Result<Outcome, Failure> {
    let outcome = passes(host, request);
    if let Err(failure) = host.cdp_send(request.view, CLEAR, "{}") {
        host.log("debug", &format!("the width lease did not end: {failure}"));
    }
    outcome
}

fn passes(host: &mut dyn PluginHost, request: &Request) -> Result<Outcome, Failure> {
    let mut outcome = Outcome::default();
    let coarse = plan::coarse(request.span);
    let mut total = count(coarse.len());
    for (done, &width) in coarse.iter().enumerate() {
        if let Some(end) = step(host, request, width, done == 0, &mut outcome)? {
            outcome.end = end;
            return Ok(outcome);
        }
        host.report_progress(count(done + 1), total);
    }
    let fine = plan::refine(&outcome.samples, MAX_REFINE);
    let before = count(coarse.len());
    total = total.saturating_add(count(fine.len()));
    for (done, &width) in fine.iter().enumerate() {
        if let Some(end) = step(host, request, width, false, &mut outcome)? {
            outcome.end = end;
            return Ok(outcome);
        }
        host.report_progress(before.saturating_add(count(done + 1)), total);
    }
    Ok(outcome)
}

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// One width: set, looked at, kept; `Some` when the sweep ends here.
fn step(
    host: &mut dyn PluginHost,
    request: &Request,
    width: u32,
    first: bool,
    outcome: &mut Outcome,
) -> Result<Option<End>, Failure> {
    let params = json!({ "width": width }).to_string();
    match host.cdp_send(request.view, SET, &params) {
        Ok(_) => {}
        Err(failure) if failure.kind == FailureKind::Cancelled => return Ok(Some(End::Cancelled)),
        Err(failure) => return Err(failure),
    }
    let answer = probe::probe(host, request.view, width, &request.kinds, first)?;
    if answer.escape {
        return Ok(Some(End::Escaped));
    }
    outcome.probes += 1;
    if answer.width != width {
        outcome.mismatched += 1;
    }
    outcome
        .samples
        .insert(width, answer.issues.into_iter().collect());
    Ok(None)
}

#[cfg(test)]
#[path = "sweep_tests.rs"]
mod tests;
