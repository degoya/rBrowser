//! The events the sync agent captures, with the selectors and targets they carry: the Rust
//! mirror of `agent/src/protocol.ts` (protocol version [`PROTOCOL_VERSION`]). Both are held to
//! `agent/src/protocol-fixtures.json` (`tests/protocol_fixtures.rs` here, `protocol.test.ts`
//! there). Coordinates are CSS pixels of the view's emulated page.
//!
//! The values of password fields travel in [`InputData`] and [`ChangeData`] (the views need
//! them), marked `sensitive`; their `Debug` output masks every value, so a log line never shows
//! one (RB-020-05). Keys typed into a password field are [`KeyData`] marked `sensitive`, whose
//! `Debug` output masks `key`, `code` and `key_code`.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Raised on every incompatible change; messages of another version are refused.
pub const PROTOCOL_VERSION: u32 = 1;
/// The isolated world the agent runs in (`Page.addScriptToEvaluateOnNewDocument.worldName`).
pub const WORLD_NAME: &str = "rbrowser";
/// The binding the agent reports through, visible only in that world (`Runtime.addBinding`).
pub const BINDING_NAME: &str = "__rbEmit";
/// The agent's handle in its world: `__rbAgent.command(<AgentCommand>)`.
pub const AGENT_GLOBAL: &str = "__rbAgent";
/// The boot configuration put in front of the bundle.
pub const BOOT_GLOBAL: &str = "__rbAgentBoot";

/// CDP modifier bit: Alt.
pub const MOD_ALT: u8 = 1;
/// CDP modifier bit: Ctrl.
pub const MOD_CTRL: u8 = 2;
/// CDP modifier bit: Meta (Windows key, Command).
pub const MOD_META: u8 = 4;
/// CDP modifier bit: Shift.
pub const MOD_SHIFT: u8 = 8;

fn is_zero(value: &u32) -> bool {
    *value == 0
}

fn is_false(value: &bool) -> bool {
    !*value
}

/// How an element is found again in another view. `nth` picks among several matches
/// (document order, 0 is omitted); `path` is the CSS path, the fallback of every other kind. A
/// later role or text match of a shown element carries `visibleNth`, its index among the
/// visible matches (RB-090-81); a role or text shown more than once carries `within`, the
/// ancestor that makes it unique (RB-090-84).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ElementSelector {
    /// `[data-testid="..."]`.
    Testid {
        /// The attribute's value.
        testid: String,
        /// The match to take.
        #[serde(default, skip_serializing_if = "is_zero")]
        nth: u32,
        /// The CSS path.
        path: String,
    },
    /// ARIA role and accessible name.
    Role {
        /// The role (`button`).
        role: String,
        /// The accessible name.
        name: String,
        /// The match to take.
        #[serde(default, skip_serializing_if = "is_zero")]
        nth: u32,
        /// The match's index among the visible ones, when it is a later match and shown.
        #[serde(
            default,
            rename = "visibleNth",
            skip_serializing_if = "Option::is_none"
        )]
        visible_nth: Option<u32>,
        /// An ancestor's CSS selector inside which it names the element alone (RB-090-84).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        within: Option<String>,
        /// The CSS path.
        path: String,
    },
    /// Tag and normalised text.
    Text {
        /// The tag name.
        tag: String,
        /// The text, white space collapsed.
        text: String,
        /// The match to take.
        #[serde(default, skip_serializing_if = "is_zero")]
        nth: u32,
        /// The match's index among the visible ones, when it is a later match and shown.
        #[serde(
            default,
            rename = "visibleNth",
            skip_serializing_if = "Option::is_none"
        )]
        visible_nth: Option<u32>,
        /// An ancestor's CSS selector inside which it names the element alone (RB-090-84).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        within: Option<String>,
        /// The CSS path.
        path: String,
    },
    /// Only the CSS path.
    Css {
        /// The CSS path.
        path: String,
    },
}

/// A point in an element's border box, each axis 0 to 1.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Offset {
    /// Horizontal.
    pub x: f64,
    /// Vertical.
    pub y: f64,
}

/// A point in an element: its selector, the open shadow hosts above it and the offset.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ElementTarget {
    /// The open shadow hosts from the document inward.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hosts: Vec<ElementSelector>,
    /// The element.
    pub element: ElementSelector,
    /// Where in it.
    pub offset: Offset,
}

/// A mouse button, named as CDP names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MouseButton {
    /// No button.
    None,
    /// Primary.
    Left,
    /// Wheel button.
    Middle,
    /// Secondary.
    Right,
    /// Browser back.
    Back,
    /// Browser forward.
    Forward,
}

/// Pointer down or up.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PointerPhase {
    /// `pointerdown`.
    Down,
    /// `pointerup`.
    Up,
}

/// `pointerdown` / `pointerup`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PointerData {
    /// Down or up.
    pub phase: PointerPhase,
    /// The button that changed.
    pub button: MouseButton,
    /// The buttons held (`PointerEvent.buttons`).
    pub buttons: u32,
    /// `mouse`, `pen` or `touch`.
    pub pointer_type: String,
    /// CDP modifier bits.
    pub modifiers: u8,
    /// Where.
    pub target: ElementTarget,
}

/// `click`, `dblclick` and `contextmenu`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MouseData {
    /// The button.
    pub button: MouseButton,
    /// The click count (`UIEvent.detail`).
    pub detail: u32,
    /// CDP modifier bits.
    pub modifiers: u8,
    /// Where.
    pub target: ElementTarget,
    /// A click on the label of a checkbox or radio: the CSS path of that control (RB-090-93).
    /// The engine's own click on the control that follows is not sent; the control's `change`
    /// belongs to this click.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub control: Option<String>,
}

/// An event with only a target (`focus`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TargetData {
    /// The element.
    pub target: ElementTarget,
}

/// `hover`: the pointer moved onto another element.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HoverData {
    /// The element.
    pub target: ElementTarget,
    /// Not a move: a hover the recorder keeps as a step - the agent's hover on a menu entry,
    /// sent at a mouse press inside its hover-only submenu (`agent/src/menu-trigger.ts`,
    /// RB-090-76), or, while a recording runs, on a menu's entry where the pointer rested
    /// (`agent/src/record-hover.ts`, RB-090-87). Absent for a move.
    #[serde(default, skip_serializing_if = "is_false")]
    pub menu: bool,
}

/// A point of the pointer's way to an event while a recording keeps it (RB-090-87): CSS px of
/// the viewport, rounded, `t` ms after the way's first point.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PathPoint {
    /// Horizontal, CSS px of the viewport.
    pub x: i32,
    /// Vertical, CSS px of the viewport.
    pub y: i32,
    /// Ms after the way's first point.
    pub t: u32,
}

/// `wheel`, deltas summed over about a frame.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WheelData {
    /// Horizontal delta.
    pub delta_x: f64,
    /// Vertical delta.
    pub delta_y: f64,
    /// `WheelEvent.deltaMode`: 0 pixels, 1 lines, 2 pages.
    pub delta_mode: u32,
    /// CDP modifier bits.
    pub modifiers: u8,
    /// Under the pointer.
    pub target: ElementTarget,
}

/// `scroll` of the document or an element, coalesced per frame (RB-020-04); also the core's
/// `scroll` command, which brings the same content to the same edge in a follower.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScrollData {
    /// The scrolled element; `None` for the document.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub container: Option<ElementTarget>,
    /// The element at the container's top edge, and the point of it at the top-left corner of
    /// the visible area (`offset.y` the share of its height above the edge; `offset.x` may lie
    /// outside 0 to 1 when it does not reach the left edge). `None` when no element qualifies:
    /// the follower takes the same share of its scroll range.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor: Option<ElementTarget>,
    /// `scrollLeft`.
    pub x: f64,
    /// `scrollTop`.
    pub y: f64,
    /// The largest `scrollLeft`.
    pub max_x: f64,
    /// The largest `scrollTop`.
    pub max_y: f64,
}

/// `keydown` and `keyup`.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyData {
    /// `KeyboardEvent.key`.
    pub key: String,
    /// `KeyboardEvent.code`.
    pub code: String,
    /// `KeyboardEvent.keyCode`, CDP's `windowsVirtualKeyCode`.
    pub key_code: u32,
    /// `KeyboardEvent.location`.
    pub location: u32,
    /// Auto-repeat.
    pub repeat: bool,
    /// CDP modifier bits.
    pub modifiers: u8,
    /// A key in a password field: never logged, never shown to MCP or the recorder in clear.
    #[serde(default, skip_serializing_if = "is_false")]
    pub sensitive: bool,
    /// The focused element; `None` for the body.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<ElementTarget>,
}

/// `input`.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InputData {
    /// `InputEvent.inputType`.
    pub input_type: String,
    /// `InputEvent.data`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
    /// The field's value after the input.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// A password field: never logged, never shown to MCP or the recorder in clear.
    #[serde(default, skip_serializing_if = "is_false")]
    pub sensitive: bool,
    /// The field.
    pub target: ElementTarget,
}

/// `change` of a form control.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct ChangeData {
    /// The control's value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// A checkbox's or radio's state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checked: Option<bool>,
    /// The selected option indexes of a `<select>`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected: Option<Vec<u32>>,
    /// A password field.
    #[serde(default, skip_serializing_if = "is_false")]
    pub sensitive: bool,
    /// The control.
    pub target: ElementTarget,
}

/// The mask of a value in `Debug` output.
fn masked(value: Option<&String>) -> Option<&'static str> {
    value.map(|_| "<masked>")
}

impl fmt::Debug for KeyData {
    /// A sensitive key shows neither `key`, `code` nor `key_code`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut out = f.debug_struct("KeyData");
        if self.sensitive {
            out.field("key", &"<masked>")
                .field("code", &"<masked>")
                .field("key_code", &"<masked>");
        } else {
            out.field("key", &self.key)
                .field("code", &self.code)
                .field("key_code", &self.key_code);
        }
        out.field("location", &self.location)
            .field("repeat", &self.repeat)
            .field("modifiers", &self.modifiers)
            .field("sensitive", &self.sensitive)
            .field("target", &self.target)
            .finish()
    }
}

impl fmt::Debug for InputData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InputData")
            .field("input_type", &self.input_type)
            .field("data", &masked(self.data.as_ref()))
            .field("value", &masked(self.value.as_ref()))
            .field("sensitive", &self.sensitive)
            .field("target", &self.target)
            .finish()
    }
}

impl fmt::Debug for ChangeData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ChangeData")
            .field("value", &masked(self.value.as_ref()))
            .field("checked", &self.checked)
            .field("selected", &self.selected)
            .field("sensitive", &self.sensitive)
            .field("target", &self.target)
            .finish()
    }
}

/// `submit` of a form.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SubmitData {
    /// The form's method, lower case.
    pub method: String,
    /// The form's action, absolute.
    pub action: String,
    /// The form.
    pub target: ElementTarget,
}

/// One captured event.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum SyncEvent {
    /// Pointer down or up.
    Pointer(PointerData),
    /// A click.
    Click(MouseData),
    /// A double click.
    Dblclick(MouseData),
    /// A context menu (right click).
    Contextmenu(MouseData),
    /// The pointer moved onto another element (or rests on a menu entry, `menu`).
    Hover(HoverData),
    /// The wheel turned.
    Wheel(WheelData),
    /// The document or an element scrolled.
    Scroll(ScrollData),
    /// A key went down.
    Keydown(KeyData),
    /// A key went up.
    Keyup(KeyData),
    /// Text input.
    Input(InputData),
    /// A form control changed.
    Change(ChangeData),
    /// A form was submitted.
    Submit(SubmitData),
    /// An element got the focus.
    Focus(TargetData),
}

#[cfg(test)]
#[path = "event_tests.rs"]
mod tests;
