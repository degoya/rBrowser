//! The "Force state" submenu of the page's context menu (RB-090-53): a switch per pseudo-class
//! state for the element under the pointer - checked while the clicked view has that state
//! forced - and "Reset states"; "All views" repeats them with `target: "all"`, checked by what
//! every view has. Each switch is `view.force_state.set { toggle }` with the click in DIP of the
//! view's widget, so the element is resolved as "Inspect element" resolves it (RB-090-51). No
//! logic of its own.

use serde_json::{Value, json};

use super::model::{Action, Builder, Item, Label, Mark};
use crate::force_state::{ForcedMarks, PseudoClass, RESET, SET};

/// The submenu's title.
pub const KEY_MENU: &str = "contextmenu.force_state.menu";
/// "All views".
pub const KEY_ALL: &str = "contextmenu.force_state.all_views";
/// "Reset states".
pub const KEY_RESET: &str = "contextmenu.force_state.reset";

impl Builder<'_> {
    /// The switches for `target`, checked by `marks`, and the reset - enabled when `forced`.
    fn force_entries(
        &mut self,
        marks: &std::collections::BTreeSet<PseudoClass>,
        target: &Value,
        forced: bool,
    ) -> Vec<Item> {
        let state = self.state;
        // Without the click's point or a CDP session there is no element to force on.
        let enabled = state.attached && state.point.is_some();
        let mut items = Vec::new();
        for class in PseudoClass::ALL {
            let args = match state.point {
                Some((x, y)) => json!({
                    "target": target,
                    "view": state.view,
                    "x": x,
                    "y": y,
                    "dip": true,
                    "toggle": class.name(),
                }),
                None => json!({ "target": target }),
            };
            items.push(self.labelled(
                Label::Text(format!(":{}", class.name())),
                enabled,
                Mark::Check(marks.contains(&class)),
                Action::Run {
                    command: SET.to_owned(),
                    args,
                },
            ));
        }
        items.push(Item::Separator);
        items.push(self.run(KEY_RESET, forced, RESET, json!({ "target": target })));
        items
    }

    /// The submenu for the clicked view, with "All views" below.
    pub(super) fn force_state(&mut self, marks: &ForcedMarks) -> Item {
        let view = json!(self.state.view);
        let mut items = self.force_entries(&marks.this_view, &view, !marks.this_view.is_empty());
        let for_all = self.force_entries(&marks.all_views, &json!("all"), marks.any_view);
        items.push(self.submenu(Label::Key(KEY_ALL), for_all));
        self.submenu(Label::Key(KEY_MENU), items)
    }
}

#[cfg(test)]
#[path = "force_state_tests.rs"]
mod tests;
