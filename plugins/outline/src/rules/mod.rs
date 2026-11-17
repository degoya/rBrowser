//! The outline's rules and the findings one view's outline gives (`check.describe`, `check.run`).
//!
//! The host qualifies each id with the plugin's (`outline.heading-skip`); a rule that finds the
//! problem another plugin finds too carries their shared id, so the host merges the findings at
//! the same element of the same page into one (`docs/plugins.md`, "Check plugins"): with `seo`
//! the headings (`h1-missing`, `h1-multiple`, `heading-order`), images and link names; with
//! `a11y-axe` the image alts, link names, labels, empty headings, the landmark rules and
//! `region`. A finding about the whole page sits at `html`, where axe reports it. Only elements
//! that are exposed count (not `display: none`), as axe and the screen readers see the page.
//!
//! The groups live in their own modules: [`headings`], [`landmarks`], [`focus`] and the rules
//! over the breakpoints, [`across`], which compare a view with the others on the same page.

use rb_plugin_api::{Finding, RuleInfo, Severity, ViewId};

use crate::key;
use crate::links::LinkResult;
use crate::page::{Located, PageOutline};
use crate::settings::Settings;

mod across;
mod focus;
mod headings;
mod landmarks;
#[cfg(test)]
mod test_util;

pub use across::{Peer, across};

/// One rule of `describe`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rule {
    pub id: &'static str,
    pub severity: Severity,
    pub shared: Option<&'static str>,
}

const fn rule(id: &'static str, severity: Severity, shared: Option<&'static str>) -> Rule {
    Rule {
        id,
        severity,
        shared,
    }
}

pub const HEADING_SKIP: &str = "heading-skip";
pub const MULTIPLE_H1: &str = "multiple-h1";
pub const H1_MISSING: &str = "h1-missing";
pub const HEADING_FIRST_NOT_H1: &str = "heading-first-not-h1";
pub const HEADING_EMPTY: &str = "heading-empty";
pub const HEADING_IMAGE_NO_ALT: &str = "heading-image-no-alt";
pub const HEADING_TOO_LONG: &str = "heading-too-long";
pub const LANDMARK_MAIN_MISSING: &str = "landmark-main-missing";
pub const LANDMARK_MAIN_MULTIPLE: &str = "landmark-main-multiple";
pub const LANDMARK_BANNER_MULTIPLE: &str = "landmark-banner-multiple";
pub const LANDMARK_CONTENTINFO_MULTIPLE: &str = "landmark-contentinfo-multiple";
pub const LANDMARK_BANNER_NESTED: &str = "landmark-banner-nested";
pub const LANDMARK_CONTENTINFO_NESTED: &str = "landmark-contentinfo-nested";
pub const LANDMARK_UNIQUE: &str = "landmark-unique";
pub const CONTENT_OUTSIDE_LANDMARKS: &str = "content-outside-landmarks";
pub const LINK_BROKEN: &str = "link-broken";
pub const LINK_EMPTY: &str = "link-empty";
pub const IMG_ALT_MISSING: &str = "img-alt-missing";
pub const FORM_LABEL_MISSING: &str = "form-label-missing";
pub const FOCUS_ORDER_JUMP: &str = "focus-order-jump";
pub const HEADING_DIFFERS: &str = "heading-differs";
pub const LANDMARK_DIFFERS: &str = "landmark-differs";

use Severity::{Error, Info, Warning};

/// Every rule, in the order the findings list them.
pub const RULES: &[Rule] = &[
    rule(HEADING_SKIP, Warning, Some("heading-order")),
    rule(MULTIPLE_H1, Warning, Some("h1-multiple")),
    rule(H1_MISSING, Warning, Some("h1-missing")),
    rule(HEADING_FIRST_NOT_H1, Info, None),
    rule(HEADING_EMPTY, Warning, Some("heading-empty")),
    rule(HEADING_IMAGE_NO_ALT, Error, Some("heading-empty")),
    rule(HEADING_TOO_LONG, Info, None),
    rule(
        LANDMARK_MAIN_MISSING,
        Warning,
        Some("landmark-main-missing"),
    ),
    rule(
        LANDMARK_MAIN_MULTIPLE,
        Warning,
        Some("landmark-main-multiple"),
    ),
    rule(
        LANDMARK_BANNER_MULTIPLE,
        Warning,
        Some("landmark-banner-multiple"),
    ),
    rule(
        LANDMARK_CONTENTINFO_MULTIPLE,
        Warning,
        Some("landmark-contentinfo-multiple"),
    ),
    rule(
        LANDMARK_BANNER_NESTED,
        Warning,
        Some("landmark-banner-nested"),
    ),
    rule(
        LANDMARK_CONTENTINFO_NESTED,
        Warning,
        Some("landmark-contentinfo-nested"),
    ),
    rule(LANDMARK_UNIQUE, Warning, Some("landmark-unique")),
    rule(
        CONTENT_OUTSIDE_LANDMARKS,
        Info,
        Some("content-outside-landmarks"),
    ),
    rule(LINK_BROKEN, Error, None),
    rule(LINK_EMPTY, Error, Some("link-name-missing")),
    rule(IMG_ALT_MISSING, Error, Some("img-alt-missing")),
    rule(FORM_LABEL_MISSING, Error, Some("form-label-missing")),
    rule(FOCUS_ORDER_JUMP, Warning, None),
    rule(HEADING_DIFFERS, Info, None),
    rule(LANDMARK_DIFFERS, Info, None),
];

/// The rules as `check.describe` answers them.
#[must_use]
pub fn describe() -> Vec<RuleInfo> {
    RULES
        .iter()
        .map(|rule| RuleInfo {
            id: rule.id.to_owned(),
            title: key(&format!("rules.{}", rule.id)),
            severity: rule.severity,
            shared_id: rule.shared.map(str::to_owned),
            help_url: None,
        })
        .collect()
}

/// The page and view a finding is about.
type Page<'a> = (&'a str, ViewId);

/// Where the findings of one view go: the rules the run asks for (all when empty).
pub struct Sink<'a> {
    enabled: &'a [String],
    pub findings: Vec<Finding>,
}

impl<'a> Sink<'a> {
    #[must_use]
    pub fn new(enabled: &'a [String]) -> Self {
        Self {
            enabled,
            findings: Vec::new(),
        }
    }

    /// Whether rule `id` runs.
    #[must_use]
    pub fn wants(&self, id: &str) -> bool {
        self.enabled.is_empty() || self.enabled.iter().any(|rule| rule == id)
    }

    fn emit(&mut self, id: &str, page: Page<'_>, at: Option<&Located>, params: &[(&str, String)]) {
        let Some(rule) = RULES.iter().find(|rule| rule.id == id) else {
            return;
        };
        if !self.wants(id) {
            return;
        }
        self.findings.push(Finding {
            rule: id.to_owned(),
            severity: rule.severity,
            message: key(&format!("findings.{id}")),
            params: params
                .iter()
                .map(|(name, value)| ((*name).to_owned(), value.clone()))
                .collect(),
            view: Some(page.1),
            url: Some(page.0.to_owned()),
            selector: at.map(|at| at.selector.clone()),
            path: at.map(|at| at.path.clone()),
            data: None,
        });
    }
}

/// The document element: where a finding about the whole page sits, as axe reports it.
fn root() -> Located {
    Located {
        selector: "html".to_owned(),
        path: "html".to_owned(),
        ..Located::default()
    }
}

/// A heading's level, 1 to 9 (an `aria-level` beyond 6 is allowed); the page's nonsense is 2.
fn level(value: i64) -> u32 {
    u32::try_from(value)
        .ok()
        .filter(|level| (1..=9).contains(level))
        .unwrap_or(2)
}

/// The findings of one view's outline; `checked` the results of the link check, if one ran.
/// `sections` are the sections the outline was collected with (rules of the others stay quiet).
#[must_use]
pub fn findings(
    enabled: &[String],
    view: ViewId,
    outline: &PageOutline,
    sections: &[String],
    checked: &[LinkResult],
    settings: &Settings,
) -> Vec<Finding> {
    let has = |name: &str| sections.is_empty() || sections.iter().any(|s| s == name);
    let mut sink = Sink::new(enabled);
    let page = (outline.url.as_str(), view);
    if has("headings") {
        headings::check(&mut sink, page, outline, settings.heading_max_chars);
    }
    if has("landmarks") {
        landmarks::check(&mut sink, page, outline);
    }
    if has("links") {
        for link in outline.links.iter().filter(|l| !l.at.hidden) {
            if link.text.is_empty() {
                sink.emit(
                    LINK_EMPTY,
                    page,
                    Some(&link.at),
                    &[("href", link.href.clone())],
                );
            }
            let target = crate::url::without_fragment(&link.href);
            if let Some(result) = checked.iter().find(|r| r.url == target && r.is_broken()) {
                sink.emit(
                    LINK_BROKEN,
                    page,
                    Some(&link.at),
                    &[("url", result.url.clone()), ("status", result.describe())],
                );
            }
        }
    }
    if has("images") {
        for image in outline.images.iter().filter(|i| !i.at.hidden) {
            if image.alt.is_none() && !image.decorative {
                sink.emit(
                    IMG_ALT_MISSING,
                    page,
                    Some(&image.at),
                    &[("src", image.src.clone())],
                );
            }
        }
    }
    if has("forms") {
        for field in outline.fields.iter().filter(|f| !f.at.hidden) {
            if field.label.is_empty() {
                let name = if field.name.is_empty() {
                    field.kind.clone()
                } else {
                    field.name.clone()
                };
                sink.emit(FORM_LABEL_MISSING, page, Some(&field.at), &[("name", name)]);
            }
        }
    }
    if has("focus") {
        focus::check(&mut sink, page, outline);
    }
    sink.findings
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
