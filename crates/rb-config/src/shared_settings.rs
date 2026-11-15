//! Sections that both the project config and the user settings may hold, the project winning per
//! key (`x-scope: ["project", "local", "user"]`): Checks > Findings (RB-060-01), Capture >
//! Screenshots (RB-050-03) and Capture > Cookie banners & animations (RB-080-12). The keys are
//! described once, in the project schema; [`crate::UserSettings`] reads the same section
//! without a schema of its own, so the settings registry knows each key once.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// `checks.parallel`, `checks.keepRuns`, `checks.rules`, `checks.ignore` (Checks > Findings,
/// RB-060-01), read by `rb_plugin_host::ChecksSettings`. A key not set is `None` or empty, so
/// [`FindingsSettings::over`] can let the project win per key.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct FindingsSettings {
    /// How many check plugins run at once, counted per view.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 1, max = 16), extend("default" = 4))]
    pub parallel: Option<u32>,
    /// How many check runs stay under `.rbrowser/out/checks/`; older ones are removed.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 1, max = 100), extend("default" = 10))]
    pub keep_runs: Option<u32>,
    /// Per rule id (`<plugin>.<rule>`): off, or the severity its findings get. The settings
    /// window offers the rules of the loaded check plugins by name (`x-options`, RB-080-72).
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    #[schemars(extend("x-options" = "check-rules"))]
    pub rules: BTreeMap<String, RuleSetting>,
    /// Findings to leave out: each entry's rule id, page URL and element selector globs (`*`
    /// any text) must all match.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub ignore: Vec<IgnoreEntry>,
}

impl FindingsSettings {
    /// These settings (the project's) over `user`: a key set here wins, the user's fills in;
    /// rule overrides merge per rule id.
    #[must_use]
    pub fn over(&self, user: &FindingsSettings) -> FindingsSettings {
        let mut rules = user.rules.clone();
        rules.extend(self.rules.iter().map(|(id, level)| (id.clone(), *level)));
        FindingsSettings {
            parallel: self.parallel.or(user.parallel),
            keep_runs: self.keep_runs.or(user.keep_runs),
            rules,
            ignore: if self.ignore.is_empty() {
                user.ignore.clone()
            } else {
                self.ignore.clone()
            },
        }
    }
}

/// One value of `checks.rules`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum RuleSetting {
    /// The rule does not run.
    Off,
    /// Its findings are infos.
    Info,
    /// Its findings are warnings.
    Warning,
    /// Its findings are errors.
    Error,
}

/// One entry of `checks.ignore`; a field left out matches anything.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct IgnoreEntry {
    /// A glob of the qualified rule id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule: Option<String>,
    /// A glob of the page URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// A glob of the element selector.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selector: Option<String>,
}

/// `capture`: Capture > Screenshots (RB-050-03), read by `rb_visual::CaptureSettings::resolve`
/// (the project's section over the user's, per key). A value that does not fit is refused there
/// per key, so it stays loose here; a project `dir` must stay inside the project, only the user
/// settings may name an absolute folder.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct CaptureConfig {
    /// The folder screenshots are written to; empty for `.rbrowser/out/screenshots` in the
    /// project. A project may only name a folder inside the project, the user settings also an
    /// absolute one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dir: Option<String>,
    /// The file path below the folder, without `.png`: `{date}` (date and time of the run),
    /// `{day}`, `{time}`, `{env}`, `{breakpoint}`, `{view}`, `{mode}`, `{width}`, `{height}`,
    /// `{dpr}`; `/` separates folders.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 1), extend("default" = "{date}/{breakpoint}"))]
    pub path_pattern: Option<String>,
    /// CSS pixels around an element screenshot, on every side.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(range(max = 512), extend("default" = 16))]
    pub element_padding: Option<u32>,
    /// Seconds before a screenshot from a shortcut, the toolbar or the palette (0, 5 or 10);
    /// `Ctrl+Shift+Space C D` cycles it for the session.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(extend("enum" = [0, 5, 10], "default" = 0))]
    pub default_delay: Option<u32>,
    /// The overview image shows label, size, engine and environment above each view.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(extend("default" = true))]
    pub overview_labels: Option<bool>,
    /// Hide cookie banners, consent dialogs and chat widgets before every capture.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(extend("default" = true))]
    pub hide_banners: Option<bool>,
    /// The project's own elements to hide before every capture, as CSS selectors, beside the
    /// built-in list (at most 200).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(length(max = 200), inner(length(min = 1, max = 1024)))]
    pub hide_selectors: Option<Vec<String>>,
    /// Hide the elements of rBrowser's built-in list of consent managers and chat widgets.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(extend("default" = true))]
    pub use_builtin_list: Option<bool>,
    /// Freeze CSS animations, transitions, the caret and videos, with
    /// `prefers-reduced-motion: reduce`, during every capture.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(extend("default" = true))]
    pub freeze_animations: Option<bool>,
    /// The wait after hiding and freezing, before the capture, in milliseconds (0 to 5000).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(range(max = 5000), extend("default" = 100))]
    pub settle_ms: Option<u32>,
    /// Capture > Video (RB-080-02).
    #[serde(skip_serializing_if = "is_default")]
    pub video: crate::VideoConfig,
}

/// Whether `value` is its type's default: a user section not set is not written.
pub(crate) fn is_default<T: Default + PartialEq>(value: &T) -> bool {
    *value == T::default()
}

#[cfg(test)]
#[path = "shared_settings_tests.rs"]
mod tests;
