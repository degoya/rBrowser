//! What is synced: the kinds of sync (one per `sync.*` switch), the kind of each event, the
//! runtime switches the hub routes by, and the target an event aims at.

use serde::{Deserialize, Serialize};

use crate::event::{ElementSelector, ElementTarget, SyncEvent};
use crate::message::CaptureSwitches;

/// A kind of sync, one per `sync.*` switch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SyncKind {
    /// `sync.navigation`: the views follow a navigation (here: the leader's submit).
    Navigation,
    /// `sync.scroll`: scroll and wheel.
    Scroll,
    /// `sync.click`: pointer down/up, click, double click, context menu.
    Click,
    /// `sync.input`: input and submit.
    Input,
    /// `sync.keyboard`: key down/up.
    Keyboard,
    /// `sync.hover`.
    Hover,
    /// `sync.forms`: change of form controls (RB-020-05).
    Forms,
    /// `sync.focus`: focus (RB-020-05; off by default).
    Focus,
}

impl SyncKind {
    /// Every kind.
    pub const ALL: [SyncKind; 8] = [
        SyncKind::Navigation,
        SyncKind::Scroll,
        SyncKind::Click,
        SyncKind::Input,
        SyncKind::Keyboard,
        SyncKind::Hover,
        SyncKind::Forms,
        SyncKind::Focus,
    ];

    /// The kind whose switch governs `event` (`docs/sync-protocol.md`, Events).
    pub fn of(event: &SyncEvent) -> SyncKind {
        match event {
            SyncEvent::Pointer(_)
            | SyncEvent::Click(_)
            | SyncEvent::Dblclick(_)
            | SyncEvent::Contextmenu(_) => SyncKind::Click,
            SyncEvent::Hover(_) => SyncKind::Hover,
            SyncEvent::Wheel(_) | SyncEvent::Scroll(_) => SyncKind::Scroll,
            SyncEvent::Keydown(_) | SyncEvent::Keyup(_) => SyncKind::Keyboard,
            SyncEvent::Input(_) | SyncEvent::Submit(_) => SyncKind::Input,
            SyncEvent::Change(_) => SyncKind::Forms,
            SyncEvent::Focus(_) => SyncKind::Focus,
        }
    }
}

/// Whether a view that cannot follow `event` is desynced. Hover and scroll are not: an element
/// the narrow layout hides is hovered all the time, and scroll positions are RB-020-04's.
pub fn marks_desync(event: &SyncEvent) -> bool {
    !matches!(
        event,
        SyncEvent::Hover(_) | SyncEvent::Scroll(_) | SyncEvent::Wheel(_)
    )
}

/// The event's `type` as the protocol names it (`click`).
pub fn event_type(event: &SyncEvent) -> &'static str {
    match event {
        SyncEvent::Pointer(_) => "pointer",
        SyncEvent::Click(_) => "click",
        SyncEvent::Dblclick(_) => "dblclick",
        SyncEvent::Contextmenu(_) => "contextmenu",
        SyncEvent::Hover(_) => "hover",
        SyncEvent::Wheel(_) => "wheel",
        SyncEvent::Scroll(_) => "scroll",
        SyncEvent::Keydown(_) => "keydown",
        SyncEvent::Keyup(_) => "keyup",
        SyncEvent::Input(_) => "input",
        SyncEvent::Change(_) => "change",
        SyncEvent::Submit(_) => "submit",
        SyncEvent::Focus(_) => "focus",
    }
}

/// The element `event` aims at, if any.
pub fn target_of(event: &SyncEvent) -> Option<&ElementTarget> {
    match event {
        SyncEvent::Pointer(data) => Some(&data.target),
        SyncEvent::Click(data) | SyncEvent::Dblclick(data) | SyncEvent::Contextmenu(data) => {
            Some(&data.target)
        }
        SyncEvent::Hover(data) | SyncEvent::Focus(data) => Some(&data.target),
        SyncEvent::Wheel(data) => Some(&data.target),
        SyncEvent::Scroll(data) => data.container.as_ref(),
        SyncEvent::Keydown(data) | SyncEvent::Keyup(data) => data.target.as_ref(),
        SyncEvent::Input(data) => Some(&data.target),
        SyncEvent::Change(data) => Some(&data.target),
        SyncEvent::Submit(data) => Some(&data.target),
    }
}

/// The CSS path of the element `event` aims at: what the desync badge names. Never a value.
pub fn selector_of(event: &SyncEvent) -> Option<String> {
    target_of(event).map(|target| path_of(&target.element).to_owned())
}

fn path_of(selector: &ElementSelector) -> &str {
    match selector {
        ElementSelector::Testid { path, .. }
        | ElementSelector::Role { path, .. }
        | ElementSelector::Text { path, .. }
        | ElementSelector::Css { path } => path,
    }
}

/// The switches the hub routes by: the global switch (runtime only, starts on) and one per
/// kind, which start from the project's `sync.*` and can be changed at runtime (`sync_set`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SyncSwitches {
    /// Sync as a whole (the toolbar switch, `Leader S S`).
    pub enabled: bool,
    /// `sync.navigation`.
    pub navigation: bool,
    /// `sync.scroll`.
    pub scroll: bool,
    /// `sync.click`.
    pub click: bool,
    /// `sync.input`.
    pub input: bool,
    /// `sync.keyboard`.
    pub keyboard: bool,
    /// `sync.hover`.
    pub hover: bool,
    /// `sync.forms`: change of form controls (RB-020-05).
    pub forms: bool,
    /// `sync.focus`: focus (RB-020-05).
    pub focus: bool,
    /// `sync.leaderOnlySubmit`: form submits run only in the leader; the others follow by URL.
    pub leader_only_submit: bool,
}

impl Default for SyncSwitches {
    fn default() -> Self {
        Self {
            enabled: true,
            navigation: true,
            scroll: true,
            click: true,
            input: true,
            keyboard: true,
            hover: true,
            forms: true,
            focus: false,
            leader_only_submit: true,
        }
    }
}

impl SyncSwitches {
    /// Whether `kind` is synced (the global switch aside).
    pub fn kind(&self, kind: SyncKind) -> bool {
        match kind {
            SyncKind::Navigation => self.navigation,
            SyncKind::Scroll => self.scroll,
            SyncKind::Click => self.click,
            SyncKind::Input => self.input,
            SyncKind::Keyboard => self.keyboard,
            SyncKind::Hover => self.hover,
            SyncKind::Forms => self.forms,
            SyncKind::Focus => self.focus,
        }
    }

    /// Switches `kind` on or off.
    pub fn set_kind(&mut self, kind: SyncKind, on: bool) {
        let switch = match kind {
            SyncKind::Navigation => &mut self.navigation,
            SyncKind::Scroll => &mut self.scroll,
            SyncKind::Click => &mut self.click,
            SyncKind::Input => &mut self.input,
            SyncKind::Keyboard => &mut self.keyboard,
            SyncKind::Hover => &mut self.hover,
            SyncKind::Forms => &mut self.forms,
            SyncKind::Focus => &mut self.focus,
        };
        *switch = on;
    }

    /// What the active view's agent captures.
    pub fn capture(&self) -> CaptureSwitches {
        CaptureSwitches {
            click: self.click,
            hover: self.hover,
            scroll: self.scroll,
            keyboard: self.keyboard,
            input: self.input,
            forms: self.forms,
            focus: self.focus,
        }
    }
}

#[cfg(test)]
#[path = "switches_tests.rs"]
pub(crate) mod tests;
