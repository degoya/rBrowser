//! The recorder's steps for the Recorder panel while it records (RB-090-89): the steps from a
//! place on, `onlyIn` filled in as the saved recording has it, and the ones that changed since
//! the panel last got them ([`Recorder::take_changed`]) - never the whole recording again.

use std::collections::BTreeSet;

use super::Recorder;
use crate::recorder::model::Step;

impl Recorder {
    /// The steps from place `from` on (none past the end), `onlyIn` filled in.
    pub fn steps_from(&self, from: usize) -> Vec<Step> {
        let all: BTreeSet<&str> = self.names.values().map(String::as_str).collect();
        self.steps
            .entries
            .get(from..)
            .unwrap_or_default()
            .iter()
            .map(|entry| {
                let mut step = entry.step.clone();
                let ran: BTreeSet<&str> = entry
                    .trail
                    .views
                    .iter()
                    .filter_map(|view| self.names.get(view).map(String::as_str))
                    .collect();
                if ran != all {
                    step.only_in = ran.into_iter().map(str::to_owned).collect();
                }
                step
            })
            .collect()
    }

    /// Whether a step was added, changed or taken out since [`Recorder::take_changed`] - a step
    /// changed in place too, which leaves the count as it was.
    pub fn changed(&mut self) -> bool {
        let Some(at) = self.steps.take_dirty() else {
            return false;
        };
        self.steps.touch(at);
        true
    }

    /// Lets the changes so far go: nobody shows them (`showLiveSteps` off).
    pub fn forget_changes(&mut self) {
        self.steps.take_dirty();
    }

    /// The first place a step was added, changed or taken out since the last call, and the steps
    /// from there; `None` when no step changed.
    pub fn take_changed(&mut self) -> Option<(usize, Vec<Step>)> {
        let from = self.steps.take_dirty()?;
        Some((from, self.steps_from(from)))
    }
}

#[cfg(test)]
#[path = "record_live_tests.rs"]
mod tests;
