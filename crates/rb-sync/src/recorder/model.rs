//! The recording model (RB-050-01): what the recorder keeps of a session and writes as
//! `.rbrowser/out/recordings/<name>.json`, versioned by [`RECORDING_VERSION`] and described by
//! `schema/recording.v1.json` (generated from these types). The exporters of RB-050-02 read it.
//!
//! Coordinates are CSS pixels of the page (ADR 0001, decision 3); an element is found through
//! its selector candidates in rank order (`data-testid`, role and name, text, CSS path), never
//! through coordinates. A value from a password field or a configured secret field is never in
//! the model: it is a placeholder [`secret_placeholder`] (`{{secret:<name>}}`) in a `fill`
//! marked `secret`.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::event::{ElementSelector, ElementTarget};

/// The format version; raised on every incompatible change.
pub const RECORDING_VERSION: u32 = 1;
/// The recording's schema, relative to the repository root.
pub const RECORDING_SCHEMA_FILE: &str = "schema/recording.v1.json";

/// The placeholder that stands for the secret `name`: `{{secret:<name>}}`.
pub fn secret_placeholder(name: &str) -> String {
    format!("{{{{secret:{name}}}}}")
}

fn is_zero(value: &u32) -> bool {
    *value == 0
}

fn is_zero_u8(value: &u8) -> bool {
    *value == 0
}

fn is_false(value: &bool) -> bool {
    !*value
}

/// One recording.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Recording {
    /// The schema reference for editors.
    #[serde(rename = "$schema", default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    /// The format version, 1.
    pub version: u32,
    /// The name, also the file name (`[a-z0-9_-]`, at most 64 characters).
    pub name: String,
    /// The leader's URL when the recording started.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_url: Option<String>,
    /// The environment in effect (`local`, `staging`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub environment: Option<String>,
    /// The breakpoints of the views that took part, with their emulation.
    #[serde(default)]
    pub breakpoints: Vec<RecordedBreakpoint>,
    /// When the recording started, ms since the epoch.
    pub started_at_ms: u64,
    /// The rBrowser version that recorded it.
    pub app_version: String,
    /// The steps in order.
    #[serde(default)]
    pub steps: Vec<Step>,
}

/// A breakpoint a view showed while recording.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecordedBreakpoint {
    /// The breakpoint id (`mobile`), or the size of a free-size view (`500x800`).
    pub id: String,
    /// Viewport width in CSS px.
    pub width: u32,
    /// Viewport height in CSS px.
    pub height: u32,
    /// Device pixel ratio.
    pub device_scale_factor: f64,
}

/// One step.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Step {
    /// Unique in the recording; stays when steps are reordered.
    pub id: u32,
    /// A name the user gave it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// What happens.
    pub action: Action,
    /// The breakpoint of the view it was recorded in.
    pub view: String,
    /// When, ms after the recording started.
    pub at_ms: u64,
    /// The breakpoints it ran in, when not all of the recording's; empty means all.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub only_in: Vec<String>,
}

/// What a step does.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum Action {
    /// Load a URL (typed into the address bar, back, forward, the start).
    Navigate {
        /// The URL.
        url: String,
    },
    /// Click an element.
    Click {
        /// The element.
        target: Locator,
        /// The button; `left` when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        button: Option<String>,
        /// CDP modifier bits (1 Alt, 2 Ctrl, 4 Meta, 8 Shift).
        #[serde(default, skip_serializing_if = "is_zero_u8")]
        modifiers: u8,
    },
    /// Double-click an element.
    Dblclick {
        /// The element.
        target: Locator,
    },
    /// Set a field's value (consecutive typing coalesced).
    Fill {
        /// The field.
        target: Locator,
        /// The value, or a placeholder (`{{secret:password}}`) for a secret field.
        value: String,
        /// The value is the placeholder of a secret field: set only by the recorder, and the
        /// only value an exporter turns into an environment variable - text that merely looks
        /// like a placeholder (typed, from a page, in a URL) stays text.
        #[serde(default, skip_serializing_if = "is_false")]
        secret: bool,
    },
    /// Press a key that is not text input (Enter, Escape, Tab, a shortcut).
    Press {
        /// The focused element; absent for the page.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target: Option<Locator>,
        /// `KeyboardEvent.key`.
        key: String,
        /// CDP modifier bits.
        #[serde(default, skip_serializing_if = "is_zero_u8")]
        modifiers: u8,
    },
    /// Choose options of a `<select>`.
    Select {
        /// The control.
        target: Locator,
        /// The value of the (first) chosen option.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        value: Option<String>,
        /// The chosen options' indexes.
        indexes: Vec<u32>,
    },
    /// Check a checkbox or radio.
    Check {
        /// The control.
        target: Locator,
    },
    /// Uncheck a checkbox.
    Uncheck {
        /// The control.
        target: Locator,
    },
    /// Move the pointer onto an element (kept only when a click into what it revealed
    /// follows).
    Hover {
        /// The element.
        target: Locator,
    },
    /// Scroll the page or an element (only with `automation.recorder.recordScroll`).
    Scroll {
        /// The scrolled element; absent for the page.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target: Option<Locator>,
        /// `scrollLeft` in CSS px.
        x: f64,
        /// `scrollTop` in CSS px.
        y: f64,
    },
    /// Wait until the page is at a URL (a navigation that followed a click or a key).
    WaitForUrl {
        /// The URL.
        url: String,
    },
    /// Check something.
    Assert {
        /// What.
        assertion: Assertion,
    },
}

/// What an `assert` step checks.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum Assertion {
    /// The element is visible.
    Visible {
        /// The element.
        target: Locator,
    },
    /// The element's text.
    Text {
        /// The element.
        target: Locator,
        /// The expected text.
        text: String,
    },
    /// A field's value.
    Value {
        /// The field.
        target: Locator,
        /// The expected value.
        value: String,
    },
    /// The page's URL.
    Url {
        /// The expected URL.
        url: String,
    },
}

/// How an element is found: selector candidates in rank order, inside the open shadow hosts
/// and the iframes named.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Locator {
    /// The candidates, best first; the last is always the CSS path.
    pub selectors: Vec<Selector>,
    /// The open shadow hosts from the document inward, by their best selector.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hosts: Vec<Selector>,
    /// The iframes' CSS paths from the top document down; absent for the top frame.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame: Option<Vec<String>>,
}

impl Locator {
    /// The locator of `target` in the frame `frame`.
    pub fn from_target(target: &ElementTarget, frame: Option<&[String]>) -> Self {
        Locator {
            selectors: candidates(&target.element),
            hosts: target
                .hosts
                .iter()
                .filter_map(|host| candidates(host).into_iter().next())
                .collect(),
            frame: frame
                .filter(|frame| !frame.is_empty())
                .map(<[String]>::to_vec),
        }
    }

    /// The CSS path, the fallback every locator has.
    pub fn css_path(&self) -> &str {
        self.selectors
            .iter()
            .rev()
            .find_map(|selector| match selector {
                Selector::Css { path } => Some(path.as_str()),
                _ => None,
            })
            .unwrap_or("")
    }
}

/// One way to find an element.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub enum Selector {
    /// `[data-testid="..."]`.
    Testid {
        /// The attribute's value.
        testid: String,
        /// The match to take among several (document order).
        #[serde(default, skip_serializing_if = "is_zero")]
        nth: u32,
    },
    /// ARIA role and accessible name.
    Role {
        /// The role.
        role: String,
        /// The accessible name.
        name: String,
        /// The match to take.
        #[serde(default, skip_serializing_if = "is_zero")]
        nth: u32,
    },
    /// Tag and normalised text.
    Text {
        /// The tag name.
        tag: String,
        /// The text.
        text: String,
        /// The match to take.
        #[serde(default, skip_serializing_if = "is_zero")]
        nth: u32,
    },
    /// The CSS path.
    Css {
        /// The path.
        path: String,
    },
}

/// The candidates of the agent's selector, best first, the CSS path last.
pub fn candidates(selector: &ElementSelector) -> Vec<Selector> {
    match selector {
        ElementSelector::Testid { testid, nth, path } => vec![
            Selector::Testid {
                testid: testid.clone(),
                nth: *nth,
            },
            Selector::Css { path: path.clone() },
        ],
        ElementSelector::Role {
            role,
            name,
            nth,
            path,
        } => vec![
            Selector::Role {
                role: role.clone(),
                name: name.clone(),
                nth: *nth,
            },
            Selector::Css { path: path.clone() },
        ],
        ElementSelector::Text {
            tag,
            text,
            nth,
            path,
        } => vec![
            Selector::Text {
                tag: tag.clone(),
                text: text.clone(),
                nth: *nth,
            },
            Selector::Css { path: path.clone() },
        ],
        ElementSelector::Css { path } => vec![Selector::Css { path: path.clone() }],
    }
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;
