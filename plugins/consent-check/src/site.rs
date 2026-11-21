//! Whose request it is: the site (registrable domain, eTLD+1) of a host, approximately - its
//! last two labels, three below a country's `co.`, `com.`, ... (`example.co.uk`), as the Fonts
//! check reckons it; the hosts of `consent-check.allowHosts` (the project's own CDNs count as
//! first party); the kind of a request (script, pixel, font, iframe, ...) from its CDP resource
//! type; and the URL as a finding shows it, without query and fragment (they may carry ids).

use rb_util::url::host_of;

/// Second-level labels under which a country's registrable domains sit (`example.co.uk`).
const SECOND_LEVEL: &[&str] = &["co", "com", "org", "net", "ac", "gov", "edu", "or", "ne"];
/// The most host rules taken from the settings.
pub const MAX_HOST_RULES: usize = 64;
/// The longest host name (DNS).
const MAX_HOST_CHARS: usize = 253;
/// The longest URL a finding or the answer shows, in characters.
pub const MAX_URL_CHARS: usize = 200;

/// The registrable domain of `host`, approximately. An address is itself.
#[must_use]
pub fn site_of(host: &str) -> String {
    let labels: Vec<&str> = host.split('.').collect();
    let numeric = labels
        .iter()
        .all(|label| label.chars().all(|c| c.is_ascii_digit()));
    if labels.len() <= 2 || numeric || host.starts_with('[') {
        return host.to_owned();
    }
    let n = labels.len();
    let take = if labels[n - 1].len() == 2 && SECOND_LEVEL.contains(&labels[n - 2]) {
        3
    } else {
        2
    };
    labels[n.saturating_sub(take)..].join(".")
}

/// One host of `allowHosts`: the host itself, or with `*.` the domain and its subdomains.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostRule {
    pub host: String,
    pub subdomains: bool,
}

fn host_rule(text: &str) -> Option<HostRule> {
    let text = text.trim().to_ascii_lowercase();
    let (host, subdomains) = match text.strip_prefix("*.") {
        Some(rest) => (rest.to_owned(), true),
        None => (text, false),
    };
    let valid = !host.is_empty()
        && host.len() <= MAX_HOST_CHARS
        && !host.starts_with('.')
        && host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-');
    valid.then_some(HostRule { host, subdomains })
}

/// The valid rules of `list`, at most [`MAX_HOST_RULES`]; a malformed entry is dropped.
#[must_use]
pub fn host_rules(list: &[String]) -> Vec<HostRule> {
    list.iter()
        .filter_map(|text| host_rule(text))
        .take(MAX_HOST_RULES)
        .collect()
}

/// Whether `host` is one of `rules`.
#[must_use]
pub fn allowed(host: &str, rules: &[HostRule]) -> bool {
    let host = host.to_ascii_lowercase();
    rules.iter().any(|rule| {
        host == rule.host
            || (rule.subdomains
                && host
                    .strip_suffix(rule.host.as_str())
                    .is_some_and(|head| head.ends_with('.')))
    })
}

/// Whose a request is, seen from a page on the site `page_site`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Party {
    /// The page's own site.
    First,
    /// Another site, this host.
    Third(String),
    /// Another site the settings count as the page's own, this host.
    Allowed(String),
    /// No web URL (`data:`, `blob:`, ...): no request to anyone.
    None,
}

/// Whose `url` is on a page of the site `page_site`.
#[must_use]
pub fn party(url: &str, page_site: &str, rules: &[HostRule]) -> Party {
    let Some(host) = host_of(url) else {
        return Party::None;
    };
    if site_of(&host) == page_site {
        Party::First
    } else if allowed(&host, rules) {
        Party::Allowed(host)
    } else {
        Party::Third(host)
    }
}

/// What a request loads, as the findings name it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    Script,
    Pixel,
    Font,
    Iframe,
    Style,
    Fetch,
    Media,
    Other,
}

impl Kind {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Script => "script",
            Self::Pixel => "pixel",
            Self::Font => "font",
            Self::Iframe => "iframe",
            Self::Style => "style",
            Self::Fetch => "fetch",
            Self::Media => "media",
            Self::Other => "other",
        }
    }
}

/// The kind of a request of CDP's `resourceType`. A `Document` of another site is a frame (the
/// page's own document is first party); an image of another site is counted as a pixel.
#[must_use]
pub fn kind_of(resource_type: &str) -> Kind {
    match resource_type {
        "Script" => Kind::Script,
        "Image" | "Ping" => Kind::Pixel,
        "Font" => Kind::Font,
        "Document" => Kind::Iframe,
        "Stylesheet" => Kind::Style,
        "XHR" | "Fetch" | "EventSource" | "WebSocket" => Kind::Fetch,
        "Media" => Kind::Media,
        _ => Kind::Other,
    }
}

/// `url` without query and fragment, at most [`MAX_URL_CHARS`] characters.
#[must_use]
pub fn shown_url(url: &str) -> String {
    let end = url.find(['?', '#']).unwrap_or(url.len());
    url[..end].chars().take(MAX_URL_CHARS).collect()
}

#[cfg(test)]
#[path = "site_tests.rs"]
mod tests;
