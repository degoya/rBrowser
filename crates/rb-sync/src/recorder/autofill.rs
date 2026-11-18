//! The form autofill in a recording (RB-090-78). The `form-autofill` plugin fills fields by
//! script, so the sync agent - which captures trusted input only, and must: a page's own scripts
//! never become steps - sees none of its text and selects. The plugin reports, per view, what it
//! set (its `record_fill` host post, the app's `recorder::autofill`), and
//! [`super::Recorder::on_autofill`] takes each field as a step - `fill`, `check`/`uncheck` or
//! `select` with the value set - in the views that set it so: the posts of one fill, one per
//! view, arrive back to back, and a field a later post sets as an earlier one did joins that step.
//! A password field (`sensitive`) and a field of `secretFields` are recorded as typed input is
//! ([`super::secrets`]): as the placeholder `{{secret:<name>}}`, never the value.
//!
//! The autofill clicks checkboxes and radios, and the browser reports the `change` of such a
//! click as trusted: the agent records it in the active view. That step and the autofill's are
//! one - the agent's goes, whether it came before the autofill's post (`Echoes::before`, among
//! the toggles at the end of the steps, within [`FILL_WINDOW_MS`]) or after it (`Echoes::after`,
//! within [`ECHO_WINDOW_MS`]).

use std::collections::BTreeSet;

use rb_engine::ViewId;
use serde::Deserialize;

use super::cleanup::{Builder, Trail, same_element};
use super::model::{Action, Locator, secret_placeholder};
use super::secrets::{is_secret, secret_name};
use crate::event::ElementTarget;

/// How long after an autofill's post the agent's record of one of its clicks may come.
pub const ECHO_WINDOW_MS: u64 = 5_000;
/// How long one fill may take: the plugin's limit for one call (its manifest, 30 s).
pub const FILL_WINDOW_MS: u64 = 30_000;
/// The most fields of one view a post is taken with (the page script reads at most 300).
pub const MAX_FIELDS: usize = 300;

/// What the autofill did to a field.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AutofillKind {
    /// Set a text field's value.
    Fill,
    /// Checked a checkbox or a radio.
    Check,
    /// Unchecked a checkbox.
    Uncheck,
    /// Chose a select's options.
    Select,
}

/// One field the autofill set in a view, as its page script reports it.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutofillField {
    /// The field, in the shape the agent gives an element (`agent/src/selector.ts`).
    pub target: ElementTarget,
    /// What was done.
    pub kind: AutofillKind,
    /// The value set: a text field's, a select's (first) chosen option's; none for a password.
    #[serde(default)]
    pub value: Option<String>,
    /// A select's chosen options' indexes.
    #[serde(default)]
    pub selected: Vec<u32>,
    /// A password field, as the agent marks one.
    #[serde(default)]
    pub sensitive: bool,
}

/// What the autofill set in one view.
#[derive(Clone, Debug, PartialEq)]
pub struct AutofillView {
    /// The view.
    pub view: ViewId,
    /// Its fields, in document order.
    pub fields: Vec<AutofillField>,
}

/// The step of `field`; a secret one's value as its placeholder.
pub(crate) fn action_of(field: &AutofillField, secret_fields: &[String]) -> Action {
    let target = Locator::from_target(&field.target, None);
    let secret = is_secret(field.sensitive, &field.target, secret_fields);
    match field.kind {
        AutofillKind::Fill => Action::Fill {
            target,
            value: if secret {
                secret_placeholder(&secret_name(&field.target))
            } else {
                field.value.clone().unwrap_or_default()
            },
            secret,
        },
        AutofillKind::Check => Action::Check { target },
        AutofillKind::Uncheck => Action::Uncheck { target },
        AutofillKind::Select => Action::Select {
            target,
            value: if secret { None } else { field.value.clone() },
            indexes: field.selected.clone(),
        },
    }
}

fn is_toggle(action: &Action) -> bool {
    matches!(action, Action::Check { .. } | Action::Uncheck { .. })
}

/// Whether `a` and `b` set one checkbox or radio the same way.
fn same_toggle(a: &Action, b: &Action) -> bool {
    match (a, b) {
        (Action::Check { target: x }, Action::Check { target: y })
        | (Action::Uncheck { target: x }, Action::Uncheck { target: y }) => same_element(x, y),
        _ => false,
    }
}

impl Builder {
    /// The autofill's step `action` in `view` (breakpoint `name`) at `at_ms`: it joins the step of
    /// the same fill that sets the same in other views - one of the autofill's steps at the end -
    /// else it is a new step. Answers whether anything changed.
    pub(crate) fn autofill(
        &mut self,
        action: Action,
        name: &str,
        view: ViewId,
        at_ms: u64,
        echoes: &mut Echoes,
    ) -> bool {
        let tail = self
            .entries
            .iter()
            .rposition(|entry| !entry.trail.autofill)
            .map_or(0, |at| at + 1);
        if let Some(entry) = self.entries[tail..]
            .iter_mut()
            .find(|entry| entry.step.action == action && !entry.trail.views.contains(&view))
        {
            entry.trail.views.insert(view);
            return true;
        }
        echoes.before(self, &action, at_ms);
        let trail = Trail {
            views: BTreeSet::from([view]),
            origins: Vec::new(),
            autofill: true,
        };
        self.push(action, name, at_ms, trail);
        true
    }
}

/// The autofill's toggles whose record by the agent has not come yet.
#[derive(Debug, Default)]
pub(crate) struct Echoes {
    waiting: Vec<(Action, u64)>,
}

impl Echoes {
    /// Before the autofill's new step `action` at `at_ms` goes in: the agent's record of its
    /// click, when it came first, goes - a toggle among those at the end of the steps (the
    /// autofill's own passed over), recorded within [`FILL_WINDOW_MS`]. Else the action waits
    /// for it.
    pub fn before(&mut self, steps: &mut Builder, action: &Action, at_ms: u64) {
        if !is_toggle(action) {
            return;
        }
        for at in (0..steps.entries.len()).rev() {
            let entry = &steps.entries[at];
            if entry.trail.autofill {
                continue;
            }
            if !is_toggle(&entry.step.action)
                || at_ms.saturating_sub(entry.step.at_ms) > FILL_WINDOW_MS
            {
                break;
            }
            if same_toggle(&entry.step.action, action) {
                steps.entries.remove(at);
                return;
            }
        }
        self.waiting.push((action.clone(), at_ms));
    }

    /// The agent's event at `at_ms` made a new last step: when it is a waiting toggle's click,
    /// it goes.
    pub fn after(&mut self, steps: &mut Builder, at_ms: u64) {
        self.waiting
            .retain(|(_, since)| at_ms.saturating_sub(*since) <= ECHO_WINDOW_MS);
        let Some(last) = steps.entries.last() else {
            return;
        };
        if last.trail.autofill {
            return;
        }
        if let Some(at) = self
            .waiting
            .iter()
            .position(|(action, _)| same_toggle(action, &last.step.action))
        {
            self.waiting.remove(at);
            steps.pop_last();
        }
    }

    /// Whether a toggle waits; the recorder looks at the agent's steps only then.
    pub fn any(&self) -> bool {
        !self.waiting.is_empty()
    }
}

#[cfg(test)]
#[path = "autofill_tests.rs"]
mod tests;
