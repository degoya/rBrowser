//! A click on the label of a checkbox or radio (RB-090-93). The agent names the control the label
//! goes on to (`MouseData::control`) and sends no click of its own for it; the control's `change`
//! that follows belongs to the label's click. The click stays a click: played back, it toggles
//! the control as the page expects, where a `check` of a visually hidden checkbox would wait for
//! good. The views it ran in stay those of the click and the change.
//!
//! The change comes in the click's task or never - a radio checked already, a page that cancels
//! the toggle: any event but the control's own focus, `input` or `change` ends the wait, so a
//! later change of the control is a step of its own.

use super::{Builder, Trail, merge};
use crate::event::SyncEvent;
use crate::recorder::model::{Action, Locator};

impl Builder {
    /// The click step just pushed went on to the checkbox or radio at `control`, if any.
    pub(super) fn label_clicked(&mut self, control: Option<&str>) {
        self.label_control = control.map(|control| (self.next_id, control.to_owned()));
    }

    /// Forgets the label's control unless `event` (in `frame`) is its focus, `input` or `change`.
    pub(super) fn label_event(&mut self, event: &SyncEvent, frame: Option<&[String]>) {
        let Some((_, control)) = &self.label_control else {
            return;
        };
        let target = match event {
            SyncEvent::Focus(data) => Some(&data.target),
            SyncEvent::Input(input) => Some(&input.target),
            SyncEvent::Change(change) => Some(&change.target),
            _ => None,
        };
        let own =
            target.is_some_and(|target| Locator::from_target(target, frame).css_path() == control);
        if !own {
            self.label_control = None;
        }
    }

    /// Whether the `change` of the checkbox or radio at `path` is the one the last step - the
    /// click on its label - caused; it is then part of that step, with the `carried` trail of
    /// the control's fill. Only the first `change` after the click is.
    pub(super) fn label_change(
        &mut self,
        path: &str,
        kind: &'static str,
        carried: &mut Option<Trail>,
    ) -> bool {
        let last_click = self
            .entries
            .last()
            .filter(|entry| matches!(entry.step.action, Action::Click { .. }))
            .map(|entry| entry.step.id);
        let caused = matches!(
            &self.label_control,
            Some((id, control)) if Some(*id) == last_click && control == path
        );
        if !caused {
            return false;
        }
        self.label_control = None;
        let Some(last) = self.last_entry() else {
            return false;
        };
        merge(&mut last.trail, carried.take());
        last.trail.add_origin(kind, path);
        true
    }
}

#[cfg(test)]
#[path = "cleanup_label_tests.rs"]
mod tests;
