//! `automation`: Settings > Automation in the project config. `automation.recorder` is the
//! recorder (RB-050-01, `rb_sync::recorder`), `automation.recorderExport` its export
//! (RB-050-02, the `recorder-export` plugin); further automation sections join here.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// `automation`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct AutomationConfig {
    /// The recorder (RB-050-01).
    pub recorder: RecorderConfig,
    /// The recorder's export (RB-050-02).
    pub recorder_export: RecorderExportConfig,
}

/// `automation.recorder`: what the recorder keeps and where (RB-050-01).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct RecorderConfig {
    /// Scrolls are steps, consecutive scrolls of one container as one. On by default, so a
    /// recording played back scrolls as the person did.
    pub record_scroll: bool,
    /// A hover is kept as a step when it revealed what the next click hits (a menu trigger).
    pub record_hover: bool,
    /// The form autofill's fills are steps (RB-090-78): each field it set as a `fill`, `check`,
    /// `uncheck` or `select` with the value set, a password as its placeholder.
    pub record_autofill: bool,
    /// How a recording plays back ("Play", `recorder.play`) and how its export waits: `fast`
    /// runs the steps back to back, each waiting only for its element; `recorded` keeps the
    /// pause before each step as it was recorded, each pause at most 10 seconds.
    pub playback_pace: PlaybackPace,
    /// Further fields whose values are never recorded, besides password fields: a CSS path as
    /// the recorder shows it (`#otp`), `[data-testid="..."]` or a field's accessible name. An
    /// entry also names a URL query or fragment parameter whose value is recorded masked.
    pub secret_fields: Vec<String>,
    /// The recordings folder, relative to the project root; it must lie inside `.rbrowser/`
    /// and outside the folders workspace trust protects.
    pub dir: String,
}

impl Default for RecorderConfig {
    fn default() -> Self {
        Self {
            record_scroll: true,
            record_hover: true,
            record_autofill: true,
            playback_pace: PlaybackPace::Fast,
            secret_fields: Vec::new(),
            dir: ".rbrowser/out/recordings".to_owned(),
        }
    }
}

/// `automation.recorder.playbackPace` (RB-090-66).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum PlaybackPace {
    /// The steps back to back, each waiting only for its element.
    #[default]
    Fast,
    /// The recorded pause before each step, at most 10 seconds.
    Recorded,
}

impl PlaybackPace {
    /// The value as the config writes it.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Fast => "fast",
            Self::Recorded => "recorded",
        }
    }
}

/// `automation.recorderExport`: how a recording becomes a flow or a Playwright test
/// (RB-050-02).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct RecorderExportConfig {
    /// Which selector candidate a locator prefers when the element has it; the recorder's rank
    /// order (`auto`) otherwise.
    pub locator_preference: LocatorPreference,
    /// The exports folder, relative to the project root; it must lie inside `.rbrowser/` and
    /// outside the folders workspace trust protects. Each export is a new folder in it.
    pub dir: String,
    /// A spec export writes its `playwright.config.ts` (one project per breakpoint) beside it.
    pub include_config: bool,
}

impl Default for RecorderExportConfig {
    fn default() -> Self {
        Self {
            locator_preference: LocatorPreference::Auto,
            dir: ".rbrowser/out/exports".to_owned(),
            include_config: true,
        }
    }
}

/// `automation.recorderExport.locatorPreference`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum LocatorPreference {
    /// The recorder's rank order: test id, role and name, text, CSS path.
    #[default]
    Auto,
    /// `getByTestId`.
    Testid,
    /// `getByRole` with the accessible name.
    Role,
    /// `getByText`.
    Text,
    /// `locator(css)`.
    Css,
}

impl LocatorPreference {
    /// The value as the config writes it.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Testid => "testid",
            Self::Role => "role",
            Self::Text => "text",
            Self::Css => "css",
        }
    }
}

#[cfg(test)]
#[path = "automation_tests.rs"]
mod tests;
