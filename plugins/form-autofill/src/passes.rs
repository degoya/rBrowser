//! The passes of one view's fill (RB-094-22). A form changes while it is filled: a select shows
//! other fields once a framework has rendered its `change`, a checkbox enables a field or a
//! button. So a pass sets the control fields (select, radio group, checkbox) first and the page
//! script waits after each one it changed until the page has had no DOM change for the quiet
//! window (`settleMilliseconds`, each wait at most [`STEP_CAP_MS`], one `apply` at most
//! [`APPLY_BUDGET_MS`]); then the text fields. When the pass changed the page the fields are read
//! again and a later pass sets what is new: a field with a value planned that no earlier pass
//! set and that holds nothing - a field the page filled itself is left alone, and none is set
//! twice. A field is told apart across reads by the page script's identity of its element
//! ([`Planned::identity`]), never by its key: a field the page inserts before another shifts the
//! keys of unnamed and repeated fields. At most `maxPasses` passes, and a fill stops all of them
//! at `fillBudgetSeconds` ([`crate::settings::Settings::fill_budget_seconds`]): past it no
//! further pass and no further view starts, and the answer says so (`outOfTime`, a view's
//! `stopped: deadline`, a view not started `timeout`). Every view runs them alike, with the
//! same plan by key.

use std::collections::BTreeSet;

use serde::Deserialize;
use serde_json::{Value, json};

use crate::plan::Planned;
use crate::value::Fill;

pub use crate::settings::MAX_SETTLE_MS;

/// The longest one wait for the page to settle takes, in milliseconds, however busy the page.
pub const STEP_CAP_MS: u32 = 3000;
/// The most one `apply` waits in all, in milliseconds (an `evaluate` may take 30 s).
pub const APPLY_BUDGET_MS: u32 = 15_000;

/// How a view's fill paces itself: the settings `settleMilliseconds` and `maxPasses`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Pacing {
    pub settle_ms: u32,
    pub max_passes: u32,
}

impl Pacing {
    /// What `apply` waits with (`request.settle`); none when the setting is 0.
    #[must_use]
    pub fn settle(&self) -> Option<Value> {
        (self.settle_ms > 0).then(|| {
            json!({ "quiet": self.settle_ms, "cap": STEP_CAP_MS.max(self.settle_ms),
                    "budget": APPLY_BUDGET_MS })
        })
    }
}

/// What `apply` says about one field.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct AppliedField {
    pub index: u32,
    pub status: Value,
    pub value: Value,
    pub valid: Value,
    pub errors: Value,
    /// The run changed what the field holds.
    pub changed: Value,
    /// What the recorder takes of the field (`src/record.rs`).
    pub record: Value,
}

/// What `apply` answers.
#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Applied {
    pub fields: Vec<AppliedField>,
    pub form_valid: Value,
    pub blocked_submits: Value,
}

impl Applied {
    /// The page script's answer; one that does not read is an empty one.
    #[must_use]
    pub fn parse(answer: &str) -> Self {
        serde_json::from_str(answer).unwrap_or_default()
    }
}

/// Whether a planned value sets a control field: a select, a radio group or a checkbox.
#[must_use]
pub fn is_control(planned: &Planned) -> bool {
    matches!(
        planned.fill,
        Some(Fill::Check { .. } | Fill::Radio { .. } | Fill::Select { .. })
    )
}

/// The fields a pass sets, control fields first, each group in page order: in the first pass
/// every field with a value planned; in a later one only those no earlier pass set (`done`, by
/// [`Planned::identity`]) that held nothing when read.
#[must_use]
pub fn todo<'a>(planned: &'a [Planned], done: &BTreeSet<String>, first: bool) -> Vec<&'a Planned> {
    let mut list: Vec<&Planned> = planned
        .iter()
        .filter(|entry| entry.fill.is_some() && !done.contains(&entry.identity()))
        .filter(|entry| first || !entry.filled)
        .collect();
    list.sort_by_key(|entry| !is_control(entry));
    list
}

/// What `apply` gets for `todo`, in its order.
#[must_use]
pub fn actions(todo: &[&Planned]) -> Vec<Value> {
    todo.iter().filter_map(|entry| entry.action()).collect()
}

/// What a view's passes did, field by field in the order the page first showed them.
#[derive(Debug, Default)]
pub struct Outcome {
    entries: Vec<(Planned, Option<AppliedField>)>,
    /// The identities of the fields a pass set ([`Planned::identity`]).
    pub done: BTreeSet<String>,
    /// The passes that applied something.
    pub passes: u32,
    /// The last pass's word on the form's validity.
    pub form_valid: Value,
    /// The submits cancelled while the fields were set, all passes.
    pub blocked: u64,
    /// What the recorder takes, pass after pass (`src/record.rs`).
    pub record: Vec<Value>,
}

impl Outcome {
    /// Takes in a pass's plan: a field not seen before is added - in a later pass one that held a
    /// value and is not set is marked `filled`.
    pub fn note(&mut self, planned: &[Planned], first: bool) {
        for entry in planned {
            let identity = entry.identity();
            if self
                .entries
                .iter()
                .any(|(seen, _)| seen.identity() == identity)
            {
                continue;
            }
            let mut entry = entry.clone();
            if !first && entry.filled && entry.fill.is_some() {
                entry.fill = None;
                entry.skip = "filled".to_owned();
            }
            self.entries.push((entry, None));
        }
    }

    /// Takes in what `apply` did with `todo`; whether the page changed - a field changed, gone or
    /// replaced - so that it is read again.
    pub fn absorb(&mut self, todo: &[&Planned], applied: Applied) -> bool {
        self.passes += 1;
        self.form_valid = applied.form_valid;
        self.blocked += applied.blocked_submits.as_u64().unwrap_or(0);
        let changed = applied.fields.iter().any(|field| {
            field.changed.as_bool().unwrap_or(false)
                || field.status == "missing"
                || field.status == "changed"
        });
        for planned in todo {
            let found = applied
                .fields
                .iter()
                .find(|field| field.index == planned.index)
                .cloned();
            if let Some(field) = &found
                && field.status == "done"
            {
                self.done.insert(planned.identity());
                if field.record.is_object() {
                    self.record.push(field.record.clone());
                }
            }
            let entry = ((*planned).clone(), found);
            match self
                .entries
                .iter_mut()
                .find(|(seen, _)| seen.identity() == planned.identity())
            {
                Some(slot) => *slot = entry,
                None => self.entries.push(entry),
            }
        }
        changed
    }

    /// The fields with what the last pass that set each one answered.
    pub fn entries(&self) -> impl Iterator<Item = (&Planned, Option<&AppliedField>)> {
        self.entries
            .iter()
            .map(|(planned, found)| (planned, found.as_ref()))
    }

    /// No field was read at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
#[path = "passes_tests.rs"]
mod tests;
