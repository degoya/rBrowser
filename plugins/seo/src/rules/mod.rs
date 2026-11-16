//! The rules of the SEO check, one module per group, and the sink their findings go through.
//!
//! A rule id is `<group>.<name>` (`images.alt_missing`); the host shows it qualified with the
//! plugin's id (`seo.images.alt_missing`), so `seo.images` is the alt text group. Its title is
//! `plugin.seo.rules.<id>`, a finding's message `plugin.seo.findings.<id>` or a variant below it.
//! The hreflang group (RB-060-09) lives in `crate::hreflang`, the parity group in
//! `crate::parity`; their rules are listed here with the others. RB-080-11 adds the share image
//! and Twitter card to `social`, and the groups `wellknown` (`llms.txt`, `security.txt`),
//! `microdata` (Microdata and RDFa) and `head` (the order of the raw head).

use rb_plugin_api::{Finding, RuleInfo, Severity, ViewId};
use scraper::ElementRef;

use crate::config::{Config, Level};
use crate::path::{element_path, selector_of};

pub mod canonical;
pub mod diff;
pub mod head;
pub mod headings;
pub mod images;
pub mod indexability;
pub mod meta;
pub mod microdata;
pub mod site;
pub mod social;
pub mod social_image;
pub mod wellknown;

/// One rule of `describe`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rule {
    pub id: &'static str,
    pub severity: Severity,
    /// The id other plugins' rules finding the same share (`img-alt-missing`, with `a11y-axe`).
    pub shared: Option<&'static str>,
}

const fn rule(id: &'static str, severity: Severity) -> Rule {
    Rule {
        id,
        severity,
        shared: None,
    }
}

const fn shared(id: &'static str, severity: Severity, shared: &'static str) -> Rule {
    Rule {
        id,
        severity,
        shared: Some(shared),
    }
}

use Severity::{Error, Info, Warning};

/// Every rule, in the order the report lists them.
pub const RULES: &[Rule] = &[
    rule("index.status", Error),
    rule("index.redirect", Warning),
    rule("index.redirect_broken", Error),
    rule("index.noindex_meta", Error),
    rule("index.noindex_header", Error),
    rule("index.robots_blocked", Error),
    rule("canonical.missing", Warning),
    rule("canonical.multiple", Error),
    rule("canonical.relative", Warning),
    rule("canonical.target", Error),
    rule("canonical.not_self", Info),
    rule("meta.title_missing", Error),
    rule("meta.title_multiple", Warning),
    rule("meta.title_length", Warning),
    rule("meta.title_duplicate", Warning),
    rule("meta.description_missing", Warning),
    rule("meta.description_multiple", Warning),
    rule("meta.description_length", Warning),
    rule("meta.description_duplicate", Warning),
    shared("meta.lang_missing", Warning, "html-lang-missing"),
    rule("meta.viewport_missing", Error),
    rule("meta.charset_missing", Warning),
    shared("headings.h1_missing", Error, "h1-missing"),
    shared("headings.h1_multiple", Warning, "h1-multiple"),
    shared("headings.skipped", Warning, "heading-order"),
    rule("social.og_missing", Warning),
    rule("social.image_relative", Error),
    rule("social.image_unreachable", Error),
    rule("social.image_type", Error),
    rule("social.image_small", Warning),
    rule("social.image_ratio", Info),
    rule("social.card_small", Info),
    rule("social.twitter_card", Warning),
    rule("social.url_canonical", Warning),
    rule("structured.jsonld_invalid", Error),
    rule("site.sitemap_missing", Warning),
    rule("site.sitemap_unreachable", Error),
    rule("render.differs", Warning),
    shared("images.alt_missing", Error, "img-alt-missing"),
    rule("images.alt_empty", Warning),
    shared("images.linked_no_name", Error, "link-name-missing"),
    rule("images.alt_filename", Warning),
    rule("images.alt_placeholder", Warning),
    rule("images.alt_prefix", Warning),
    rule("images.alt_too_long", Warning),
    rule("images.alt_duplicate", Warning),
    rule("images.alt_redundant", Warning),
    rule("images.alt_language", Warning),
    rule("images.input_alt_missing", Error),
    rule("images.area_alt_missing", Error),
    rule("images.svg_name_missing", Warning),
    rule("images.social_alt_missing", Info),
    rule("images.background", Info),
    rule("images.alt_untranslated", Info),
    rule("hreflang.none", Info),
    rule("hreflang.source_conflict", Error),
    rule("hreflang.source_missing", Warning),
    rule("hreflang.code_invalid", Error),
    rule("hreflang.target_relative", Error),
    rule("hreflang.duplicate_code", Error),
    rule("hreflang.duplicate_url", Warning),
    rule("hreflang.self_missing", Error),
    rule("hreflang.x_default_missing", Warning),
    rule("hreflang.locale_missing", Warning),
    rule("hreflang.target_status", Error),
    rule("hreflang.target_noindex", Error),
    rule("hreflang.target_canonical", Error),
    rule("hreflang.target_unchecked", Info),
    rule("hreflang.lang_mismatch", Warning),
    rule("hreflang.content_detected", Warning),
    rule("hreflang.return_missing", Error),
    rule("hreflang.set_incomplete", Warning),
    rule("parity.title", Warning),
    rule("parity.h1", Warning),
    rule("parity.canonical", Error),
    rule("parity.hreflang", Error),
    rule("parity.structured_data", Warning),
    rule("parity.text", Warning),
    rule("wellknown.llms_missing", Info),
    rule("wellknown.llms_h1", Error),
    rule("wellknown.llms_structure", Warning),
    rule("wellknown.llms_link", Error),
    rule("wellknown.security_missing", Info),
    rule("wellknown.security_location", Info),
    rule("wellknown.security_transport", Error),
    rule("wellknown.security_contact", Error),
    rule("wellknown.security_expires", Error),
    rule("wellknown.security_expires_far", Warning),
    rule("microdata.type_missing", Warning),
    rule("microdata.type_unknown", Info),
    rule("microdata.required_missing", Error),
    rule("microdata.jsonld_conflict", Warning),
    rule("head.charset_late", Error),
    rule("head.charset_not_first", Warning),
    rule("head.viewport_late", Warning),
    rule("head.title_late", Warning),
    rule("head.preconnect_late", Warning),
    rule("head.script_before_css", Warning),
    rule("head.in_body", Error),
    rule("head.rendered_differs", Info),
];

/// The rule groups a run may be narrowed to (`seo_check`'s `groups`).
pub const GROUPS: [&str; 14] = [
    "index",
    "canonical",
    "meta",
    "headings",
    "social",
    "structured",
    "site",
    "render",
    "images",
    "hreflang",
    "parity",
    "wellknown",
    "microdata",
    "head",
];

/// The ids of every rule of `groups`.
#[must_use]
pub fn of_groups(groups: &[String]) -> Vec<String> {
    RULES
        .iter()
        .filter(|rule| {
            rule.id
                .split_once('.')
                .is_some_and(|(group, _)| groups.iter().any(|g| g == group))
        })
        .map(|rule| rule.id.to_owned())
        .collect()
}

/// The rule `id`.
#[must_use]
pub fn find(id: &str) -> Option<&'static Rule> {
    RULES.iter().find(|rule| rule.id == id)
}

/// The rules as `check.describe` answers them.
#[must_use]
pub fn describe() -> Vec<RuleInfo> {
    RULES
        .iter()
        .map(|rule| RuleInfo {
            id: rule.id.to_owned(),
            title: format!("plugin.seo.rules.{}", rule.id),
            severity: rule.severity,
            shared_id: rule.shared.map(str::to_owned),
            help_url: None,
        })
        .collect()
}

/// The element a finding is about.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Element {
    pub selector: String,
    pub path: String,
}

impl Element {
    #[must_use]
    pub fn of(element: ElementRef<'_>) -> Self {
        Self {
            selector: selector_of(element),
            path: element_path(element),
        }
    }

    /// An element known only by its path (a heading, a background).
    #[must_use]
    pub fn at(path: &str) -> Self {
        Self {
            selector: path.to_owned(),
            path: path.to_owned(),
        }
    }
}

/// Where findings go: the rules switched on (by the run and `seo.rules`), the severities
/// `seo.rules` sets and the patterns of `seo.ignore` applied as they arrive.
pub struct Sink<'a> {
    config: &'a Config,
    /// The rules the run asks for (`check-context.rules`); every rule when empty.
    enabled: &'a [String],
    view: Option<ViewId>,
    url: String,
    pub findings: Vec<Finding>,
}

impl<'a> Sink<'a> {
    #[must_use]
    pub fn new(config: &'a Config, enabled: &'a [String]) -> Self {
        Self {
            config,
            enabled,
            view: None,
            url: String::new(),
            findings: Vec::new(),
        }
    }

    /// The page and view the next findings are about.
    pub fn page(&mut self, url: &str, view: Option<ViewId>) {
        url.clone_into(&mut self.url);
        self.view = view;
    }

    /// The URL of the page the findings are about.
    #[must_use]
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Whether `rule` runs at all: asked for and not switched off.
    #[must_use]
    pub fn wants(&self, rule: &str) -> bool {
        (self.enabled.is_empty() || self.enabled.iter().any(|id| id == rule))
            && self.config.level(rule) != Some(Level::Off)
    }

    /// A finding of `rule` with the message `plugin.seo.findings.<message>`.
    pub fn emit(
        &mut self,
        rule: &str,
        message: &str,
        element: Option<Element>,
        params: &[(&str, String)],
    ) {
        self.keep(rule, message, element, params);
    }

    /// [`Sink::emit`], answering whether the finding was kept (the rule runs and no ignore
    /// pattern drops it).
    pub fn keep(
        &mut self,
        rule: &str,
        message: &str,
        element: Option<Element>,
        params: &[(&str, String)],
    ) -> bool {
        let Some(known) = find(rule) else {
            return false;
        };
        if !self.wants(rule) {
            return false;
        }
        let selector = element.as_ref().map(|element| element.selector.as_str());
        if self.config.ignores(rule, Some(&self.url), selector) {
            return false;
        }
        let severity = match self.config.level(rule) {
            Some(Level::Severity(severity)) => severity,
            _ => known.severity,
        };
        self.findings.push(Finding {
            rule: rule.to_owned(),
            severity,
            message: format!("plugin.seo.findings.{message}"),
            params: params
                .iter()
                .map(|(key, value)| ((*key).to_owned(), value.clone()))
                .collect(),
            view: self.view,
            url: Some(self.url.clone()),
            selector: element.as_ref().map(|element| element.selector.clone()),
            path: element.map(|element| element.path),
            data: None,
        });
        true
    }
}

/// `text` cut to `max` characters for a message parameter, `...` marking the cut.
#[must_use]
pub fn short(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_owned()
    } else {
        let mut cut: String = text.chars().take(max).collect();
        cut.push_str("...");
        cut
    }
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
