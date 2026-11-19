//! The clean-up while recording (RB-050-01): which captured events become which steps.
//!
//! - Consecutive input into one field is one `fill` with the field's last value; a `change`
//!   that only confirms it adds nothing.
//! - A click that a `change` follows on the same checkbox, radio or select becomes `check`,
//!   `uncheck` or `select`; two clicks and the `dblclick` on one element are one `dblclick`.
//!   A checkbox or radio is never filled: its `input` (no text, the value `on`) is no step, and
//!   a `fill` its `change` follows becomes the `check` or `uncheck` (RB-090-76).
//! - Keys that type text are in the `fill`; only the others (Enter, Escape, Tab, shortcuts) are
//!   `press` steps, and Backspace or Delete in a field never are. In a password field only a
//!   named key is a step, in clear ([`super::keys::shown`]).
//! - Hover is a heuristic: of the hovers since the last step, a click keeps the latest one on
//!   an element with a test id or a role that is neither the clicked element nor one of its
//!   ancestors (by CSS path) - the menu trigger whose submenu it clicks into; the agent reports
//!   that trigger's hover at the press (`agent/src/menu-trigger.ts`). A hover on the element of
//!   the click before (same CSS path, or same best selector after a navigation) is the pointer
//!   resting where it clicked, never a step - unless the agent marked it as the menu entry's
//!   (`menu`, RB-090-76). Every other move is dropped. `recordHover` (RB-090-81) decides which
//!   hovers wait for a click at all: `menu` (the default) only the agent's, `all` every one,
//!   `off` none.
//! - Scroll only with `recordScroll`, consecutive scrolls of one container as one step.
//! - Pointer, key-up, focus, wheel, context menu and submit events are no steps.
//!
//! Each step keeps, beside it, the views it ran in and the events it came from, so a view
//! whose replay of one of them failed drops out of its `onlyIn`.

use std::collections::BTreeSet;

use rb_engine::ViewId;

use super::keys::is_press;
use super::model::{Action, Locator, Selector, Step, secret_placeholder};
use super::record::HoverCapture;
use super::secrets::{is_secret, secret_name};
use crate::event::{ElementSelector, ElementTarget, InputData, MouseButton, SyncEvent};
use crate::forms::is_file_skip;

/// The most hovers kept while waiting for a click.
const MAX_HOVERS: usize = 8;

/// Where a step ran and what it came from.
#[derive(Clone, Debug, Default)]
pub(crate) struct Trail {
    /// The views it ran in.
    pub views: BTreeSet<ViewId>,
    /// The `(event type, CSS path)` of the events it came from.
    pub origins: Vec<(&'static str, String)>,
    /// A step of the form autofill (RB-090-78, [`super::autofill`]), not of captured events.
    pub autofill: bool,
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
    pub record_hover: HoverCapture,
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
    /// The element of the last click: a hover on it is no step...
    last_click: Option<Locator>,
    /// ...unless it is the agent's hover on a menu entry (`menu`), whose CSS path this is.
    menu_hover: Option<String>,
}

impl Builder {
    /// Adds a step; drops the hovers waiting for a click.
    pub fn push(&mut self, action: Action, view: &str, at_ms: u64, trail: Trail) {
        self.hovers.clear();
        self.menu_hover = None;
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

    /// Takes the last step out; its id is free again.
    pub fn pop_last(&mut self) {
        if let Some(entry) = self.entries.pop()
            && entry.step.id == self.next_id
        {
            self.next_id -= 1;
        }
    }

    /// Records the (already redacted) `event`.
    pub fn event(&mut self, capture: &Capture<'_>, event: &SyncEvent) {
        let kind = crate::switches::event_type(event);
        let locator = |target: &ElementTarget| Locator::from_target(target, capture.frame);
        let trail = |path: &str| {
            let mut trail = Trail {
                views: capture.views.clone(),
                ..Trail::default()
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
                self.last_click = Some(target.clone());
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
            SyncEvent::Hover(data) if capture.record_hover.keeps(data.menu) => {
                let target = locator(&data.target);
                let path = target.css_path().to_owned();
                if data.menu {
                    self.menu_hover = Some(path.clone());
                }
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
            SyncEvent::Input(input) if !is_toggle(input) => {
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
                let mut carried = None;
                if change.checked.is_some()
                    && self.last_is(|last| matches!(last, Action::Fill { target, .. } if target.css_path() == path))
                    && let Some(fill) = self.entries.pop()
                {
                    // The fill of a toggle's `input` (an unnamed checkbox): the check replaces it,
                    // in the views both ran in, and its id is free again.
                    if fill.step.id == self.next_id {
                        self.next_id -= 1;
                    }
                    carried = Some(fill.trail);
                }
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
                    merge(&mut last.trail, carried);
                    last.trail.add_origin(kind, &path);
                    return;
                }
                let mut trail = trail(&path);
                merge(&mut trail, carried);
                self.push(action, capture.view, capture.at_ms, trail);
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
            ..Trail::default()
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
        let clicked_before = self.last_click.as_ref();
        let kept = self.hovers.iter().rposition(|hover| {
            let Action::Hover { target } = &hover.step.action else {
                return false;
            };
            let menu = self.menu_hover.as_deref() == Some(target.css_path());
            if !menu && clicked_before.is_some_and(|before| same_element(before, target)) {
                return false;
            }
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

/// `trail` takes in the `merged` step's: its origins, and only the views both ran in.
fn merge(trail: &mut Trail, merged: Option<Trail>) {
    if let Some(merged) = merged {
        trail.origins.extend(merged.origins);
        trail.views.retain(|view| merged.views.contains(view));
    }
}

/// Whether `a` and `b` name one element: the same CSS path, or the same best selector that is
/// no CSS path (a link's role and name stay when a navigation moves it).
pub(crate) fn same_element(a: &Locator, b: &Locator) -> bool {
    a.css_path() == b.css_path()
        || matches!(
            (a.selectors.first(), b.selectors.first()),
            (Some(x), Some(y)) if x == y && !matches!(x, Selector::Css { .. })
        )
}

/// Whether `input` is a checkbox's, radio's or switch's: no input type, a toggle's role. Its
/// state arrives as the `change`.
fn is_toggle(input: &InputData) -> bool {
    input.input_type.is_empty()
        && matches!(
            &input.target.element,
            ElementSelector::Role { role, .. } if matches!(role.as_str(), "checkbox" | "radio" | "switch")
        )
}

#[cfg(test)]
#[path = "cleanup_tests.rs"]
mod tests;
