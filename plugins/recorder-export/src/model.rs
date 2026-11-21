//! What the exporter reads: the recorder's recording (`schema/recording.v1.json`, RB-050-01,
//! mirrored here because the plugin builds for wasm32 without `rb-sync`) and the request the host
//! wraps it in - the locator preference, the base URL and one Playwright project per breakpoint.
//!
//! `render` takes either the request `{ "recording": {...}, "options": {...} }` or a bare
//! recording; a bare one gets the default options and its projects from the recording's own
//! breakpoints (size and device scale factor only).

use serde::Deserialize;

/// The recording format this exporter reads.
pub const RECORDING_VERSION: u32 = 1;

/// The request the host sends.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportRequest {
    /// The recording.
    pub recording: Recording,
    /// How to export it.
    #[serde(default)]
    pub options: ExportOptions,
}

/// How to export.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ExportOptions {
    /// Which selector candidate a locator prefers (`automation.recorderExport.locatorPreference`).
    pub locator_preference: LocatorPreference,
    /// The origin the spec's `page.goto` and `toHaveURL` are relative to: the recording's
    /// environment, else its start URL's origin.
    pub base_url: Option<String>,
    /// One Playwright project per breakpoint; the recording's breakpoints when empty.
    pub projects: Vec<Project>,
    /// `fast` or `recorded` (`automation.recorder.playbackPace`, RB-090-66).
    pub pace: crate::pace::Pace,
    /// The least pause before each step but the first at the `recorded` pace, in ms
    /// (`automation.recorder.minPauseMs`, RB-090-84).
    pub min_pause_ms: u64,
    /// The export's name, the files' stem; the recording's name when absent or not a file name.
    /// The text still names the recording (RB-090-69).
    pub name: Option<String>,
}

/// Which selector candidate a locator prefers; the first (best ranked) one when the kind is
/// missing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LocatorPreference {
    /// The recorder's rank order: test id, role and name, text, CSS.
    #[default]
    Auto,
    /// `getByTestId`.
    Testid,
    /// `getByRole`.
    Role,
    /// The text: the elements of its tag with that whole text, else `getByText`.
    Text,
    /// `locator(css)`.
    Css,
}

/// A breakpoint as a Playwright project.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Project {
    /// The breakpoint id, the project's name (`onlyIn` guards compare it).
    pub name: String,
    /// `chromium` (absent), `firefox` or `webkit`.
    pub engine: Option<String>,
    /// Viewport width in CSS px.
    pub width: u32,
    /// Viewport height in CSS px.
    pub height: u32,
    /// Device pixel ratio; absent or `0` keeps Playwright's.
    pub device_scale_factor: Option<f64>,
    /// Mobile device type (`isMobile`; Firefox has none).
    pub is_mobile: Option<bool>,
    /// Touch input.
    pub has_touch: Option<bool>,
    /// User agent override.
    pub user_agent: Option<String>,
    /// BCP 47 locale.
    pub locale: Option<String>,
    /// `light` or `dark`.
    pub color_scheme: Option<String>,
    /// `prefers-reduced-motion: reduce`.
    pub reduced_motion: Option<bool>,
}

/// One recording (`Recording` of `rb_sync::recorder`).
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Recording {
    /// The format version.
    pub version: u32,
    /// The name, also the file name.
    pub name: String,
    /// The leader's URL when the recording started.
    #[serde(default)]
    pub start_url: Option<String>,
    /// The breakpoints of the views that took part.
    #[serde(default)]
    pub breakpoints: Vec<RecordedBreakpoint>,
    /// The steps in order.
    #[serde(default)]
    pub steps: Vec<Step>,
}

/// A breakpoint a view showed while recording.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordedBreakpoint {
    /// The breakpoint id.
    pub id: String,
    /// Viewport width in CSS px.
    pub width: u32,
    /// Viewport height in CSS px.
    pub height: u32,
    /// Device pixel ratio.
    #[serde(default)]
    pub device_scale_factor: f64,
}

/// One step.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Step {
    /// Unique in the recording; the stable comment above the step names it.
    pub id: u32,
    /// A name the user gave it.
    #[serde(default)]
    pub name: Option<String>,
    /// What happens.
    pub action: Action,
    /// The breakpoint of the view it was recorded in.
    #[serde(default)]
    pub view: String,
    /// The breakpoints it ran in; empty means all.
    #[serde(default)]
    pub only_in: Vec<String>,
    /// When it happened, ms after the recording started: the `recorded` pace waits by it.
    #[serde(default)]
    pub at_ms: u64,
    /// The pointer's way to it (RB-090-87); absent in a recording made before.
    #[serde(default)]
    pub path: Vec<PathPoint>,
    /// Switched off in the editor (RB-092-01): [`parse_request`] leaves it out.
    #[serde(default)]
    pub disabled: bool,
    /// The pause before it at the `recorded` pace, set in the editor (RB-092-01), in place of
    /// the measured one.
    #[serde(default)]
    pub pause_ms: Option<u64>,
}

/// A point of a step's way: CSS px of the viewport, `t` ms after the way's first point.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub struct PathPoint {
    /// Horizontal.
    pub x: i32,
    /// Vertical.
    pub y: i32,
    /// Ms after the first point.
    pub t: u32,
}

/// What a step does.
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Action {
    /// Load a URL.
    Navigate {
        /// The URL.
        url: String,
    },
    /// Click an element.
    Click {
        /// The element.
        target: Locator,
        /// `left` (absent), `middle` or `right`.
        #[serde(default)]
        button: Option<String>,
        /// CDP modifier bits (1 Alt, 2 Ctrl, 4 Meta, 8 Shift).
        #[serde(default)]
        modifiers: u8,
    },
    /// Double-click an element.
    Dblclick {
        /// The element.
        target: Locator,
    },
    /// Set a field's value.
    Fill {
        /// The field.
        target: Locator,
        /// The value, or a `{{secret:<name>}}` placeholder when `secret`.
        value: String,
        /// The recorder made it from a secret field: the only value that reads the environment.
        #[serde(default)]
        secret: bool,
    },
    /// Press a key.
    Press {
        /// The focused element; absent for the page.
        #[serde(default)]
        target: Option<Locator>,
        /// `KeyboardEvent.key`.
        key: String,
        /// CDP modifier bits.
        #[serde(default)]
        modifiers: u8,
    },
    /// Choose options of a `<select>`.
    Select {
        /// The control.
        target: Locator,
        /// The value of the (first) chosen option.
        #[serde(default)]
        value: Option<String>,
        /// The chosen options' indexes.
        #[serde(default)]
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
    /// Move the pointer onto an element.
    Hover {
        /// The element.
        target: Locator,
    },
    /// Scroll the page or an element.
    Scroll {
        /// The scrolled element; absent for the page.
        #[serde(default)]
        target: Option<Locator>,
        /// `scrollLeft` in CSS px.
        x: f64,
        /// `scrollTop` in CSS px.
        y: f64,
    },
    /// Wait until the page is at a URL.
    WaitForUrl {
        /// The URL.
        url: String,
    },
    /// Wait until an element is there and visible (RB-090-60).
    WaitFor {
        /// The element.
        target: Locator,
    },
    /// Wait a fixed time at every pace (RB-092-01).
    Wait {
        /// How long, ms.
        ms: u64,
    },
    /// Check something.
    Assert {
        /// What.
        assertion: Assertion,
    },
}

/// What an `assert` step checks.
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
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

/// How an element is found: selector candidates in rank order, inside the open shadow hosts and
/// the iframes named.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Locator {
    /// The candidates, best first; the last is the CSS path.
    #[serde(default)]
    pub selectors: Vec<Selector>,
    /// The open shadow hosts from the document inward, by their best selector.
    #[serde(default)]
    pub hosts: Vec<Selector>,
    /// The iframes' CSS paths from the top document down.
    #[serde(default)]
    pub frame: Option<Vec<String>>,
}

/// One way to find an element.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Selector {
    /// `[data-testid="..."]`.
    Testid {
        /// The attribute's value.
        testid: String,
        /// The match to take among several.
        #[serde(default)]
        nth: u32,
    },
    /// ARIA role and accessible name.
    Role {
        /// The role.
        role: String,
        /// The accessible name.
        name: String,
        /// The match to take.
        #[serde(default)]
        nth: u32,
        /// The match's index among the visible ones (RB-090-81); absent in a recording made
        /// before, and for a hidden element.
        #[serde(default, rename = "visibleNth")]
        visible_nth: Option<u32>,
        /// The CSS selector of an ancestor inside which role and name match the element alone
        /// (RB-090-84); absent when the page showed the name once.
        #[serde(default)]
        within: Option<String>,
    },
    /// Tag and normalised text.
    Text {
        /// The tag name.
        tag: String,
        /// The text.
        text: String,
        /// The match to take.
        #[serde(default)]
        nth: u32,
        /// The match's index among the visible ones (RB-090-81).
        #[serde(default, rename = "visibleNth")]
        visible_nth: Option<u32>,
        /// The ancestor inside which the text matches the element alone (RB-090-84).
        #[serde(default)]
        within: Option<String>,
    },
    /// The CSS path.
    Css {
        /// The path.
        path: String,
    },
}

/// The request in `input`: the host's `{ recording, options }` or a bare recording.
///
/// # Errors
///
/// The text is no JSON, no recording, or a recording of another format version.
pub fn parse_request(input: &str) -> Result<ExportRequest, ParseError> {
    let value: serde_json::Value =
        serde_json::from_str(input).map_err(|error| ParseError::Invalid(error.to_string()))?;
    let wrapped = value.get("recording").is_some();
    let recording = if wrapped { &value["recording"] } else { &value };
    match recording.get("version").and_then(serde_json::Value::as_u64) {
        Some(version) if version == u64::from(RECORDING_VERSION) => {}
        Some(version) => return Err(ParseError::Version(version)),
        None => return Err(ParseError::Invalid("no `version`".to_owned())),
    }
    let mut request: ExportRequest = if wrapped {
        serde_json::from_value(value).map_err(|error| ParseError::Invalid(error.to_string()))?
    } else {
        let recording = serde_json::from_value(value)
            .map_err(|error| ParseError::Invalid(error.to_string()))?;
        ExportRequest {
            recording,
            options: ExportOptions::default(),
        }
    };
    // A step switched off in the editor is neither exported nor played (RB-092-01).
    request.recording.steps.retain(|step| !step.disabled);
    Ok(request)
}

/// Why the input is no request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParseError {
    /// Not a recording; the reason.
    Invalid(String),
    /// A recording of another format version.
    Version(u64),
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;
