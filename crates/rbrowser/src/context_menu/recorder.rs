//! The page menu's recorder and flows (RB-090-60): "Recorder" - start, or pause/resume and stop
//! while a recording runs; the check steps for the element under the pointer ("Check: element
//! visible", "Check: text visible", "Wait for element", enabled only while recording); the
//! Recorder panel - and "Run flow" with the project's flows, "Run all flows" and the Flows
//! panel. Every entry runs a command of the registry; a check is `recorder.check` with the click
//! in DIP of the view's widget, so the element is resolved as "Inspect element" resolves it
//! (RB-090-51). No logic of its own.

use serde_json::json;

use super::model::{Action, Builder, Item, Label, Mark};

/// The Recorder submenu's title: the main menu's Flows > Recorder (RB-090-92).
pub const KEY_MENU: &str = super::model::KEY_RECORDER;
/// The Run flow submenu's title, beside the Recorder under Flows.
pub const KEY_FLOWS: &str = "contextmenu.flows.menu";

/// What the recorder and the flows show when the menu opens.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RecorderMenu {
    /// `None` without a recording; `Some(paused)` while one runs.
    pub recording: Option<bool>,
    /// The project's flows by name, as last listed; `None` before the first listing.
    pub flows: Option<Vec<String>>,
}

impl Builder<'_> {
    /// The Recorder submenu.
    pub(super) fn recorder(&mut self) -> Item {
        let state = self.state;
        let mut items = match state.recorder.recording {
            None => vec![self.run(
                "contextmenu.recorder.start",
                true,
                crate::recorder::START,
                json!({ "target": state.view }),
            )],
            Some(paused) => {
                let key = if paused {
                    "contextmenu.recorder.resume"
                } else {
                    "contextmenu.recorder.pause"
                };
                vec![
                    self.run(key, true, crate::recorder::PAUSE, json!({})),
                    self.run(
                        "contextmenu.recorder.stop",
                        true,
                        crate::recorder::STOP,
                        json!({}),
                    ),
                ]
            }
        };
        items.push(Item::Separator);
        // A check needs a recording, the click's point and a CDP session.
        let enabled = state.recorder.recording.is_some() && state.attached && state.point.is_some();
        for (key, kind) in [
            ("contextmenu.recorder.check_visible", "visible"),
            ("contextmenu.recorder.check_text", "text"),
            ("contextmenu.recorder.wait_for", "waitFor"),
        ] {
            let args = match state.point {
                Some((x, y)) => {
                    json!({ "kind": kind, "view": state.view, "x": x, "y": y, "dip": true })
                }
                None => json!({ "kind": kind, "view": state.view }),
            };
            items.push(self.run(key, enabled, crate::recorder::CHECK, args));
        }
        items.push(Item::Separator);
        // Playing the last recording waits until none is being recorded.
        items.push(self.run(
            "contextmenu.recorder.play",
            state.recorder.recording.is_none(),
            crate::recorder::PLAY,
            json!({}),
        ));
        items.push(self.run(
            "contextmenu.recorder.panel",
            true,
            crate::recorder::PANEL,
            json!({}),
        ));
        self.submenu(Label::Key(KEY_MENU), items)
    }

    /// The Run flow submenu: each flow in every view, all of them, the Flows panel. Before the
    /// first listing one entry runs the project's flow - its only one, or the shell asks.
    pub(super) fn flows(&mut self) -> Item {
        let state = self.state;
        let mut items = Vec::new();
        match &state.recorder.flows {
            None => {
                items.push(self.run("contextmenu.flows.run", true, crate::flows::RUN, json!({})))
            }
            Some(flows) if flows.is_empty() => {
                items.push(self.run(
                    "contextmenu.flows.none",
                    false,
                    crate::flows::RUN,
                    json!({}),
                ));
            }
            Some(flows) => {
                for name in flows {
                    items.push(self.labelled(
                        Label::Text(name.clone()),
                        true,
                        Mark::None,
                        Action::Run {
                            command: crate::flows::RUN.to_owned(),
                            args: json!({ "name": name }),
                        },
                    ));
                }
                items.push(Item::Separator);
                items.push(self.run(
                    "contextmenu.flows.run_all",
                    true,
                    crate::flows::RUN_ALL,
                    json!({}),
                ));
            }
        }
        items.push(self.run(
            "contextmenu.flows.panel",
            true,
            crate::flows::PANEL,
            json!({}),
        ));
        self.submenu(Label::Key(KEY_FLOWS), items)
    }
}

#[cfg(test)]
#[path = "recorder_tests.rs"]
mod tests;
