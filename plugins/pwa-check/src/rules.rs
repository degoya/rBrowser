//! The rules of the PWA check (group `pwa`: the host shows them as `pwa-check.<rule>`) and the
//! sink their findings go through.
//!
//! A rule's title is `plugin.pwa-check.rules.<id>`, a finding's message
//! `plugin.pwa-check.findings.<id>`. Most findings are about the manifest, the service worker or
//! Chromium's verdict rather than an element, so a finding names no selector and its `path` is a
//! place of its own (`pwa:manifest/icons/3`): the stable id of the findings model is made of it,
//! and two findings of one rule at two places stay two. Its `data` names the panel and the place
//! in it (`{"panel": "pwa", "section": "icons", "anchor": "icon-3"}`).

use rb_plugin_api::{Finding, RuleInfo, Severity, ViewId};
use serde_json::json;

use crate::PANEL_ID;
use crate::manifest::cut;

/// The longest message parameter, in characters (the host takes 256).
pub const MAX_PARAM_CHARS: usize = 200;

pub const MANIFEST_MISSING: &str = "manifest.missing";
pub const MANIFEST_UNREACHABLE: &str = "manifest.unreachable";
pub const MANIFEST_CONTENT_TYPE: &str = "manifest.content_type";
pub const MANIFEST_INVALID: &str = "manifest.invalid";
pub const MANIFEST_NAME: &str = "manifest.name";
pub const MANIFEST_START_URL: &str = "manifest.start_url";
pub const MANIFEST_DISPLAY: &str = "manifest.display";
pub const MANIFEST_ICON_SIZE: &str = "manifest.icon_size";
pub const MANIFEST_RECOMMENDED: &str = "manifest.recommended";
pub const FETCH_OTHER_ORIGIN: &str = "fetch.other_origin";
pub const ICONS_UNREACHABLE: &str = "icons.unreachable";
pub const ICONS_NOT_IMAGE: &str = "icons.not_image";
pub const ICONS_SIZE_MISMATCH: &str = "icons.size_mismatch";
pub const ICONS_TYPE_MISMATCH: &str = "icons.type_mismatch";
pub const START_URL_OUT_OF_SCOPE: &str = "start_url.out_of_scope";
pub const START_URL_STATUS: &str = "start_url.status";
pub const SW_INSECURE: &str = "sw.insecure";
pub const SW_MISSING: &str = "sw.missing";
pub const SW_NOT_ACTIVE: &str = "sw.not_active";
pub const SW_NOT_CONTROLLING: &str = "sw.not_controlling";
pub const SW_FETCH_HANDLER: &str = "sw.fetch_handler";
pub const OFFLINE_NOT_CACHED: &str = "offline.not_cached";
pub const INSTALL_ERROR: &str = "install.error";
pub const META_THEME_COLOR: &str = "meta.theme_color";
pub const META_APPLE_TOUCH_ICON: &str = "meta.apple_touch_icon";

/// One rule of `describe`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rule {
    pub id: &'static str,
    pub severity: Severity,
}

const fn rule(id: &'static str, severity: Severity) -> Rule {
    Rule { id, severity }
}

use Severity::{Error, Info, Warning};

/// Every rule, in the order the panel lists them.
pub const RULES: &[Rule] = &[
    rule(MANIFEST_MISSING, Error),
    rule(MANIFEST_UNREACHABLE, Error),
    rule(MANIFEST_CONTENT_TYPE, Warning),
    rule(MANIFEST_INVALID, Error),
    rule(MANIFEST_NAME, Error),
    rule(MANIFEST_START_URL, Error),
    rule(MANIFEST_DISPLAY, Error),
    rule(MANIFEST_ICON_SIZE, Error),
    rule(MANIFEST_RECOMMENDED, Info),
    rule(FETCH_OTHER_ORIGIN, Info),
    rule(ICONS_UNREACHABLE, Error),
    rule(ICONS_NOT_IMAGE, Error),
    rule(ICONS_SIZE_MISMATCH, Warning),
    rule(ICONS_TYPE_MISMATCH, Warning),
    rule(START_URL_OUT_OF_SCOPE, Error),
    rule(START_URL_STATUS, Error),
    rule(SW_INSECURE, Error),
    rule(SW_MISSING, Error),
    rule(SW_NOT_ACTIVE, Error),
    rule(SW_NOT_CONTROLLING, Warning),
    rule(SW_FETCH_HANDLER, Warning),
    rule(OFFLINE_NOT_CACHED, Warning),
    rule(INSTALL_ERROR, Error),
    rule(META_THEME_COLOR, Info),
    rule(META_APPLE_TOUCH_ICON, Info),
];

/// The rules as `check.describe` answers them.
#[must_use]
pub fn describe() -> Vec<RuleInfo> {
    RULES
        .iter()
        .map(|rule| RuleInfo {
            id: rule.id.to_owned(),
            title: crate::key(&format!("rules.{}", rule.id)),
            severity: rule.severity,
            shared_id: None,
            help_url: None,
        })
        .collect()
}

/// Where a finding is: its path, and the section and anchor of the panel that shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Place {
    /// The manifest as a whole (its link, its answer, its text).
    Manifest,
    /// A member of the manifest.
    Field(&'static str),
    /// The icon of this size the manifest lacks.
    RequiredIcon(u32),
    /// The entry of `icons` at this index.
    Icon(usize),
    ServiceWorker,
    /// The offline test of the start URL.
    Offline,
    /// Chromium's installability error with this id.
    Install(String),
    /// A `<meta>` or `<link>` of the page's head.
    Meta(&'static str),
}

impl Place {
    /// The finding's `path`.
    #[must_use]
    pub fn path(&self) -> String {
        match self {
            Self::Manifest => "pwa:manifest".to_owned(),
            Self::Field(field) => format!("pwa:manifest/{field}"),
            Self::RequiredIcon(size) => format!("pwa:manifest/icons/{size}px"),
            Self::Icon(index) => format!("pwa:manifest/icons/{index}"),
            Self::ServiceWorker => "pwa:service-worker".to_owned(),
            Self::Offline => "pwa:service-worker/offline".to_owned(),
            Self::Install(id) => format!("pwa:install/{}", cut(id, 64)),
            Self::Meta(name) => format!("pwa:head/{name}"),
        }
    }

    /// The panel's section showing the place.
    #[must_use]
    pub fn section(&self) -> &'static str {
        match self {
            Self::Manifest | Self::Field(_) | Self::Meta(_) => "manifest",
            Self::RequiredIcon(_) | Self::Icon(_) => "icons",
            Self::ServiceWorker | Self::Offline => "sw",
            Self::Install(_) => "install",
        }
    }

    /// The element id in the panel.
    #[must_use]
    pub fn anchor(&self) -> String {
        match self {
            Self::Icon(index) => format!("icon-{index}"),
            Self::Field(field) => format!("field-{field}"),
            other => other.section().to_owned(),
        }
    }
}

/// Where findings go: the rules the run asks for, about one page in one view.
pub struct Sink<'a> {
    /// The rules the run asks for (`check-context.rules`); every rule when empty.
    enabled: &'a [String],
    url: String,
    view: Option<ViewId>,
    pub findings: Vec<Finding>,
}

impl<'a> Sink<'a> {
    #[must_use]
    pub fn new(enabled: &'a [String], url: &str, view: Option<ViewId>) -> Self {
        Self {
            enabled,
            url: url.to_owned(),
            view,
            findings: Vec::new(),
        }
    }

    /// Whether `rule` runs at all.
    #[must_use]
    pub fn wants(&self, rule: &str) -> bool {
        self.enabled.is_empty() || self.enabled.iter().any(|id| id == rule)
    }

    /// A finding of `rule` at `place` with the message `plugin.pwa-check.findings.<rule>`;
    /// dropped when the run does not want the rule or the rule is not described.
    pub fn emit(&mut self, rule: &str, place: Place, params: &[(&str, String)]) {
        let Some(known) = RULES.iter().find(|known| known.id == rule) else {
            return;
        };
        if !self.wants(rule) {
            return;
        }
        let data =
            json!({ "panel": PANEL_ID, "section": place.section(), "anchor": place.anchor() });
        self.findings.push(Finding {
            rule: rule.to_owned(),
            severity: known.severity,
            message: crate::key(&format!("findings.{rule}")),
            params: params
                .iter()
                .map(|(key, value)| ((*key).to_owned(), cut(value, MAX_PARAM_CHARS)))
                .collect(),
            view: self.view,
            url: Some(self.url.clone()),
            selector: None,
            path: Some(place.path()),
            data: Some(data.to_string()),
        });
    }
}

#[cfg(test)]
#[path = "rules_tests.rs"]
mod tests;
