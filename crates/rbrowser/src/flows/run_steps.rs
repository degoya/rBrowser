//! The step a run going on is at, per view (RB-090-68): every `running` step of a view's
//! progress counts one more ([`RunSteps::on_step`]), so the activity bar shows "step n/m" - `n`
//! the steps the view started, `m` the `step(` calls of the flow's text ([`total_steps`]). A step
//! inside a loop is counted once in `m`; the bar then shows `n` alone once it passes `m`. The
//! pushed `flowStep` carries the number and the total, `getFlowRun` each view's step
//! ([`current`]), so a page that opens during a run shows it too.

use std::collections::BTreeMap;
use std::sync::Mutex;

use rb_ipc::{FlowStep, FlowStepAt};

use crate::cdp::lock;

/// The calls of `step(` in a flow's text - not in a line comment, not a method (`test.step`),
/// not part of a longer name; `None` without any.
#[must_use]
pub fn total_steps(source: &str) -> Option<u32> {
    let mut count: u32 = 0;
    for line in source.lines() {
        let code = line.split("//").next().unwrap_or_default();
        let bytes = code.as_bytes();
        let mut from = 0;
        while let Some(found) = code[from..].find("step") {
            let at = from + found;
            let end = at + "step".len();
            let before = at.checked_sub(1).map(|index| bytes[index]);
            let ours = before.is_none_or(|byte| {
                !(byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'$' | b'.'))
            });
            let call = code[end..].trim_start().starts_with('(');
            let whole = bytes
                .get(end)
                .is_none_or(|&byte| !(byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'$')));
            if ours && whole && call && !inside_block_comment(code, at) {
                count = count.saturating_add(1);
            }
            from = end;
        }
    }
    (count > 0).then_some(count)
}

/// Whether `at` lies in a `/* ... */` comment of `line` (one that opens and closes on the line).
fn inside_block_comment(line: &str, at: usize) -> bool {
    let mut from = 0;
    while let Some(open) = line[from..].find("/*") {
        let open = from + open;
        let close = line[open + 2..]
            .find("*/")
            .map_or(line.len(), |close| open + 2 + close);
        if (open..close).contains(&at) {
            return true;
        }
        from = close.min(line.len());
        if from >= line.len() {
            break;
        }
    }
    false
}

/// The steps of one run.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RunSteps {
    run_id: String,
    total: Option<u32>,
    views: BTreeMap<u32, FlowStepAt>,
}

impl RunSteps {
    /// The run `run_id`, no step yet.
    #[must_use]
    pub fn new(run_id: &str) -> Self {
        Self {
            run_id: run_id.to_owned(),
            ..Self::default()
        }
    }

    /// The flow's steps (`total_steps`).
    pub fn set_total(&mut self, total: Option<u32>) {
        self.total = total;
    }

    /// Counts `step` when it starts and gives it its number; every step gets the total.
    pub fn on_step(&mut self, step: &mut FlowStep) {
        step.total = self.total;
        if step.state != "running" {
            return;
        }
        let number = self
            .views
            .get(&step.view)
            .map_or(1, |at| at.number.saturating_add(1));
        self.views.insert(
            step.view,
            FlowStepAt {
                number,
                name: step.name.clone(),
            },
        );
        step.number = Some(number);
    }

    /// The step `view` is at.
    #[must_use]
    pub fn at(&self, view: u32) -> Option<FlowStepAt> {
        self.views.get(&view).cloned()
    }
}

/// The run going on's steps.
static CURRENT: Mutex<Option<RunSteps>> = Mutex::new(None);

/// A run starts: its steps from nothing.
pub fn begin(run_id: &str) {
    *lock(&CURRENT) = Some(RunSteps::new(run_id));
}

/// The total of the run `run_id`'s flow.
pub fn set_total(run_id: &str, total: Option<u32>) {
    if let Some(steps) = lock(&CURRENT)
        .as_mut()
        .filter(|steps| steps.run_id == run_id)
    {
        steps.set_total(total);
    }
}

/// Counts `step` of the run `run_id` ([`RunSteps::on_step`]); a step of another run is left as
/// it is.
pub fn note(run_id: &str, step: &mut FlowStep) {
    if let Some(steps) = lock(&CURRENT)
        .as_mut()
        .filter(|steps| steps.run_id == run_id)
    {
        steps.on_step(step);
    }
}

/// The total and each view's step of the run `run_id`; `None` for a run not tracked.
#[must_use]
pub fn current(run_id: &str) -> Option<(Option<u32>, BTreeMap<u32, FlowStepAt>)> {
    lock(&CURRENT)
        .as_ref()
        .filter(|steps| steps.run_id == run_id)
        .map(|steps| (steps.total, steps.views.clone()))
}

#[cfg(test)]
#[path = "run_steps_tests.rs"]
mod tests;
