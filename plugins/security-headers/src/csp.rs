//! The `Content-Security-Policy` of a document, read as far as the checks need it (CSP Level 3):
//! the policies of every header (a header may carry several, comma-separated), each a list of
//! directives (the first of a repeated name counts) with their sources in lower case.
//!
//! A browser enforces every policy, so a script runs only when each policy that rules on
//! scripts lets it: a keyword is reported only when every such policy allows it. `script-src`
//! falls back to `default-src`; `'unsafe-inline'` is ignored next to a nonce, a hash or
//! `'strict-dynamic'`, and host and scheme sources next to `'strict-dynamic'`.
//!
//! A foreign server's text: at most [`MAX_POLICIES`] policies, [`MAX_DIRECTIVES`] directives a
//! policy, [`MAX_SOURCES`] sources a directive and [`MAX_TOKEN_CHARS`] characters a source are
//! read, the rest dropped; nothing indexes into the text.

/// The most policies read of all headers together.
pub const MAX_POLICIES: usize = 16;
/// The most directives read of one policy.
pub const MAX_DIRECTIVES: usize = 64;
/// The most sources read of one directive.
pub const MAX_SOURCES: usize = 256;
/// The longest source kept, in characters.
pub const MAX_TOKEN_CHARS: usize = 512;
/// The longest directive name read.
const MAX_NAME_CHARS: usize = 64;

/// One policy: its directives in order, each name once.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Policy {
    directives: Vec<(String, Vec<String>)>,
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_NAME_CHARS
        && name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

impl Policy {
    /// The policy of `text` (one serialized policy, no comma).
    #[must_use]
    pub fn parse(text: &str) -> Self {
        let mut directives: Vec<(String, Vec<String>)> = Vec::new();
        for directive in text.split(';') {
            if directives.len() >= MAX_DIRECTIVES {
                break;
            }
            let mut tokens = directive.split_ascii_whitespace();
            let Some(name) = tokens.next() else {
                continue;
            };
            let name = name.to_ascii_lowercase();
            if !valid_name(&name) || directives.iter().any(|(seen, _)| *seen == name) {
                continue;
            }
            let sources = tokens
                .take(MAX_SOURCES)
                .map(|token| token.chars().take(MAX_TOKEN_CHARS).collect::<String>())
                .map(|token| token.to_lowercase())
                .collect();
            directives.push((name, sources));
        }
        Self { directives }
    }

    /// The sources of `name`; `None` when the policy has no such directive.
    #[must_use]
    pub fn directive(&self, name: &str) -> Option<&[String]> {
        self.directives
            .iter()
            .find(|(own, _)| own == name)
            .map(|(_, sources)| sources.as_slice())
    }

    /// How many directives were read.
    #[must_use]
    pub fn directive_count(&self) -> usize {
        self.directives.len()
    }

    /// The sources that rule on scripts: `script-src`, else `default-src`.
    fn script_sources(&self) -> Option<&[String]> {
        self.directive("script-src")
            .or_else(|| self.directive("default-src"))
    }
}

fn has(sources: &[String], keyword: &str) -> bool {
    sources.iter().any(|source| source == keyword)
}

fn strict_dynamic(sources: &[String]) -> bool {
    has(sources, "'strict-dynamic'")
}

/// Whether `sources` let inline scripts run.
fn allows_inline(sources: &[String]) -> bool {
    let disabled = sources.iter().any(|source| {
        source.starts_with("'nonce-")
            || source.starts_with("'sha256-")
            || source.starts_with("'sha384-")
            || source.starts_with("'sha512-")
    }) || strict_dynamic(sources);
    has(sources, "'unsafe-inline'") && !disabled
}

/// Whether `source` lets scripts of (nearly) any origin in.
fn is_wildcard(source: &str) -> bool {
    matches!(source, "*" | "http:" | "https:" | "data:" | "blob:")
        || source.split_once("://").is_some_and(|(_, host)| {
            host == "*" || host.starts_with("*/") || host.starts_with("*:")
        })
}

/// Every policy of a document.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Csp {
    policies: Vec<Policy>,
}

impl Csp {
    /// The policies of every `Content-Security-Policy` header value.
    #[must_use]
    pub fn parse<'a>(values: impl IntoIterator<Item = &'a str>) -> Self {
        let policies = values
            .into_iter()
            .flat_map(|value| value.split(','))
            .map(Policy::parse)
            .filter(|policy| policy.directive_count() > 0)
            .take(MAX_POLICIES)
            .collect();
        Self { policies }
    }

    /// The policies read.
    #[must_use]
    pub fn policies(&self) -> &[Policy] {
        &self.policies
    }

    /// Whether the document has no policy.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.policies.is_empty()
    }

    /// The script source lists of the policies that rule on scripts.
    fn script_lists(&self) -> impl Iterator<Item = &[String]> {
        self.policies.iter().filter_map(Policy::script_sources)
    }

    /// Whether any policy rules on scripts (`script-src` or `default-src`).
    #[must_use]
    pub fn restricts_scripts(&self) -> bool {
        self.script_lists().next().is_some()
    }

    /// Whether every policy that rules on scripts lets `allowed` through (and one does).
    fn all_scripts(&self, allowed: impl Fn(&[String]) -> bool) -> bool {
        self.restricts_scripts() && self.script_lists().all(allowed)
    }

    /// `'unsafe-inline'` and `'unsafe-eval'`, each when the policies let it work.
    #[must_use]
    pub fn unsafe_keywords(&self) -> Vec<&'static str> {
        let mut found = Vec::new();
        if self.all_scripts(allows_inline) {
            found.push("'unsafe-inline'");
        }
        if self.all_scripts(|sources| has(sources, "'unsafe-eval'")) {
            found.push("'unsafe-eval'");
        }
        found
    }

    /// The wildcard sources of the first script source list, when every policy that rules on
    /// scripts has one that works (none next to `'strict-dynamic'`).
    #[must_use]
    pub fn wildcards(&self) -> Vec<String> {
        let working = |sources: &[String]| -> Vec<String> {
            if strict_dynamic(sources) {
                return Vec::new();
            }
            sources
                .iter()
                .filter(|source| is_wildcard(source))
                .cloned()
                .collect()
        };
        if !self.all_scripts(|sources| !working(sources).is_empty()) {
            return Vec::new();
        }
        self.script_lists().next().map(working).unwrap_or_default()
    }

    /// Whether a policy rules on plugins' objects (`object-src`, else `default-src`).
    #[must_use]
    pub fn restricts_objects(&self) -> bool {
        self.policies.iter().any(|policy| {
            policy.directive("object-src").is_some() || policy.directive("default-src").is_some()
        })
    }

    /// Whether a policy sets `base-uri` (it has no fallback).
    #[must_use]
    pub fn has_base_uri(&self) -> bool {
        self.policies
            .iter()
            .any(|policy| policy.directive("base-uri").is_some())
    }

    /// Whether a policy sets `frame-ancestors`.
    #[must_use]
    pub fn has_frame_ancestors(&self) -> bool {
        self.policies
            .iter()
            .any(|policy| policy.directive("frame-ancestors").is_some())
    }
}

#[cfg(test)]
#[path = "csp_tests.rs"]
mod tests;
