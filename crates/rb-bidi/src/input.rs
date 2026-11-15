//! `input.performActions`: an [`InputEvent`] in CSS px of the viewport as WebDriver input
//! sources - a mouse, a keyboard, a wheel. Firefox dispatches them as trusted events (ADR 0001
//! Q4), and keeps the pressed keys and buttons between calls until `releaseActions`; a replayed
//! key therefore goes through [`HeldKeys`], which keeps the modifiers Firefox holds in step with
//! those the key event says are down.

use std::collections::BTreeSet;

use rb_engine::{Button, InputEvent, Phase};
use rb_sync::{MOD_ALT, MOD_CTRL, MOD_META, MOD_SHIFT};
use serde_json::{Value, json};

use crate::protocol::Command;

/// Replays `sources` in `context`.
pub fn perform_actions(context: &str, sources: Vec<Value>) -> Command {
    Command::call(
        "input.performActions",
        json!({ "context": context, "actions": sources }),
    )
}

/// Releases every key and button still held in `context`.
pub fn release_actions(context: &str) -> Command {
    Command::call("input.releaseActions", json!({ "context": context }))
}

/// The input sources of `input`; the reason when it cannot be replayed (a dead key).
pub fn actions_for(input: &InputEvent) -> Result<Vec<Value>, String> {
    match input {
        InputEvent::Mouse {
            phase,
            x,
            y,
            button,
            modifiers,
            ..
        } => Ok(mouse(*phase, *x, *y, *button, *modifiers)),
        InputEvent::Wheel {
            x,
            y,
            delta_x,
            delta_y,
        } => Ok(vec![json!({
            "type": "wheel",
            "id": "wheel",
            "actions": [{
                "type": "scroll",
                "x": px(*x),
                "y": px(*y),
                "deltaX": px(*delta_x),
                "deltaY": px(*delta_y),
                "origin": "viewport",
            }],
        })]),
        InputEvent::Key {
            phase,
            key,
            code,
            text,
            ..
        } => {
            let value = key_value(key, code, text.as_deref())
                .ok_or_else(|| format!("the key {key} cannot be replayed"))?;
            let kind = if *phase == Phase::Up {
                "keyUp"
            } else {
                "keyDown"
            };
            Ok(vec![keyboard(vec![
                json!({ "type": kind, "value": value }),
            ])])
        }
        InputEvent::Text { text } => {
            let actions = text
                .chars()
                .flat_map(|c| {
                    let c = c.to_string();
                    [
                        json!({ "type": "keyDown", "value": c }),
                        json!({ "type": "keyUp", "value": c }),
                    ]
                })
                .collect();
            Ok(vec![keyboard(actions)])
        }
    }
}

fn mouse(phase: Phase, x: f64, y: f64, button: Button, modifiers: u8) -> Vec<Value> {
    let mut pointer =
        vec![json!({ "type": "pointerMove", "x": px(x), "y": px(y), "origin": "viewport" })];
    let number = match button {
        Button::Left => Some(0),
        Button::Middle => Some(1),
        Button::Right => Some(2),
        Button::None => None,
    };
    match (phase, number) {
        (Phase::Down, Some(number)) => {
            pointer.push(json!({ "type": "pointerDown", "button": number }));
        }
        (Phase::Up, Some(number)) => pointer.push(json!({ "type": "pointerUp", "button": number })),
        _ => {}
    }
    let pointer = json!({
        "type": "pointer",
        "id": "mouse",
        "parameters": { "pointerType": "mouse" },
        "actions": pointer,
    });
    let held = modifier_keys(modifiers);
    if held.is_empty() || phase == Phase::Move {
        return vec![pointer];
    }
    // Keys and pointer advance tick by tick: the keys go down before the press and come up
    // after the release.
    let keys = if phase == Phase::Down {
        held.iter()
            .map(|key| json!({ "type": "keyDown", "value": key }))
            .collect()
    } else {
        let mut keys = vec![json!({ "type": "pause" }); 2];
        keys.extend(
            held.iter()
                .map(|key| json!({ "type": "keyUp", "value": key })),
        );
        keys
    };
    vec![keyboard(keys), pointer]
}

fn keyboard(actions: Vec<Value>) -> Value {
    json!({ "type": "key", "id": "keyboard", "actions": actions })
}

/// The CDP modifier bits with their WebDriver keys, left and right: Ctrl, Shift, Alt, Meta.
const MODIFIERS: [(u8, &str, &str, &str); 4] = [
    (MOD_CTRL, "Control", "\u{E009}", "\u{E051}"),
    (MOD_SHIFT, "Shift", "\u{E008}", "\u{E050}"),
    (MOD_ALT, "Alt", "\u{E00A}", "\u{E052}"),
    (MOD_META, "Meta", "\u{E03D}", "\u{E053}"),
];

/// The WebDriver codes of the CDP modifier bits, Ctrl, Shift, Alt, Meta.
fn modifier_keys(modifiers: u8) -> Vec<&'static str> {
    MODIFIERS
        .into_iter()
        .filter(|(bit, ..)| modifiers & bit != 0)
        .map(|(_, _, left, _)| left)
        .collect()
}

/// The modifier keys one Firefox view holds after the replayed keys: a keyup that never came
/// (the leader lost the focus, the leader changed) would leave Shift or Ctrl down for every key
/// after it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HeldKeys {
    keys: BTreeSet<&'static str>,
}

impl HeldKeys {
    /// The keyboard source of the key `key` (`code`) going down or up with the CDP modifier bits
    /// `modifiers` of the event: first every other modifier pressed or released so Firefox holds
    /// what the event says, then the key itself. `None` for a key WebDriver cannot type.
    pub fn key(&mut self, key: &str, code: &str, down: bool, modifiers: u8) -> Option<Value> {
        let value = key_value(key, code, None)?;
        let own = MODIFIERS
            .iter()
            .find(|(_, name, ..)| *name == key)
            .map_or(0, |(bit, ..)| *bit);
        let mut actions = Vec::new();
        for (bit, _, left, right) in MODIFIERS {
            if bit == own {
                continue;
            }
            let holding: Vec<&'static str> = [left, right]
                .into_iter()
                .filter(|held| self.keys.contains(held))
                .collect();
            if modifiers & bit != 0 {
                if holding.is_empty() {
                    actions.push(json!({ "type": "keyDown", "value": left }));
                    self.keys.insert(left);
                }
            } else {
                for held in holding {
                    actions.push(json!({ "type": "keyUp", "value": held }));
                    self.keys.remove(held);
                }
            }
        }
        if let Some((_, _, left, right)) = MODIFIERS.iter().find(|(bit, ..)| *bit == own) {
            let (left, right) = (*left, *right);
            let this = if value == right { right } else { left };
            if down {
                self.keys.insert(this);
            } else {
                // The other side's key goes too: the modifier is up.
                for side in [left, right] {
                    if side != this && self.keys.remove(side) {
                        actions.push(json!({ "type": "keyUp", "value": side }));
                    }
                }
                self.keys.remove(this);
            }
        }
        let kind = if down { "keyDown" } else { "keyUp" };
        actions.push(json!({ "type": kind, "value": value }));
        Some(keyboard(actions))
    }

    /// A replayed click with `modifiers` pressed and released those modifiers itself.
    pub fn clicked(&mut self, modifiers: u8) {
        for key in modifier_keys(modifiers) {
            self.keys.remove(key);
        }
    }

    /// Everything was released (`input.releaseActions`).
    pub fn clear(&mut self) {
        self.keys.clear();
    }
}

/// What WebDriver types for `key`: one character as itself, a named key as its code.
fn key_value(key: &str, code: &str, text: Option<&str>) -> Option<String> {
    if key.chars().count() == 1 {
        return Some(key.to_owned());
    }
    let right = code.ends_with("Right");
    let named = match key {
        "Backspace" => '\u{E003}',
        "Tab" => '\u{E004}',
        "Enter" => '\u{E007}',
        "Shift" if right => '\u{E050}',
        "Shift" => '\u{E008}',
        "Control" if right => '\u{E051}',
        "Control" => '\u{E009}',
        "Alt" if right => '\u{E052}',
        "Alt" => '\u{E00A}',
        "Meta" if right => '\u{E053}',
        "Meta" => '\u{E03D}',
        "Pause" => '\u{E00B}',
        "Escape" => '\u{E00C}',
        "PageUp" => '\u{E00E}',
        "PageDown" => '\u{E00F}',
        "End" => '\u{E010}',
        "Home" => '\u{E011}',
        "ArrowLeft" => '\u{E012}',
        "ArrowUp" => '\u{E013}',
        "ArrowRight" => '\u{E014}',
        "ArrowDown" => '\u{E015}',
        "Insert" => '\u{E016}',
        "Delete" => '\u{E017}',
        function if function.starts_with('F') => {
            let number: u32 = function[1..]
                .parse()
                .ok()
                .filter(|n| (1..=12).contains(n))?;
            char::from_u32(0xE031 + number - 1)?
        }
        _ => {
            return text
                .filter(|text| text.chars().count() == 1)
                .map(str::to_owned);
        }
    };
    Some(named.to_string())
}

/// CSS px as WebDriver's integers.
fn px(value: f64) -> i64 {
    // Viewport coordinates and deltas are far inside i64; `as` saturates anyway.
    value.round() as i64
}

#[cfg(test)]
#[path = "input_tests.rs"]
mod tests;
