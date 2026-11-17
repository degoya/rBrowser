//! The switches of the bars under every view in the page's context menu: "Breakpoint bar under
//! every view" (RB-080-60, `Leader O B`), "Vitals bar under every view" (RB-090-71,
//! `Leader O W`) and "Lighthouse bar under every view" (RB-090-73, `Leader O Q`), check entries
//! side by side, each shown once its command is registered.

use serde_json::Value;

use super::{Action, Builder, Item, KEY_FOOTER, KEY_LIGHTHOUSE_BAR, KEY_VITALS_BAR, Label, Mark};
use crate::view_footer::defs::{LIGHTHOUSE_TOGGLE, TOGGLE, VITALS_TOGGLE};

impl Builder<'_> {
    /// The bars' check entries, each checked while its bar shows.
    pub(super) fn bars(&mut self) -> Vec<Item> {
        let state = self.state;
        let rows = [
            (TOGGLE, KEY_FOOTER, state.footer_visible),
            (VITALS_TOGGLE, KEY_VITALS_BAR, state.vitals_bar_visible),
            (
                LIGHTHOUSE_TOGGLE,
                KEY_LIGHTHOUSE_BAR,
                state.lighthouse_bar_visible,
            ),
        ];
        rows.into_iter()
            .filter(|(command, _, _)| state.knows(command))
            .map(|(command, key, shown)| {
                let action = Action::Run {
                    command: command.to_owned(),
                    args: Value::Null,
                };
                self.labelled(Label::Key(key), true, Mark::Check(shown), action)
            })
            .collect()
    }
}
