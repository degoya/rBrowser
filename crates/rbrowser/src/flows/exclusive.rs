//! A flow run and a recording that takes steps exclude each other (RB-090-71): the recorder
//! follows the active view's navigations whether or not the view is in sync, so a flow's would
//! become the recording's steps. A flow's start ([`super::app`]) and a recording's start or resume
//! ([`crate::recorder::glue`]) each check the other and claim their own state inside
//! [`exclusively`], so neither slips in between the other's check and claim. Ending either needs
//! no gate.

use std::sync::Mutex;

use crate::cdp::lock;

/// Held from a start's check to its claim.
static GATE: Mutex<()> = Mutex::new(());

/// Runs `work` - a check of the other side and the claim of this one - under the one gate.
pub fn exclusively<T>(work: impl FnOnce() -> T) -> T {
    let _gate = lock(&GATE);
    work()
}

/// The flow running now (a played recording included), by name.
pub fn running_flow() -> Option<String> {
    lock(&super::app::STATE)
        .running
        .as_ref()
        .map(|running| running.flow.clone())
}

#[cfg(test)]
#[path = "exclusive_tests.rs"]
mod tests;
