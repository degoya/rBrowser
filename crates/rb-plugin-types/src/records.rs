//! The records of `rbrowser:plugin@0.1.0`, as the host and a plugin's native logic hold them.
//! Each mirrors one WIT record field for field; the host converts between them and the
//! generated bindings, a plugin's guest half between them and `wit-bindgen`'s.

use serde::{Deserialize, Serialize};

/// A view's id, as `rb-engine`'s `ViewId` holds it.
pub type ViewId = u32;

/// What a plugin with `views` learns about one view (`interface views`).
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct ViewInfo {
    pub id: ViewId,
    pub label: String,
    /// The breakpoint id from the project config.
    pub breakpoint: String,
    /// `chromium`, `firefox` or `webkit`.
    pub engine: String,
    pub width: u32,
    pub height: u32,
    pub dpr: f32,
    pub url: String,
    pub title: String,
}

/// How serious a finding is; ordered from `Info` to `Error`.
#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    #[default]
    Warning,
    Error,
}

impl Severity {
    /// Every severity, from the mildest.
    pub const ALL: [Self; 3] = [Self::Info, Self::Warning, Self::Error];

    /// The name in JSON and in settings (`error`).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }

    /// The severity named `name`; `None` for anything else.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|severity| severity.as_str() == name)
    }
}

/// One finding as a check plugin answers or reports it (`types.finding`). The host turns it into
/// a [`crate::MergedFinding`]: rule id qualified with the plugin's id, stable id, views merged
/// (RB-060-01).
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Finding {
    /// The rule's id within the plugin (`images.alt`); one `describe` lists.
    pub rule: String,
    pub severity: Severity,
    /// An i18n key of the plugin's locales (`plugin.<id>.*`).
    pub message: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub params: Vec<(String, String)>,
    /// The view it was found in; the run's view when `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view: Option<ViewId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// A CSS selector that finds the element (`img.hero`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selector: Option<String>,
    /// The element's full path from the root (`html > body > main > img:nth-of-type(2)`): what
    /// the stable id is made of, so the same element is the same finding in every view and run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Free details as JSON text, shown and exported as they are.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
}

/// One HTTP request of `http.fetch`.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct HttpRequest {
    pub method: String,
    pub url: String,
    #[serde(default)]
    pub headers: Vec<(String, String)>,
    #[serde(default)]
    pub body: Vec<u8>,
}

/// The answer of `http.fetch`; a redirect is answered, never followed.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct HttpResponse {
    pub status: u16,
    #[serde(default)]
    pub headers: Vec<(String, String)>,
    #[serde(default)]
    pub body: Vec<u8>,
}

/// A config change an analyzer proposes (`analyzer.proposal`); a person applies it.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Proposal {
    /// JSON Pointer into `.rbrowser/config.json` (`/breakpoints/2/width`).
    pub pointer: String,
    /// The proposed value, as JSON text.
    pub value: String,
    /// An i18n key of the plugin's locales: why.
    pub reason: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub params: Vec<(String, String)>,
}

/// The key a plugin passes to `settings.get` for the person's levels of its own rules
/// (`checks.rules.<plugin>.<rule>`, RB-091-06): a JSON object of rule id, without the plugin's
/// prefix, to `off`, `info`, `warning` or `error` - the project's keys over the user's, as the
/// findings run applies them. Read only; another plugin's rules never show.
pub const RULE_LEVELS_SETTING: &str = "checks.rules";

/// One rule a check plugin offers (`check.rule-info`).
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuleInfo {
    /// Stable id, unique within the plugin (`images.alt`); the findings name it as `rule`.
    pub id: String,
    /// An i18n key of the plugin's locales.
    pub title: String,
    /// The severity its findings usually have; `checks.rules.<plugin>.<id>` overrides it.
    pub severity: Severity,
    /// The id it shares with other plugins' rules that find the same (`img-alt-missing`): their
    /// findings at the same element merge into one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shared_id: Option<String>,
    /// A page that explains the rule (`https://...`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub help_url: Option<String>,
}

/// What one `check.run` looks at (`check.check-context`).
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct CheckContext {
    /// The view to check; `None` leaves the views to the plugin (one without views).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view: Option<ViewId>,
    /// The URL the view shows, or the one asked for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// The ids of the rules to run: those of `describe` the person has not switched off.
    #[serde(default)]
    pub rules: Vec<String>,
    /// Arguments as JSON text (`{}` when none).
    pub args: String,
}

/// One format an export plugin writes (`exporter.export-format`).
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ExportFormat {
    pub id: String,
    /// An i18n key of the plugin's locales.
    pub title: String,
    /// The file extension, without the dot (`ts`).
    pub extension: String,
}

/// What an export plugin rendered (`exporter.export-output`).
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ExportOutput {
    /// A file name without folders (`flow.spec.ts`).
    pub file_name: String,
    pub text: String,
}
