//! An rBrowser check plugin: the rule `title.missing` finds a page without a `<title>`, the rule
//! `lang.missing` one whose `<html>` names no language.
//!
//! What the host promises a check plugin (docs/plugins.md, "Check plugins"):
//!
//! - `describe` lists the rules once: id, title key, usual severity, optionally a shared id (the
//!   same problem another plugin finds too: their findings at one element become one) and a help
//!   page.
//! - `run` gets a context: the view (none to choose them yourself), the URL, the rules the person
//!   left switched on and JSON arguments. It answers at most 10 000 findings, each of a described
//!   rule and speaking in the plugin's own i18n keys (`plugin.<id>.*`) - anything else is
//!   refused. `host.report-progress` tells the findings panel how far a long run is.
//! - The host qualifies each rule with the plugin's id (`example-check.title.missing`), holds it
//!   to the person's overrides (`checks.rules`, `checks.ignore`) and merges the findings of every
//!   view and every check plugin into one list with stable ids - made of the rule, the URL and
//!   the element `path`, so give a path that stays the same between runs.
//! - Every host function behind a capability the manifest does not grant is missing.
//!
//! The logic lives here and is tested natively (`cargo test`); `src/guest.rs` only translates it
//! into the vocabulary of `wit/rbrowser-plugin.wit` and exists on `wasm32` alone.

#[cfg(target_arch = "wasm32")]
mod guest;

/// How serious a finding is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Info,
    Warning,
    Error,
}

/// One rule as `describe` lists it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rule {
    pub id: &'static str,
    /// The i18n key of its title.
    pub title: &'static str,
    pub severity: Level,
    pub shared_id: Option<&'static str>,
    pub help_url: Option<&'static str>,
}

/// The page has no title.
pub const TITLE_MISSING: &str = "title.missing";
/// The document names no language.
pub const LANG_MISSING: &str = "lang.missing";

/// Every rule of the plugin.
pub const RULES: [Rule; 2] = [
    Rule {
        id: TITLE_MISSING,
        title: "plugin.example-check.rules.title_missing",
        severity: Level::Warning,
        shared_id: Some("document-title-missing"),
        help_url: Some("https://developer.mozilla.org/docs/Web/HTML/Element/title"),
    },
    Rule {
        id: LANG_MISSING,
        title: "plugin.example-check.rules.lang_missing",
        severity: Level::Error,
        shared_id: Some("html-lang-missing"),
        help_url: Some("https://developer.mozilla.org/docs/Web/HTML/Global_attributes/lang"),
    },
];

/// What the page is asked: its title and its language, each answered as a JSON string.
pub const TITLE_SCRIPT: &str = "document.title";
pub const LANG_SCRIPT: &str = "document.documentElement.lang";

/// One finding as the logic makes it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Finding {
    pub rule: &'static str,
    pub severity: Level,
    pub message: &'static str,
    pub selector: &'static str,
    pub path: &'static str,
}

fn blank(json: &str) -> bool {
    json.trim().trim_matches('"').trim().is_empty()
}

/// The findings of a page whose title evaluated to `title_json` and whose language to
/// `lang_json` (JSON strings), for the switched-on `rules`.
#[must_use]
pub fn page_findings(rules: &[String], title_json: &str, lang_json: &str) -> Vec<Finding> {
    let on = |rule: &str| rules.iter().any(|id| id == rule);
    let mut findings = Vec::new();
    if on(TITLE_MISSING) && blank(title_json) {
        findings.push(Finding {
            rule: TITLE_MISSING,
            severity: Level::Warning,
            message: "plugin.example-check.findings.title_missing",
            selector: "head",
            path: "html > head",
        });
    }
    if on(LANG_MISSING) && blank(lang_json) {
        findings.push(Finding {
            rule: LANG_MISSING,
            severity: Level::Error,
            message: "plugin.example-check.findings.lang_missing",
            selector: "html",
            path: "html",
        });
    }
    findings
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
