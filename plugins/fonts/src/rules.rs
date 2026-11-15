//! The font rules and the findings one view gives (`check.describe`, `check.run`): privacy
//! (`external-host`), performance (`font-display-missing`, `format-not-woff2`, `preload-missing`,
//! `face-unused`, `too-many-weights`, `large-file`), readability (`text-too-small`,
//! `input-too-small`, `line-height-tight`), rendered vs declared (`rendered-fallback`,
//! `glyph-fallback`, Chromium) and the breakpoints (`fluid-not-scaled`, across the views).
//!
//! The host qualifies each id with the plugin's (`fonts.external-host`) and holds the findings
//! to `checks.rules` and `checks.ignore`. A finding about a face, a family, a host or a file has no
//! element: its `path` names it (`@font-face Inter 600 normal`), so it is one finding in every
//! view and every run; `selector` stays empty.

use rb_plugin_api::{Finding, RuleInfo, Severity, ViewId};

use crate::key;
use crate::page::Located;

/// One rule of `describe`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rule {
    pub id: &'static str,
    pub severity: Severity,
}

pub const EXTERNAL_HOST: &str = "external-host";
pub const FONT_DISPLAY_MISSING: &str = "font-display-missing";
pub const FORMAT_NOT_WOFF2: &str = "format-not-woff2";
pub const PRELOAD_MISSING: &str = "preload-missing";
pub const FACE_UNUSED: &str = "face-unused";
pub const TOO_MANY_WEIGHTS: &str = "too-many-weights";
pub const LARGE_FILE: &str = "large-file";
pub const TEXT_TOO_SMALL: &str = "text-too-small";
pub const INPUT_TOO_SMALL: &str = "input-too-small";
pub const LINE_HEIGHT_TIGHT: &str = "line-height-tight";
pub const RENDERED_FALLBACK: &str = "rendered-fallback";
pub const GLYPH_FALLBACK: &str = "glyph-fallback";
pub const FLUID_NOT_SCALED: &str = "fluid-not-scaled";

const fn rule(id: &'static str, severity: Severity) -> Rule {
    Rule { id, severity }
}

/// Every rule, in the order the findings list them.
pub const RULES: &[Rule] = &[
    rule(EXTERNAL_HOST, Severity::Warning),
    rule(FONT_DISPLAY_MISSING, Severity::Warning),
    rule(FORMAT_NOT_WOFF2, Severity::Warning),
    rule(PRELOAD_MISSING, Severity::Info),
    rule(FACE_UNUSED, Severity::Info),
    rule(TOO_MANY_WEIGHTS, Severity::Warning),
    rule(LARGE_FILE, Severity::Warning),
    rule(TEXT_TOO_SMALL, Severity::Warning),
    rule(INPUT_TOO_SMALL, Severity::Warning),
    rule(LINE_HEIGHT_TIGHT, Severity::Info),
    rule(RENDERED_FALLBACK, Severity::Error),
    rule(GLYPH_FALLBACK, Severity::Warning),
    rule(FLUID_NOT_SCALED, Severity::Info),
];

/// The rules the rendered-font check serves (it needs CDP).
pub const RENDERED_RULES: &[&str] = &[RENDERED_FALLBACK, GLYPH_FALLBACK];

/// The rules as `check.describe` answers them.
#[must_use]
pub fn describe() -> Vec<RuleInfo> {
    RULES
        .iter()
        .map(|rule| RuleInfo {
            id: rule.id.to_owned(),
            title: key(&format!("rules.{}", rule.id)),
            severity: rule.severity,
            shared_id: None,
            help_url: None,
        })
        .collect()
}

/// Whether a run of the rules `enabled` (every rule when empty) wants `id`.
#[must_use]
pub fn wants(enabled: &[String], id: &str) -> bool {
    enabled.is_empty() || enabled.iter().any(|rule| rule == id)
}

/// Where a finding points: an element, or a named thing without one.
pub enum At<'a> {
    Element(&'a Located),
    Named(String),
}

/// Where the findings of one view go: the rules the run asks for (all when empty).
pub struct Sink<'a> {
    enabled: &'a [String],
    view: ViewId,
    url: &'a str,
    pub findings: Vec<Finding>,
}

impl<'a> Sink<'a> {
    #[must_use]
    pub fn new(enabled: &'a [String], view: ViewId, url: &'a str) -> Self {
        Self {
            enabled,
            view,
            url,
            findings: Vec::new(),
        }
    }

    /// Whether rule `id` runs.
    #[must_use]
    pub fn wants(&self, id: &str) -> bool {
        wants(self.enabled, id)
    }

    /// A finding of rule `id` with the message `findings.<message>`.
    pub fn emit(&mut self, id: &str, message: &str, at: &At<'_>, params: &[(&str, String)]) {
        let Some(rule) = RULES.iter().find(|rule| rule.id == id) else {
            return;
        };
        if !self.wants(id) {
            return;
        }
        let (selector, path) = match at {
            At::Element(located) => (Some(located.selector.clone()), Some(located.path.clone())),
            At::Named(name) => (None, Some(name.clone())),
        };
        self.findings.push(Finding {
            rule: id.to_owned(),
            severity: rule.severity,
            message: key(&format!("findings.{message}")),
            params: params
                .iter()
                .map(|(name, value)| ((*name).to_owned(), value.clone()))
                .collect(),
            view: Some(self.view),
            url: Some(self.url.to_owned()),
            selector,
            path,
            data: None,
        });
    }
}

/// A number as a finding shows it: whole when it is, else two decimals at most.
#[must_use]
pub fn shown(value: f64) -> String {
    let rounded = (value * 100.0).round() / 100.0;
    if rounded.fract() == 0.0 {
        format!("{rounded:.0}")
    } else {
        let text = format!("{rounded:.2}");
        text.trim_end_matches('0').to_owned()
    }
}

#[cfg(test)]
#[path = "rules_tests.rs"]
mod tests;
