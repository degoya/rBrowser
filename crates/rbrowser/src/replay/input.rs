//! Points and CDP `Input.*` commands (RB-020-03).
//!
//! The coordinates on the way:
//!
//! 1. The agent resolves the target to a point of its layout viewport, CSS px of the emulated
//!    page (`getBoundingClientRect`; a page scroll is already in it).
//! 2. [`input_point`] moves it into the visual viewport: a pinch-zoomed mobile view shows part
//!    of the layout viewport, larger (ADR 0001 addendum).
//! 3. The host multiplies every `Input.*` coordinate by the view's emulation scale on the way
//!    out (`ViewHost::cdp_send`, `fit::scale_input`): CDP input is widget DIP once the scale is
//!    not 1. The device pixel ratio plays no part - it scales device pixels, not DIP.

use rb_sync::MouseButton;
use serde_json::{Value, json};

/// `Input.dispatchMouseEvent`.
pub const MOUSE: &str = "Input.dispatchMouseEvent";
/// `Input.dispatchTouchEvent`.
pub const TOUCH: &str = "Input.dispatchTouchEvent";
/// `Input.dispatchKeyEvent`.
pub const KEY: &str = "Input.dispatchKeyEvent";
/// `Input.insertText`.
pub const INSERT_TEXT: &str = "Input.insertText";

/// The largest click count sent: a triple click selects a paragraph, more adds nothing.
const MAX_CLICK_COUNT: u32 = 3;
/// The largest visual viewport scale taken from a page (Chromium's own maximum is 5).
const MAX_VIEWPORT_SCALE: f64 = 20.0;

/// `window.visualViewport`: the part of the layout viewport the view shows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VisualViewport {
    /// `offsetLeft`, CSS px.
    pub offset_left: f64,
    /// `offsetTop`, CSS px.
    pub offset_top: f64,
    /// `scale`, the pinch zoom.
    pub scale: f64,
}

impl VisualViewport {
    /// No pinch zoom.
    pub const IDENTITY: Self = Self {
        offset_left: 0.0,
        offset_top: 0.0,
        scale: 1.0,
    };

    /// Whether the values are usable (the page may have replaced `visualViewport`).
    fn is_sane(&self) -> bool {
        self.offset_left.is_finite()
            && self.offset_top.is_finite()
            && self.scale.is_finite()
            && self.scale > 0.0
            && self.scale <= MAX_VIEWPORT_SCALE
    }
}

/// The point CDP input takes (before the host's emulation scale) for the agent's point
/// (`x`, `y`) of the layout viewport.
pub fn input_point(x: f64, y: f64, viewport: Option<VisualViewport>) -> (f64, f64) {
    let viewport = viewport
        .filter(VisualViewport::is_sane)
        .unwrap_or(VisualViewport::IDENTITY);
    (
        (x - viewport.offset_left) * viewport.scale,
        (y - viewport.offset_top) * viewport.scale,
    )
}

/// One CDP command for a view. Its `Debug` output names the method only: the parameters of
/// `Input.insertText` are what the user typed, a password field's value included.
#[derive(Clone, PartialEq)]
pub struct Call {
    /// The method.
    pub method: &'static str,
    /// Its parameters (CSS px; the host scales them).
    pub params: Value,
}

impl std::fmt::Debug for Call {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Call")
            .field("method", &self.method)
            .finish_non_exhaustive()
    }
}

/// CDP's name of `button`.
fn button_name(button: MouseButton) -> &'static str {
    match button {
        MouseButton::None => "none",
        MouseButton::Left => "left",
        MouseButton::Middle => "middle",
        MouseButton::Right => "right",
        MouseButton::Back => "back",
        MouseButton::Forward => "forward",
    }
}

/// The bit of `button` in `MouseEvent.buttons` (CDP's `buttons`).
pub fn button_mask(button: MouseButton) -> u32 {
    match button {
        MouseButton::None => 0,
        MouseButton::Left => 1,
        MouseButton::Right => 2,
        MouseButton::Middle => 4,
        MouseButton::Back => 8,
        MouseButton::Forward => 16,
    }
}

fn mouse(
    kind: &str,
    x: f64,
    y: f64,
    button: MouseButton,
    buttons: u32,
    count: u32,
    modifiers: u8,
) -> Call {
    let mut params = json!({
        "type": kind,
        "x": x,
        "y": y,
        "button": button_name(button),
        "buttons": buttons,
        "modifiers": modifiers,
        "pointerType": "mouse",
    });
    if count > 0 {
        params["clickCount"] = json!(count);
    }
    Call {
        method: MOUSE,
        params,
    }
}

/// A click at (`x`, `y`): the pointer moves there, `button` goes down and up with the click
/// count `count` (the second click of a double click counts 2; the page gets `dblclick` from
/// the engine itself).
pub fn click_calls(x: f64, y: f64, button: MouseButton, count: u32, modifiers: u8) -> Vec<Call> {
    let count = count.clamp(1, MAX_CLICK_COUNT);
    vec![
        mouse("mouseMoved", x, y, MouseButton::None, 0, 0, modifiers),
        mouse(
            "mousePressed",
            x,
            y,
            button,
            button_mask(button),
            count,
            modifiers,
        ),
        mouse("mouseReleased", x, y, button, 0, count, modifiers),
    ]
}

/// The pointer moves onto (`x`, `y`): `:hover` styles and hover menus open.
pub fn hover_calls(x: f64, y: f64) -> Vec<Call> {
    vec![mouse("mouseMoved", x, y, MouseButton::None, 0, 0, 0)]
}

/// A tap at (`x`, `y`) in a touch view: the page sees `pointerType == "touch"`.
pub fn tap_calls(x: f64, y: f64, modifiers: u8) -> Vec<Call> {
    let touch = |kind: &str, points: Value| Call {
        method: TOUCH,
        params: json!({ "type": kind, "touchPoints": points, "modifiers": modifiers }),
    };
    vec![
        touch("touchStart", json!([{ "x": x, "y": y }])),
        touch("touchEnd", json!([])),
    ]
}

/// `Input.insertText`: text into the focused element, as an IME or an emoji keyboard would.
pub fn insert_text_call(text: &str) -> Call {
    Call {
        method: INSERT_TEXT,
        params: json!({ "text": text }),
    }
}

/// `Input.dispatchKeyEvent` with `params` ([`super::keys::key_event`]).
pub fn key_call(params: Value) -> Call {
    Call {
        method: KEY,
        params,
    }
}

#[cfg(test)]
#[path = "input_tests.rs"]
mod tests;
