//! The outline's rules and the findings one view's outline gives (`check.describe`, `check.run`).
//!
//! The host qualifies each id with the plugin's (`outline.heading-skip`); a rule that finds the
//! problem another plugin finds too carries their shared id, so the host merges the findings at
//! the same element of the same page into one: `img-alt-missing` and `link-name-missing` with
//! `seo` and `a11y-axe`, `form-label-missing` with `a11y-axe`'s `label`. Only elements that are
//! exposed count (not `display: none`), as axe and the screen readers see the page.

use rb_plugin_api::{Finding, RuleInfo, Severity, ViewId};

use crate::key;
use crate::links::LinkResult;
use crate::page::{Located, PageOutline};

/// One rule of `describe`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rule {
    pub id: &'static str,
    pub severity: Severity,
    pub shared: Option<&'static str>,
}

pub const HEADING_SKIP: &str = "heading-skip";
pub const MULTIPLE_H1: &str = "multiple-h1";
pub const LANDMARK_MAIN_MISSING: &str = "landmark-main-missing";
pub const LINK_BROKEN: &str = "link-broken";
pub const LINK_EMPTY: &str = "link-empty";
pub const IMG_ALT_MISSING: &str = "img-alt-missing";
pub const FORM_LABEL_MISSING: &str = "form-label-missing";

/// Every rule, in the order the findings list them.
pub const RULES: &[Rule] = &[
    Rule {
        id: HEADING_SKIP,
        severity: Severity::Warning,
        shared: None,
    },
    Rule {
        id: MULTIPLE_H1,
        severity: Severity::Warning,
        shared: None,
    },
    Rule {
        id: LANDMARK_MAIN_MISSING,
        severity: Severity::Warning,
        shared: None,
    },
    Rule {
        id: LINK_BROKEN,
        severity: Severity::Error,
        shared: None,
    },
    Rule {
        id: LINK_EMPTY,
        severity: Severity::Error,
        shared: Some("link-name-missing"),
    },
    Rule {
        id: IMG_ALT_MISSING,
        severity: Severity::Error,
        shared: Some("img-alt-missing"),
    },
    Rule {
        id: FORM_LABEL_MISSING,
        severity: Severity::Error,
        shared: Some("form-label-missing"),
    },
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

    fn emit(
        &mut self,
        id: &str,
        page: (&str, ViewId),
        at: Option<&Located>,
        params: &[(&str, String)],
    ) {
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

/// A heading's level, 1 to 9 (an `aria-level` beyond 6 is allowed); the page's nonsense is 2.
fn level(value: i64) -> u32 {
    u32::try_from(value)
        .ok()
        .filter(|level| (1..=9).contains(level))
        .unwrap_or(2)
}

fn headings(sink: &mut Sink<'_>, page: (&str, ViewId), outline: &PageOutline) {
    let exposed: Vec<_> = outline.headings.iter().filter(|h| !h.at.hidden).collect();
    let mut previous: Option<u32> = None;
    for heading in &exposed {
        let current = level(heading.level);
        if let Some(before) = previous
            && current > before + 1
        {
            sink.emit(
                HEADING_SKIP,
                page,
                Some(&heading.at),
                &[
                    ("from", format!("h{before}")),
                    ("to", format!("h{current}")),
                    ("text", heading.text.clone()),
                ],
            );
        }
        previous = Some(current);
    }
    let h1: Vec<_> = exposed.iter().filter(|h| level(h.level) == 1).collect();
    if h1.len() > 1 {
        for heading in h1.iter().skip(1) {
            sink.emit(
                MULTIPLE_H1,
                page,
                Some(&heading.at),
                &[
                    ("count", h1.len().to_string()),
                    ("text", heading.text.clone()),
                ],
            );
        }
    }
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
) -> Vec<Finding> {
    let has = |name: &str| sections.is_empty() || sections.iter().any(|s| s == name);
    let mut sink = Sink::new(enabled);
    let page = (outline.url.as_str(), view);
    if has("headings") {
        headings(&mut sink, page, outline);
    }
    if has("landmarks")
        && !outline
            .landmarks
            .iter()
            .any(|l| l.role == "main" && !l.at.hidden)
    {
        let root = Located {
            selector: "html".to_owned(),
            path: "html".to_owned(),
            ..Located::default()
        };
        sink.emit(LANDMARK_MAIN_MISSING, page, Some(&root), &[]);
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
    sink.findings
}

#[cfg(test)]
#[path = "rules_tests.rs"]
mod tests;
