//! The clean-up while recording (RB-050-01): which captured events become which steps.
//!
//! - Consecutive input into one field is one `fill` with the field's last value; a `change`
//!   that only confirms it adds nothing.
//! - A click that a `change` follows on the same checkbox, radio or select becomes `check`,
//!   `uncheck` or `select`; two clicks and the `dblclick` on one element are one `dblclick`.
//! - Keys that type text are in the `fill`; only the others (Enter, Escape, Tab, shortcuts) are
//!   `press` steps, and Backspace or Delete in a field never are. In a password field only a
//!   named key is a step, in clear ([`shown`]).
//! - Hover is a heuristic: of the hovers since the last step, a click keeps the latest one on
//!   an element with a test id or a role that is neither the clicked element nor one of its
//!   ancestors (by CSS path) - the menu trigger whose submenu it clicks into. Every other move
//!   is dropped, and with `recordHover` off all of them.
//! - Scroll only with `recordScroll`, consecutive scrolls of one container as one step.
//! - Pointer, key-up, focus, wheel, context menu and submit events are no steps.
//!
//! Each step keeps, beside it, the views it ran in and the events it came from, so a view
//! whose replay of one of them failed drops out of its `onlyIn`.

use std::collections::BTreeSet;

use rb_engine::ViewId;

use super::model::{Action, Locator, Selector, Step, secret_placeholder};
use super::secrets::{is_secret, secret_name};
use crate::event::{ElementTarget, KeyData, MOD_ALT, MOD_CTRL, MOD_META, MouseButton, SyncEvent};
use crate::forms::is_file_skip;

/// The most hovers kept while waiting for a click.
const MAX_HOVERS: usize = 8;
/// Keys that never are a step of their own.
const SILENT_KEYS: &[&str] = &[
    "Shift",
    "Control",
    "Alt",
    "AltGraph",
    "Meta",
    "CapsLock",
    "Dead",
    "Process",
    "Unidentified",
];

/// Where a step ran and what it came from.
#[derive(Clone, Debug, Default)]
pub(crate) struct Trail {
    /// The views it ran in.
    pub views: BTreeSet<ViewId>,
    /// The `(event type, CSS path)` of the events it came from.
    pub origins: Vec<(&'static str, String)>,
}

impl Trail {
    fn add_origin(&mut self, kind: &'static str, path: &str) {
        if !self.origins.iter().any(|(k, p)| *k == kind && p == path) {
            self.origins.push((kind, path.to_owned()));
        }
    }
}

/// A step and its trail.
#[derive(Clone, Debug)]
pub(crate) struct Entry {
    pub step: Step,
    pub trail: Trail,
}

/// What a captured event is recorded with.
pub(crate) struct Capture<'a> {
    /// The breakpoint of the view it came from.
    pub view: &'a str,
    /// The iframes' CSS paths, `None` for the top frame.
    pub frame: Option<&'a [String]>,
    /// The view and the views it was replayed into.
    pub views: BTreeSet<ViewId>,
    /// Ms after the start.
    pub at_ms: u64,
    /// `automation.recorder.recordHover`.
    pub record_hover: bool,
    /// `automation.recorder.recordScroll`.
    pub record_scroll: bool,
    /// `automation.recorder.secretFields`.
    pub secret_fields: &'a [String],
}

/// The steps while recording.
#[derive(Debug, Default)]
pub(crate) struct Builder {
    pub entries: Vec<Entry>,
    hovers: Vec<Entry>,
    next_id: u32,
    /// When the last step that can navigate (a click, Enter, a submit) happened.
    pub last_trigger_ms: Option<u64>,
}

impl Builder {
    /// Adds a step; drops the hovers waiting for a click.
    pub fn push(&mut self, action: Action, view: &str, at_ms: u64, trail: Trail) {
        self.hovers.clear();
        self.next_id += 1;
        self.entries.push(Entry {
            step: Step {
                id: self.next_id,
                name: None,
                action,
                view: view.to_owned(),
                at_ms,
                only_in: Vec::new(),
            },
            trail,
        });
    }

    /// Forgets the hovers waiting for a click.
    pub fn drop_hovers(&mut self) {
        self.hovers.clear();
    }

    /// Records the (already redacted) `event`.
    pub fn event(&mut self, capture: &Capture<'_>, event: &SyncEvent) {
        let kind = crate::switches::event_type(event);
        let locator = |target: &ElementTarget| Locator::from_target(target, capture.frame);
        let trail = |path: &str| {
            let mut trail = Trail {
                views: capture.views.clone(),
                origins: Vec::new(),
            };
            trail.add_origin(kind, path);
            trail
        };
        match event {
            SyncEvent::Click(mouse) => {
                let target = locator(&mouse.target);
                self.keep_hover(&target);
                self.last_trigger_ms = Some(capture.at_ms);
                let button = match mouse.button {
                    MouseButton::Left | MouseButton::None => None,
                    other => serde_json::to_value(other)
                        .ok()
                        .and_then(|value| value.as_str().map(str::to_owned)),
                };
                let path = target.css_path().to_owned();
                let action = Action::Click {
                    target,
                    button,
                    modifiers: mouse.modifiers,
                };
                self.push_kept_hovers(capture.view, capture.at_ms, action, trail(&path));
            }
            SyncEvent::Dblclick(mouse) => {
                let target = locator(&mouse.target);
                let path = target.css_path().to_owned();
                let mut merged = trail(&path);
                for _ in 0..2 {
                    let same = |action: &Action| matches!(action, Action::Click { target, .. } if target.css_path() == path);
                    if self.last_is(same)
                        && let Some(entry) = self.entries.pop()
                    {
                        merged.origins.extend(entry.trail.origins);
                        merged.views.retain(|view| entry.trail.views.contains(view));
                    }
                }
                self.push(
                    Action::Dblclick { target },
                    capture.view,
                    capture.at_ms,
                    merged,
                );
            }
            SyncEvent::Hover(data) if capture.record_hover => {
                let target = locator(&data.target);
                let path = target.css_path().to_owned();
                if self.hovers.len() == MAX_HOVERS {
                    self.hovers.remove(0);
                }
                self.hovers.push(Entry {
                    step: Step {
                        id: 0,
                        name: None,
                        action: Action::Hover { target },
                        view: capture.view.to_owned(),
                        at_ms: capture.at_ms,
                        only_in: Vec::new(),
                    },
                    trail: trail(&path),
                });
            }
            SyncEvent::Input(input) => {
                let target = locator(&input.target);
                let secret = is_secret(input.sensitive, &input.target, capture.secret_fields);
                let typed = if secret {
                    Some(secret_placeholder(&secret_name(&input.target)))
                } else {
                    input.value.clone()
                };
                self.fill(capture, target, typed, input.data.as_deref(), kind, secret);
            }
            SyncEvent::Change(change) if !is_file_skip(change) => {
                let target = locator(&change.target);
                let path = target.css_path().to_owned();
                let secret = is_secret(change.sensitive, &change.target, capture.secret_fields);
                let action = if let Some(checked) = change.checked {
                    if checked {
                        Action::Check { target }
                    } else {
                        Action::Uncheck { target }
                    }
                } else if let Some(indexes) = &change.selected {
                    Action::Select {
                        target,
                        value: if secret { None } else { change.value.clone() },
                        indexes: indexes.clone(),
                    }
                } else {
                    let value = if secret {
                        Some(secret_placeholder(&secret_name(&change.target)))
                    } else {
                        change.value.clone()
                    };
                    self.fill(capture, target, value, None, kind, secret);
                    return;
                };
                // The click that opened or toggled the control is part of this step.
                if self.last_is(|last| matches!(last, Action::Click { target, .. } if target.css_path() == path))
                    && let Some(last) = self.entries.last_mut()
                {
                    last.step.action = action;
                    last.trail.add_origin(kind, &path);
                    return;
                }
                self.push(action, capture.view, capture.at_ms, trail(&path));
            }
            SyncEvent::Keydown(key) => {
                if !is_press(key) {
                    return;
                }
                if key.key == "Enter" {
                    self.last_trigger_ms = Some(capture.at_ms);
                }
                let target = key.target.as_ref().map(locator);
                let path = target.as_ref().map_or("", Locator::css_path).to_owned();
                let action = Action::Press {
                    target,
                    key: key.key.clone(),
                    modifiers: key.modifiers,
                };
                self.push(action, capture.view, capture.at_ms, trail(&path));
            }
            SyncEvent::Scroll(scroll) if capture.record_scroll => {
                let target = scroll.container.as_ref().map(locator);
                let path = target.as_ref().map_or("", Locator::css_path).to_owned();
                if let Some(last) = self.entries.last_mut()
                    && let Action::Scroll {
                        target: last_target,
                        x,
                        y,
                    } = &mut last.step.action
                    && last_target.as_ref().map_or("", Locator::css_path) == path
                {
                    *x = scroll.x;
                    *y = scroll.y;
                    return;
                }
                let action = Action::Scroll {
                    target,
                    x: scroll.x,
                    y: scroll.y,
                };
                self.push(action, capture.view, capture.at_ms, trail(&path));
            }
            SyncEvent::Submit(_) => self.last_trigger_ms = Some(capture.at_ms),
            _ => {}
        }
    }

    /// A view's replay of the event `(kind, path)` failed: it drops out of the latest step that
    /// came from such an event and ran there.
    pub fn failed(&mut self, view: ViewId, kind: &'static str, path: &str) {
        let hit = self
            .hovers
            .iter_mut()
            .rev()
            .chain(self.entries.iter_mut().rev())
            .find(|entry| {
                entry.trail.views.contains(&view)
                    && entry
                        .trail
                        .origins
                        .iter()
                        .any(|(k, p)| *k == kind && p == path)
            });
        if let Some(entry) = hit {
            entry.trail.views.remove(&view);
        }
    }

    fn fill(
        &mut self,
        capture: &Capture<'_>,
        target: Locator,
        value: Option<String>,
        data: Option<&str>,
        kind: &'static str,
        secret: bool,
    ) {
        let path = target.css_path().to_owned();
        if let Some(last) = self.entries.last_mut()
            && let Action::Fill {
                target: last_target,
                value: last_value,
                secret: last_secret,
            } = &mut last.step.action
            && last_target.css_path() == path
        {
            match value {
                Some(value) => {
                    *last_value = value;
                    *last_secret = secret;
                }
                // A placeholder is never typed onto.
                None if *last_secret => {}
                None => last_value.push_str(data.unwrap_or("")),
            }
            last.trail.views.retain(|view| capture.views.contains(view));
            last.trail.add_origin(kind, &path);
            return;
        }
        let value = value.unwrap_or_else(|| data.unwrap_or("").to_owned());
        let mut trail = Trail {
            views: capture.views.clone(),
            origins: Vec::new(),
        };
        trail.add_origin(kind, &path);
        self.push(
            Action::Fill {
                target,
                value,
                secret,
            },
            capture.view,
            capture.at_ms,
            trail,
        );
    }

    fn last_is(&self, test: impl Fn(&Action) -> bool) -> bool {
        self.entries
            .last()
            .is_some_and(|entry| test(&entry.step.action))
    }

    /// Keeps, of the waiting hovers, the latest that could have revealed `click`.
    fn keep_hover(&mut self, click: &Locator) {
        let click_path = click.css_path();
        let kept = self.hovers.iter().rposition(|hover| {
            let Action::Hover { target } = &hover.step.action else {
                return false;
            };
            let path = target.css_path();
            let ancestor_or_self = click_path == path
                || click_path.starts_with(&format!("{path} >"))
                || click_path.starts_with(&format!("{path} "));
            let named = matches!(
                target.selectors.first(),
                Some(Selector::Testid { .. } | Selector::Role { .. })
            );
            named && !ancestor_or_self
        });
        self.hovers = kept
            .map(|index| vec![self.hovers.remove(index)])
            .unwrap_or_default();
    }

    fn push_kept_hovers(&mut self, view: &str, at_ms: u64, action: Action, trail: Trail) {
        for hover in std::mem::take(&mut self.hovers) {
            self.next_id += 1;
            let mut step = hover.step;
            step.id = self.next_id;
            self.entries.push(Entry {
                step,
                trail: hover.trail,
            });
        }
        self.push(action, view, at_ms, trail);
    }
}

/// What the recorder looks at of the captured `event`. A key in a password field is decided on
/// before anything is masked: a named key (Enter, Tab, Escape, an arrow) goes on in clear, any
/// other (a character, Backspace, Delete) and its key-up are dropped - so neither a mask nor the
/// password's length reaches a step. Every other event as [`crate::redact`] shows it.
pub(crate) fn shown(event: &SyncEvent) -> Option<SyncEvent> {
    match event {
        SyncEvent::Keydown(key) if key.sensitive => is_named_key(&key.key).then(|| {
            SyncEvent::Keydown(KeyData {
                sensitive: false,
                ..key.clone()
            })
        }),
        SyncEvent::Keyup(key) if key.sensitive => None,
        other => Some(crate::redact(other)),
    }
}

/// Whether `key` is a key's name (`Enter`, `ArrowLeft`, `F5`), not text: ASCII letters and
/// digits, an upper-case letter first, more than one character; Backspace and Delete edit a
/// field's text and count as text here.
fn is_named_key(key: &str) -> bool {
    key.len() > 1
        && key.starts_with(|c: char| c.is_ascii_uppercase())
        && key.chars().all(|c| c.is_ascii_alphanumeric())
        && !matches!(key, "Backspace" | "Delete")
}

/// Whether `key` going down is a `press` step: a named key (Enter, Tab, Escape, an arrow) or a
/// shortcut (Ctrl or Meta with a character) - never text input (a character alone, with Shift,
/// Alt or AltGr, which reports as Ctrl+Alt), a modifier on its own, a repeat, or Backspace or
/// Delete in a field. So no character typed into a field - a password's included - is a step.
fn is_press(key: &KeyData) -> bool {
    // A masked key never is a step ([`shown`] lets a password field's named keys through).
    if key.sensitive || key.repeat || SILENT_KEYS.contains(&key.key.as_str()) {
        return false;
    }
    let ctrl_or_meta = key.modifiers & (MOD_CTRL | MOD_META) != 0;
    let altgr = key.modifiers & MOD_CTRL != 0 && key.modifiers & MOD_ALT != 0;
    if key.key.chars().count() == 1 {
        return ctrl_or_meta && !altgr;
    }
    !(key.target.is_some() && matches!(key.key.as_str(), "Backspace" | "Delete") && !ctrl_or_meta)
}

#[cfg(test)]
#[path = "cleanup_tests.rs"]
mod tests;
