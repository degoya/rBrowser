//! The rules of the header check (the host shows them as `security-headers.<rule>`) and the
//! findings made of an audit's issues.
//!
//! A rule's title is `plugin.security-headers.rules.<id>`, a finding's message
//! `plugin.security-headers.findings.<id>`. Headers do not depend on the breakpoint: a finding
//! names the checked URL and no view, so every view showing the page is the same finding (the
//! findings run merges them). A cookie's finding has the path `cookie:<name>`, so two cookies
//! stay two findings. The rules Lighthouse audits too carry its audit id as their shared id
//! (`csp-xss`, `has-hsts`, `clickjacking-mitigation`): a Lighthouse finding of the same page
//! merges with them instead of standing twice.

use rb_plugin_api::{Finding, RuleInfo, Severity};
use serde_json::{Value, json};

use crate::headers::shown;
use crate::levels::RuleLevels;

/// The longest message parameter, in characters (the host takes 256).
pub const MAX_PARAM_CHARS: usize = 200;

pub const CSP_MISSING: &str = "csp.missing";
pub const CSP_UNSAFE: &str = "csp.unsafe";
pub const CSP_WILDCARD: &str = "csp.wildcard";
pub const CSP_OBJECT_SRC: &str = "csp.object_src";
pub const CSP_BASE_URI: &str = "csp.base_uri";
pub const HSTS_MISSING: &str = "hsts.missing";
pub const HSTS_SHORT: &str = "hsts.short";
pub const FRAME_MISSING: &str = "frame.missing";
pub const NOSNIFF_MISSING: &str = "nosniff.missing";
pub const REFERRER_MISSING: &str = "referrer.missing";
pub const REFERRER_UNSAFE: &str = "referrer.unsafe";
pub const PERMISSIONS_MISSING: &str = "permissions.missing";
pub const CORS_CREDENTIALS: &str = "cors.credentials";
pub const COOKIE_FLAGS: &str = "cookie.flags";
pub const COOKIE_PREFIX: &str = "cookie.prefix";
pub const REDIRECT_DOWNGRADE: &str = "redirect.downgrade";

const MDN: &str = "https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/";

/// One rule of `describe`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rule {
    pub id: &'static str,
    pub severity: Severity,
    /// Lighthouse's audit of the same thing.
    pub shared_id: Option<&'static str>,
    /// The header's page below MDN's reference.
    pub help: &'static str,
}

const fn rule(
    id: &'static str,
    severity: Severity,
    shared_id: Option<&'static str>,
    help: &'static str,
) -> Rule {
    Rule {
        id,
        severity,
        shared_id,
        help,
    }
}

use Severity::{Error, Info, Warning};

const CSP_HELP: &str = "Content-Security-Policy";

/// Every rule, in the order the answers list them.
pub const RULES: &[Rule] = &[
    rule(CSP_MISSING, Warning, Some("csp-xss"), CSP_HELP),
    rule(CSP_UNSAFE, Warning, Some("csp-xss"), CSP_HELP),
    rule(CSP_WILDCARD, Warning, None, CSP_HELP),
    rule(CSP_OBJECT_SRC, Info, None, CSP_HELP),
    rule(CSP_BASE_URI, Info, None, CSP_HELP),
    rule(
        HSTS_MISSING,
        Warning,
        Some("has-hsts"),
        "Strict-Transport-Security",
    ),
    rule(
        HSTS_SHORT,
        Warning,
        Some("has-hsts"),
        "Strict-Transport-Security",
    ),
    rule(
        FRAME_MISSING,
        Warning,
        Some("clickjacking-mitigation"),
        "X-Frame-Options",
    ),
    rule(NOSNIFF_MISSING, Warning, None, "X-Content-Type-Options"),
    rule(REFERRER_MISSING, Info, None, "Referrer-Policy"),
    rule(REFERRER_UNSAFE, Warning, None, "Referrer-Policy"),
    rule(PERMISSIONS_MISSING, Info, None, "Permissions-Policy"),
    rule(
        CORS_CREDENTIALS,
        Error,
        None,
        "Access-Control-Allow-Credentials",
    ),
    rule(COOKIE_FLAGS, Warning, None, "Set-Cookie"),
    rule(COOKIE_PREFIX, Error, None, "Set-Cookie"),
    rule(REDIRECT_DOWNGRADE, Error, None, "Strict-Transport-Security"),
];

/// The rule `id`.
#[must_use]
pub fn find(id: &str) -> Option<&'static Rule> {
    RULES.iter().find(|rule| rule.id == id)
}

/// The usual severity of `id` (a warning for an id this plugin does not have).
#[must_use]
pub fn severity_of(id: &str) -> Severity {
    find(id).map_or(Warning, |rule| rule.severity)
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
            help_url: Some(format!("{MDN}{}", rule.help)),
        })
        .collect()
}

/// One thing an audit found.
#[derive(Clone, Debug, PartialEq)]
pub struct Issue {
    pub rule: &'static str,
    /// The message's variant: `findings.<rule>_<variant>` instead of `findings.<rule>`.
    pub variant: Option<&'static str>,
    /// The message's parameters (each cut to [`MAX_PARAM_CHARS`] when made a finding).
    pub params: Vec<(&'static str, String)>,
    /// What tells two issues of one rule apart (`cookie:<name>`).
    pub path: Option<String>,
    /// Details for the findings panel and the answers.
    pub data: Value,
}

impl Issue {
    /// An issue of `rule` without parameters.
    #[must_use]
    pub fn new(rule: &'static str) -> Self {
        Self {
            rule,
            variant: None,
            params: Vec::new(),
            path: None,
            data: json!({}),
        }
    }

    /// The issue with the message variant `variant`.
    #[must_use]
    pub fn variant(mut self, variant: &'static str) -> Self {
        self.variant = Some(variant);
        self
    }

    /// The issue with the parameter `key`.
    #[must_use]
    pub fn param(mut self, key: &'static str, value: impl Into<String>) -> Self {
        self.params.push((key, value.into()));
        self
    }
}

/// What the findings of one checked URL carry.
#[derive(Clone, Copy, Debug)]
pub struct Scope<'a> {
    /// The rule ids the run asks for; empty for all.
    pub enabled: &'a [String],
    /// The person's levels of the rules (`checks.rules.security-headers.*`).
    pub levels: &'a RuleLevels,
    /// The URL checked.
    pub url: &'a str,
    /// The project environment the URL belongs to.
    pub environment: Option<&'a str>,
}

impl Scope<'_> {
    /// Whether `rule` is asked for.
    #[must_use]
    pub fn wants(&self, rule: &str) -> bool {
        self.enabled.is_empty() || self.enabled.iter().any(|id| id == rule)
    }
}

/// The finding of `issue` within `scope`; `None` when the rule is not asked for or off.
#[must_use]
pub fn finding(issue: &Issue, scope: &Scope<'_>) -> Option<Finding> {
    if !scope.wants(issue.rule) {
        return None;
    }
    let mut params: Vec<(String, String)> = issue
        .params
        .iter()
        .map(|(key, value)| ((*key).to_owned(), shown(value, MAX_PARAM_CHARS)))
        .collect();
    let mut data = issue.data.clone();
    if let Some(environment) = scope.environment {
        params.push((
            "environment".to_owned(),
            shown(environment, MAX_PARAM_CHARS),
        ));
        if let Some(fields) = data.as_object_mut() {
            fields.insert("environment".to_owned(), json!(environment));
        }
    }
    let finding = Finding {
        rule: issue.rule.to_owned(),
        severity: severity_of(issue.rule),
        message: match issue.variant {
            Some(variant) => crate::key(&format!("findings.{}_{variant}", issue.rule)),
            None => crate::key(&format!("findings.{}", issue.rule)),
        },
        params,
        view: None,
        url: Some(scope.url.to_owned()),
        selector: None,
        path: issue.path.clone(),
        data: Some(data.to_string()),
    };
    scope.levels.apply(finding)
}

/// The findings of `issues` within `scope`.
#[must_use]
pub fn findings(issues: &[Issue], scope: &Scope<'_>) -> Vec<Finding> {
    issues
        .iter()
        .filter_map(|issue| finding(issue, scope))
        .collect()
}

/// A finding as an answer lists it: rule, severity, message key, parameters, path, details.
#[must_use]
pub fn finding_json(finding: &Finding) -> Value {
    let params: serde_json::Map<String, Value> = finding
        .params
        .iter()
        .map(|(key, value)| (key.clone(), Value::String(value.clone())))
        .collect();
    json!({
        "rule": finding.rule,
        "severity": finding.severity.as_str(),
        "message": finding.message,
        "params": params,
        "path": finding.path,
        "data": finding
            .data
            .as_deref()
            .and_then(|data| serde_json::from_str::<Value>(data).ok()),
    })
}

#[cfg(test)]
#[path = "rules_tests.rs"]
mod tests;
