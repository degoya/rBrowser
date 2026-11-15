//! The rules document a plugin granted `network-rules` answers the host's `@rules` request with
//! (RB-070-02), validated here before anything reaches CDP. A rule the host cannot hold is
//! rejected alone, with its id and a reason, and the others apply; a document that is no object
//! applies nothing.
//!
//! ```json
//! { "rules": [ { "id": "mocks/api.json#user-500", "origin": "project",
//!                "match": { "urlPattern": "*/api/user", "regex": false,
//!                           "method": "GET", "resourceType": "XHR" },
//!                "action": { "type": "mock", "status": 500,
//!                            "headers": [["content-type", "application/json"]],
//!                            "bodyBase64": "e30=", "delayMs": 800 } } ],
//!   "cacheDisabled": false, "clearSiteData": 0 }
//! ```
//!
//! `action.type` is `mock`, `block`, `rewrite`, `mapLocal` or `mapRemote` ([`super::action`]).
//! `origin` is `project` for rules made from project content (mock files, HAR imports, rules
//! added over MCP) - they apply only in a trusted workspace - or `settings` for the person's own
//! settings (the block list). The debugger's parts of the document - `breakpoints`,
//! `breakpointsEnabled`, `settings`, `asks` - are [`super::document`] (RB-080-37).

use regex::{Regex, RegexBuilder};
use serde_json::Value;

pub use super::action::Action;
use super::document::{self, Ask, Breakpoint, DebugSettings};

/// The most rules of one document.
pub const MAX_RULES: usize = 256;
/// The longest rule id.
pub const MAX_ID_CHARS: usize = 128;
/// The longest URL pattern or rewrite URL.
pub const MAX_PATTERN_CHARS: usize = 2048;
/// The most headers of one action.
pub const MAX_HEADERS: usize = 64;
/// The longest header value.
pub const MAX_HEADER_VALUE: usize = 8192;
/// The longest body of one mock, as base64 text (about 4 MiB of bytes).
pub const MAX_BODY_BASE64: usize = 6 * 1024 * 1024;
/// The most body text of a whole document, as base64.
pub const MAX_TOTAL_BODY_BASE64: usize = 24 * 1024 * 1024;
/// The longest delay of a mock.
pub const MAX_DELAY_MS: u64 = 60_000;
/// The largest compiled regular expression.
const REGEX_SIZE_LIMIT: usize = 1024 * 1024;

/// CDP's `Network.ResourceType` values.
pub const RESOURCE_TYPES: &[&str] = &[
    "Document",
    "Stylesheet",
    "Image",
    "Media",
    "Font",
    "Script",
    "TextTrack",
    "XHR",
    "Fetch",
    "Prefetch",
    "EventSource",
    "WebSocket",
    "Manifest",
    "SignedExchange",
    "Ping",
    "CSPViolationReport",
    "Preflight",
    "FedCM",
    "Other",
];

/// Where a rule comes from, which decides whether it needs workspace trust.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    /// Project content: applies only in a trusted workspace.
    Project,
    /// The person's own settings.
    Settings,
}

/// The URL a rule matches.
#[derive(Clone, Debug)]
pub enum UrlPattern {
    /// `*` any characters, `?` one, `\` escapes (CDP's `Fetch.RequestPattern` form).
    Glob(String),
    /// A regular expression searched in the whole URL.
    Regex(Regex),
}

/// What a rule matches.
#[derive(Clone, Debug)]
pub struct Matcher {
    pub url: UrlPattern,
    /// `GET`, `POST`, ...; any when none.
    pub method: Option<String>,
    /// One of [`RESOURCE_TYPES`]; any when none.
    pub resource_type: Option<String>,
}

/// One validated rule.
#[derive(Clone, Debug)]
pub struct Rule {
    pub id: String,
    pub origin: Origin,
    pub matcher: Matcher,
    pub action: Action,
}

/// A rule the host refused, and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rejected {
    pub id: String,
    pub reason: String,
}

/// A validated document.
#[derive(Clone, Debug, Default)]
pub struct Document {
    pub rules: Vec<Rule>,
    pub cache_disabled: bool,
    /// A counter the plugin raises to ask for one "clear site data".
    pub clear_site_data: u64,
    /// The breakpoints (RB-080-37).
    pub breakpoints: Vec<Breakpoint>,
    /// The breakpoints' switch; on when the document says nothing.
    pub breakpoints_enabled: bool,
    /// The debugger settings, when the document carries them.
    pub settings: Option<DebugSettings>,
    /// What the plugin asks the host to do.
    pub asks: Vec<Ask>,
}

/// `document` validated: the rules the host holds and the ones it rejected (breakpoints and
/// asks it rejected among them, by their ids).
///
/// # Errors
///
/// `document` is no JSON object, or its `rules` no list.
pub fn parse(document: &Value) -> Result<(Document, Vec<Rejected>), String> {
    let Value::Object(map) = document else {
        return Err("the rules document is no object".to_owned());
    };
    let rules = match map.get("rules") {
        None | Some(Value::Null) => &[][..],
        Some(Value::Array(rules)) => rules.as_slice(),
        Some(_) => return Err("rules is no list".to_owned()),
    };
    let (breakpoints, mut rejected) = document::parse_breakpoints(&document["breakpoints"]);
    let (asks, rejected_asks) = document::parse_asks(&document["asks"]);
    rejected.extend(rejected_asks);
    let mut parsed = Document {
        rules: Vec::new(),
        cache_disabled: map.get("cacheDisabled").and_then(Value::as_bool) == Some(true),
        clear_site_data: map
            .get("clearSiteData")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        breakpoints,
        breakpoints_enabled: map.get("breakpointsEnabled").and_then(Value::as_bool) != Some(false),
        settings: map
            .get("settings")
            .filter(|settings| settings.is_object())
            .map(document::parse_settings),
        asks,
    };
    let mut body_total = 0usize;
    for (index, raw) in rules.iter().enumerate() {
        let id = raw["id"]
            .as_str()
            .map_or_else(|| format!("#{index}"), |id| cut(id, MAX_ID_CHARS));
        if index >= MAX_RULES {
            rejected.push(reject(&id, "too many rules"));
            continue;
        }
        if parsed.rules.iter().any(|rule| rule.id == id) {
            rejected.push(reject(&id, "duplicate id"));
            continue;
        }
        match rule(raw) {
            Ok(rule) => {
                if let Action::Mock { body_base64, .. } = &rule.action {
                    body_total += body_base64.len();
                    if body_total > MAX_TOTAL_BODY_BASE64 {
                        rejected.push(reject(&id, "bodies too large together"));
                        continue;
                    }
                }
                parsed.rules.push(rule);
            }
            Err(reason) => rejected.push(reject(&id, reason)),
        }
    }
    Ok((parsed, rejected))
}

fn reject(id: &str, reason: &str) -> Rejected {
    Rejected {
        id: id.to_owned(),
        reason: reason.to_owned(),
    }
}

fn cut(text: &str, chars: usize) -> String {
    text.chars().take(chars).collect()
}

fn rule(raw: &Value) -> Result<Rule, &'static str> {
    let id = raw["id"].as_str().ok_or("no id")?;
    if id.is_empty()
        || id.chars().count() > MAX_ID_CHARS
        || !id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "._:/#@+-".contains(c))
    {
        return Err("bad id");
    }
    let origin = match raw["origin"].as_str() {
        Some("project") => Origin::Project,
        Some("settings") => Origin::Settings,
        _ => return Err("bad origin"),
    };
    Ok(Rule {
        id: id.to_owned(),
        origin,
        matcher: matcher(&raw["match"])?,
        action: super::action::parse(&raw["action"])?,
    })
}

/// A rule's or a breakpoint's `match`.
pub(super) fn matcher(raw: &Value) -> Result<Matcher, &'static str> {
    let pattern = raw["urlPattern"].as_str().ok_or("no urlPattern")?;
    if pattern.is_empty() || pattern.chars().count() > MAX_PATTERN_CHARS || has_control(pattern) {
        return Err("bad urlPattern");
    }
    let url = if raw["regex"].as_bool() == Some(true) {
        UrlPattern::Regex(
            RegexBuilder::new(pattern)
                .size_limit(REGEX_SIZE_LIMIT)
                .build()
                .map_err(|_| "bad regex")?,
        )
    } else {
        UrlPattern::Glob(pattern.to_owned())
    };
    let method = match &raw["method"] {
        Value::Null => None,
        Value::String(method)
            if (1..=16).contains(&method.len())
                && method.chars().all(|c| c.is_ascii_uppercase()) =>
        {
            Some(method.clone())
        }
        _ => return Err("bad method"),
    };
    let resource_type = match &raw["resourceType"] {
        Value::Null => None,
        Value::String(kind) if RESOURCE_TYPES.contains(&kind.as_str()) => Some(kind.clone()),
        _ => return Err("bad resourceType"),
    };
    Ok(Matcher {
        url,
        method,
        resource_type,
    })
}

fn has_control(text: &str) -> bool {
    text.chars().any(|c| c.is_control())
}

#[cfg(test)]
#[path = "rule_tests.rs"]
mod tests;
