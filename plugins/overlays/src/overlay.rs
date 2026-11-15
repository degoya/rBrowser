//! The registry of overlays: which ids exist, which options each takes and which command
//! toggles it. An overlay is one entry here and one module in `web/src/overlays/` (RB-060-03,
//! RB-070-07, RB-070-12); the state, the tools and the commands take it as it is.

use rb_plugin_api::{Failure, FailureKind};
use serde_json::{Map, Value};

/// The longest CSS selector the highlight takes, in characters.
pub const MAX_SELECTOR_CHARS: usize = 512;
/// The largest highlight index, either way: the page wraps it around its own match count.
pub const MAX_INDEX: i64 = 1_000_000;

/// The pseudo-classes `force-state` may force (CDP `CSS.forcePseudoState`).
pub const FORCE_STATES: &[&str] = &[
    "hover",
    "focus",
    "active",
    "focus-visible",
    "focus-within",
    "visited",
];

/// The type of an overlay option.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum OptionKind {
    /// `true` or `false`.
    Bool,
    /// A CSS selector, at most [`MAX_SELECTOR_CHARS`] characters, no control characters.
    Selector,
    /// A whole number within [`MAX_INDEX`] either way.
    Index,
    /// A whole number within the bounds.
    Int(i64, i64),
    /// A number within the bounds.
    Float(f64, f64),
    /// A list of [`FORCE_STATES`], none twice, at least one.
    States,
}

/// One overlay: its id and its options.
#[derive(Debug)]
pub struct OverlayKind {
    pub id: &'static str,
    pub options: &'static [(&'static str, OptionKind)],
    /// An option the overlay needs before it can be switched on.
    pub requires: Option<&'static str>,
}

/// Every overlay of the plugin, in menu order.
pub const OVERLAYS: &[OverlayKind] = &[
    OverlayKind {
        id: "overflow",
        options: &[("show100vw", OptionKind::Bool)],
        requires: None,
    },
    OverlayKind {
        id: "outlines",
        options: &[("containersOnly", OptionKind::Bool)],
        requires: None,
    },
    OverlayKind {
        id: "highlight",
        options: &[
            ("selector", OptionKind::Selector),
            ("index", OptionKind::Index),
        ],
        requires: Some("selector"),
    },
    OverlayKind {
        id: "stacking",
        options: &[],
        requires: None,
    },
    OverlayKind {
        id: "scrollbars",
        options: &[("tolerancePx", OptionKind::Int(0, 100))],
        requires: None,
    },
    OverlayKind {
        id: "text-spacing",
        options: &[],
        requires: None,
    },
    OverlayKind {
        id: "target-size",
        options: &[("sizePx", OptionKind::Int(8, 96))],
        requires: None,
    },
    OverlayKind {
        id: "small-text",
        options: &[("px", OptionKind::Int(6, 32))],
        requires: None,
    },
    OverlayKind {
        id: "focus-tracking",
        options: &[],
        requires: None,
    },
    OverlayKind {
        id: "force-state",
        options: &[
            ("selector", OptionKind::Selector),
            ("states", OptionKind::States),
        ],
        requires: Some("selector"),
    },
    OverlayKind {
        id: "content-chaos",
        options: &[("factor", OptionKind::Float(0.1, 10.0))],
        requires: None,
    },
    OverlayKind {
        id: "placeholdifier",
        options: &[],
        requires: None,
    },
    OverlayKind {
        id: "css-off",
        options: &[("includeInline", OptionKind::Bool)],
        requires: None,
    },
    OverlayKind {
        id: "js-off",
        options: &[],
        requires: None,
    },
    OverlayKind {
        id: "images-off",
        options: &[],
        requires: None,
    },
    OverlayKind {
        id: "tab-order",
        options: &[],
        requires: None,
    },
];

/// The commands that toggle one overlay: the manifest's command id and the overlay's id (the
/// same with `_` for `-`, as a command id takes no `-`). `off` switches every overlay off.
pub const COMMANDS: &[(&str, &str)] = &[
    ("overflow", "overflow"),
    ("outlines", "outlines"),
    ("highlight", "highlight"),
    ("stacking", "stacking"),
    ("scrollbars", "scrollbars"),
    ("text_spacing", "text-spacing"),
    ("target_size", "target-size"),
    ("small_text", "small-text"),
    ("focus_tracking", "focus-tracking"),
    ("force_state", "force-state"),
    ("content_chaos", "content-chaos"),
    ("placeholdifier", "placeholdifier"),
    ("css_off", "css-off"),
    ("js_off", "js-off"),
    ("images_off", "images-off"),
    ("tab_order", "tab-order"),
];

/// The overlay command `command` toggles.
#[must_use]
pub fn command_overlay(command: &str) -> Option<&'static str> {
    COMMANDS
        .iter()
        .find(|(id, _)| *id == command)
        .map(|(_, overlay)| *overlay)
}

/// The overlay `id`.
#[must_use]
pub fn kind(id: &str) -> Option<&'static OverlayKind> {
    OVERLAYS.iter().find(|kind| kind.id == id)
}

/// `plugin.overlays.errors.<what>` with the parameters given.
#[must_use]
pub fn refusal(what: &str, params: &[(&str, &str)]) -> Failure {
    let mut failure = Failure::coded(
        FailureKind::BadData,
        &format!("plugin.overlays.errors.{what}"),
    );
    for (key, value) in params {
        let value: String = value.chars().take(64).collect();
        failure = failure.with_param(key, value);
    }
    failure
}

/// `options` checked against `kind`: every option known and of its type. Answers the options
/// as given (a selector trimmed).
///
/// # Errors
///
/// `plugin.overlays.errors.bad_option` naming the option.
pub fn validate_options(
    kind: &OverlayKind,
    options: &Map<String, Value>,
) -> Result<Map<String, Value>, Failure> {
    let mut checked = Map::new();
    for (name, value) in options {
        let bad = || refusal("bad_option", &[("overlay", kind.id), ("option", name)]);
        let option = kind
            .options
            .iter()
            .find(|(option, _)| option == name)
            .map(|(_, option)| *option)
            .ok_or_else(bad)?;
        let value = match option {
            OptionKind::Bool => Value::Bool(value.as_bool().ok_or_else(bad)?),
            OptionKind::Index => {
                let index = value.as_i64().ok_or_else(bad)?;
                if index.abs() > MAX_INDEX {
                    return Err(bad());
                }
                Value::from(index)
            }
            OptionKind::Selector => {
                Value::String(selector(value.as_str().ok_or_else(bad)?).ok_or_else(bad)?)
            }
            OptionKind::Int(low, high) => {
                let number = value.as_i64().filter(|n| (low..=high).contains(n));
                Value::from(number.ok_or_else(bad)?)
            }
            OptionKind::Float(low, high) => {
                let number = value.as_f64().filter(|n| (low..=high).contains(n));
                Value::from(number.ok_or_else(bad)?)
            }
            OptionKind::States => states(value).ok_or_else(bad)?,
        };
        checked.insert(name.clone(), value);
    }
    Ok(checked)
}

/// `value` as a list of forced states: known pseudo-classes, none twice, at least one.
fn states(value: &Value) -> Option<Value> {
    let list = value.as_array()?;
    let mut seen: Vec<&str> = Vec::new();
    for state in list {
        let state = state
            .as_str()
            .filter(|state| FORCE_STATES.contains(state))?;
        if seen.contains(&state) {
            return None;
        }
        seen.push(state);
    }
    (!seen.is_empty()).then(|| Value::from(seen))
}

/// A selector as the highlight keeps it: trimmed, within [`MAX_SELECTOR_CHARS`], free of control
/// characters; none for one that is not.
#[must_use]
pub fn selector(text: &str) -> Option<String> {
    let text = text.trim();
    if text.chars().count() > MAX_SELECTOR_CHARS || text.chars().any(char::is_control) {
        return None;
    }
    Some(text.to_owned())
}

#[cfg(test)]
#[path = "overlay_tests.rs"]
mod tests;
