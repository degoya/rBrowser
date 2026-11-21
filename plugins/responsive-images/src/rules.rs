//! The rules of the responsive images check (the host shows them as `responsive-images.<rule>`)
//! and the findings made of the collector's issues.
//!
//! A rule's title is `plugin.responsive-images.rules.<id>`, a finding's message
//! `plugin.responsive-images.findings.<id>` or, for a rule with variants,
//! `plugin.responsive-images.findings.<id>.<variant>`. Every finding carries the view's density
//! (`dpr`) and the image's file (`image`); a finding's `selector` is the image's CSS selector,
//! its `path` Lighthouse's node path of the image, and its `data` names the panel.
//!
//! **Lighthouse (review condition of the job):** `oversized`, `undersized` and `lcp` claim the
//! shared ids `image-oversized`, `image-undersized` and `lcp-image-discovery`, and the path is
//! written the way Lighthouse writes it, so a finding of the same rule at the same element of the
//! same page is one finding with both plugins as sources: the Lighthouse plugin claims the same
//! ids for `image-delivery-insight`, `image-size-responsive` and `lcp-discovery-insight`
//! (`plugins/lighthouse/src/rules.rs`, `SHARED_IDS`).

use std::collections::BTreeMap;

use rb_plugin_api::{Finding, RuleInfo, Severity, ViewId};
use serde_json::json;

use crate::PANEL_ID;
use crate::page::{Image, Issue, PageImages};

/// The longest message parameter, in characters (the host takes 256).
pub const MAX_PARAM_CHARS: usize = 200;

pub const OVERSIZED: &str = "oversized";
pub const UNDERSIZED: &str = "undersized";
pub const SIZES_MISSING: &str = "sizes-missing";
pub const LCP: &str = "lcp";

/// The parameters an issue may carry (`web/src/analyze.ts`).
const PARAMS: &[&str] = &["dpr", "rendered", "needed", "file", "ratio", "factor"];

const SIZES_HELP: &str =
    "https://developer.mozilla.org/en-US/docs/Web/HTML/Guides/Responsive_images";
const LCP_HELP: &str = "https://web.dev/articles/optimize-lcp";

/// One rule of `describe`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rule {
    pub id: &'static str,
    pub severity: Severity,
    /// The id shared with Lighthouse's audit of the same problem.
    pub shared_id: Option<&'static str>,
    /// The variants of its message; none for a rule of one message.
    pub variants: &'static [&'static str],
}

/// Every rule, in the order the panel lists them.
pub const RULES: &[Rule] = &[
    Rule {
        id: OVERSIZED,
        severity: Severity::Warning,
        shared_id: Some("image-oversized"),
        variants: &[],
    },
    Rule {
        id: UNDERSIZED,
        severity: Severity::Warning,
        shared_id: Some("image-undersized"),
        variants: &[],
    },
    Rule {
        id: SIZES_MISSING,
        severity: Severity::Warning,
        shared_id: None,
        variants: &["img", "source"],
    },
    Rule {
        id: LCP,
        severity: Severity::Warning,
        shared_id: Some("lcp-image-discovery"),
        variants: &["lazy", "priority"],
    },
];

fn rule(id: &str) -> Option<&'static Rule> {
    RULES.iter().find(|rule| rule.id == id)
}

/// The rules as `check.describe` answers them.
#[must_use]
pub fn describe() -> Vec<RuleInfo> {
    RULES
        .iter()
        .map(|rule| RuleInfo {
            id: rule.id.to_owned(),
            title: crate::key(&format!("rules.{}", rule.id)),
            severity: rule.severity,
            shared_id: rule.shared_id.map(str::to_owned),
            help_url: Some(if rule.id == LCP { LCP_HELP } else { SIZES_HELP }.to_owned()),
        })
        .collect()
}

/// The severity of `rule`; `Info` for one not described.
#[must_use]
pub fn severity_of(id: &str) -> Severity {
    rule(id).map_or(Severity::Info, |rule| rule.severity)
}

fn cut(text: &str) -> String {
    text.chars().take(MAX_PARAM_CHARS).collect()
}

/// `issue` as the plugin knows it: `None` for an unknown rule or variant (a variant is required
/// where the rule has some), its parameters only the known ones, each cut.
#[must_use]
pub fn known_issue(issue: Issue) -> Option<Issue> {
    let known = rule(&issue.rule)?;
    let variant_ok = match issue.variant.as_deref() {
        None => known.variants.is_empty(),
        Some(variant) => known.variants.contains(&variant),
    };
    if !variant_ok {
        return None;
    }
    let params: BTreeMap<String, String> = issue
        .params
        .into_iter()
        .filter(|(key, _)| PARAMS.contains(&key.as_str()))
        .map(|(key, value)| (key, cut(&value)))
        .collect();
    Some(Issue { params, ..issue })
}

/// The message key of `issue`, below `plugin.responsive-images.`.
#[must_use]
pub fn message(issue: &Issue) -> String {
    match &issue.variant {
        Some(variant) => crate::key(&format!("findings.{}.{variant}", issue.rule)),
        None => crate::key(&format!("findings.{}", issue.rule)),
    }
}

/// The finding of `issue` of `image` in `view`, on the page `url`, at the density `dpr`.
#[must_use]
pub fn finding(view: ViewId, url: &str, dpr: f64, image: &Image, issue: &Issue) -> Finding {
    let mut params: BTreeMap<String, String> = issue.params.clone();
    params
        .entry("dpr".to_owned())
        .or_insert_with(|| format!("{}", (dpr * 100.0).round() / 100.0));
    params.insert("image".to_owned(), cut(&image.current_src));
    Finding {
        rule: issue.rule.clone(),
        severity: severity_of(&issue.rule),
        message: message(issue),
        params: params.into_iter().collect(),
        view: Some(view),
        url: Some(url.to_owned()),
        selector: (!image.selector.is_empty()).then(|| image.selector.clone()),
        path: (!image.path.is_empty()).then(|| image.path.clone()),
        data: Some(json!({ "panel": PANEL_ID }).to_string()),
    }
}

/// The findings of `page`, shown in `view`, of the rules `enabled` names (every rule when empty).
#[must_use]
pub fn findings(view: ViewId, page: &PageImages, enabled: &[String]) -> Vec<Finding> {
    let mut out = Vec::new();
    for image in &page.images {
        for issue in &image.issues {
            if enabled.is_empty() || enabled.iter().any(|id| *id == issue.rule) {
                out.push(finding(view, &page.url, page.dpr, image, issue));
            }
        }
    }
    out
}

#[cfg(test)]
#[path = "rules_tests.rs"]
mod tests;
