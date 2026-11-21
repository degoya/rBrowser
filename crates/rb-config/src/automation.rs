//! `automation`: Settings > Automation in the project config. `automation.recorder` is the
//! recorder (RB-050-01, `rb_sync::recorder`), `automation.recorderExport` its export
//! (RB-050-02, the `recorder-export` plugin); further automation sections join here.

use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};

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
    /// Which hovers are steps (RB-090-81, RB-090-87): `menu` every hover on a menu's entry -
    /// one whose hover opened a submenu, or one inside an open submenu - where the pointer
    /// rested `hoverDwellMs`, with or without a click after it; `all` also a hover the recorder
    /// guesses revealed what the next click hits; `off` none.
    #[serde(deserialize_with = "record_hover_or_switch")]
    #[schemars(schema_with = "record_hover_schema")]
    pub record_hover: RecordHover,
    /// How long, in milliseconds, the pointer rests on a menu's entry before its hover is a step
    /// (RB-090-87); passing over it quicker is no step.
    #[schemars(range(max = 5_000))]
    pub hover_dwell_ms: u64,
    /// Which of the pointer's way a recording keeps (RB-090-87): `menu` the way to a menu's
    /// hover and to a click while a menu is open, `all` the way to every hover step and click,
    /// `off` none. Play and the export move the pointer along it before the step when the view
    /// has the size it was recorded in.
    pub record_pointer: RecordPointer,
    /// The form autofill's fills are steps (RB-090-78): each field it set as a `fill`, `check`,
    /// `uncheck` or `select` with the value set, a password as its placeholder.
    pub record_autofill: bool,
    /// How a recording plays back ("Play", `recorder.play`) and how its export waits: `fast`
    /// runs the steps back to back, each waiting only for its element; `recorded` keeps the
    /// pause before each step as it was recorded, each pause at most 10 seconds.
    pub playback_pace: PlaybackPace,
    /// While a recording plays, the Recorder panel marks the step each view runs, a failed step
    /// red with its reason, and scrolls the running step into sight (RB-090-84). Off, it marks
    /// the steps but leaves the list where it is.
    pub follow_playback: bool,
    /// While a recording runs, the Recorder panel lists its steps as they are recorded - the
    /// newest marked and scrolled into sight, hovers with their pause and pointer path
    /// (RB-090-89). Off, the steps show once the recording is saved.
    pub show_live_steps: bool,
    /// The least pause, in milliseconds, before each step but the first when a recording plays
    /// or exports at the pace `recorded` (RB-090-84), so a navigation's effect can be watched;
    /// 0 keeps the pauses as recorded.
    #[schemars(range(max = 10_000))]
    pub min_pause_ms: u64,
    /// The pause, in milliseconds, before a step added in the Recorder panel's editor
    /// (RB-092-01), waited at the pace `recorded`; each step's own can be changed there.
    #[schemars(range(max = 10_000))]
    pub inserted_step_pause_ms: u64,
    /// "Delete" in the Recorder panel asks in the panel before the recording's file is deleted
    /// (RB-090-91). Off, a click deletes it at once.
    pub confirm_delete: bool,
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
            record_hover: RecordHover::Menu,
            hover_dwell_ms: 250,
            record_pointer: RecordPointer::Menu,
            record_autofill: true,
            playback_pace: PlaybackPace::Fast,
            follow_playback: true,
            show_live_steps: true,
            min_pause_ms: 0,
            inserted_step_pause_ms: 500,
            confirm_delete: true,
            secret_fields: Vec::new(),
            dir: ".rbrowser/out/recordings".to_owned(),
        }
    }
}

/// `automation.recorder.recordHover` (RB-090-81).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum RecordHover {
    /// Every hover on a menu's entry where the pointer rested `hoverDwellMs`.
    #[default]
    Menu,
    /// Also a hover that seems to reveal what the next click hits.
    All,
    /// No hover.
    Off,
}

impl RecordHover {
    /// The value as the config writes it.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Menu => "menu",
            Self::All => "all",
            Self::Off => "off",
        }
    }
}

/// `recordHover` as written now, or the switch it was before RB-090-81. The switch's `true` kept
/// every hover the recorder guessed revealed the next click; it loads as `menu`, not `all` - the
/// recordings it made were full of the pointer's way (the owner's decision). `false` is `off`.
fn record_hover_or_switch<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<RecordHover, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Written {
        Switch(bool),
        Choice(RecordHover),
    }
    Ok(match Written::deserialize(deserializer)? {
        Written::Switch(true) => RecordHover::Menu,
        Written::Switch(false) => RecordHover::Off,
        Written::Choice(choice) => choice,
    })
}

/// The schema of `recordHover`: the three choices, and the earlier switch as a branch marked
/// `deprecated`, so a checked-in config that still has it validates in an editor; the settings
/// window offers only the choices.
fn record_hover_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
    let mut schema = RecordHover::json_schema(generator);
    if let Some(serde_json::Value::Array(branches)) = schema.get_mut("oneOf") {
        branches.push(serde_json::json!({
            "type": "boolean",
            "deprecated": true,
            "description": "The earlier form of this setting: `true` loads as `menu`, `false` as \
                            `off`."
        }));
    }
    schema
}

/// `automation.recorder.recordPointer` (RB-090-87).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum RecordPointer {
    /// The way to a menu's hover, and to a click while a menu is open or into one.
    #[default]
    Menu,
    /// The way to every hover step and click.
    All,
    /// No way.
    Off,
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
