//! One document's answer held to the rules: the headers of the main document and the cookies it
//! sets, as [`crate::rules`] names them. Pure: no host, so every rule is tested on its own.
//!
//! - **CSP** ([`crate::csp`]): none, or none that rules on scripts (`csp.missing`); a working
//!   `'unsafe-inline'`/`'unsafe-eval'` (`csp.unsafe`) or wildcard source (`csp.wildcard`); no
//!   `object-src` (nor `default-src`) and no `base-uri` (`csp.object_src`, `csp.base_uri`).
//! - **HSTS**: missing or unreadable (`hsts.missing`), `max-age` under `hstsMinAge`
//!   (`hsts.short`); never asked over `http:` (a browser ignores it there) or on a loopback host
//!   (`localhost`, `*.localhost`, `127.0.0.0/8`, `[::1]`).
//! - **Frames**: neither `X-Frame-Options` `DENY`/`SAMEORIGIN` nor CSP `frame-ancestors`.
//! - `X-Content-Type-Options: nosniff`, `Referrer-Policy` (missing, or `unsafe-url` as the last
//!   policy a browser knows), `Permissions-Policy`, `Access-Control-Allow-Origin: *` with
//!   credentials.
//! - **Cookies** ([`crate::cookies`]): one finding per cookie naming the missing attributes,
//!   another for a broken `__Host-`/`__Secure-` prefix. Never the value.

use rb_util::url::host_of;
use serde_json::json;

use crate::cookies::{self, Cookie};
use crate::csp::Csp;
use crate::headers::{Headers, shown};
use crate::rules::{self, Issue};
use crate::settings::Settings;

/// The longest header value a finding's details carry.
const MAX_DETAIL_CHARS: usize = 1024;
/// The most digits of a `max-age` read; more is taken as the largest number.
const MAX_AGE_DIGITS: usize = 19;

/// The policies `Referrer-Policy` knows (W3C Referrer Policy).
const REFERRER_POLICIES: [&str; 8] = [
    "no-referrer",
    "no-referrer-when-downgrade",
    "origin",
    "origin-when-cross-origin",
    "same-origin",
    "strict-origin",
    "strict-origin-when-cross-origin",
    "unsafe-url",
];

/// The answer audited: the document's final URL (its scheme decides HSTS and `Secure`), its
/// headers and the cookies of the page.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Document {
    pub url: String,
    /// The `http:` URL the `https:` page redirected to on the way ([`crate::fetch::Fetched`]).
    pub downgrade: Option<String>,
    pub headers: Headers,
    pub cookies: Vec<Cookie>,
}

/// Whether `url` is served by this device: no HSTS asked there.
fn loopback(url: &str) -> bool {
    host_of(url).is_some_and(|host| {
        host == "localhost"
            || host.ends_with(".localhost")
            || host == "[::1]"
            || host.starts_with("127.")
    })
}

fn https(url: &str) -> bool {
    crate::fetch::is_https(url)
}

/// The `max-age` of an HSTS value; `None` when it has none a browser reads.
#[must_use]
pub fn max_age(value: &str) -> Option<u64> {
    let directive = value.split(';').find_map(|directive| {
        let (name, age) = directive.split_once('=')?;
        name.trim().eq_ignore_ascii_case("max-age").then_some(age)
    })?;
    let age = directive.trim();
    let age = age
        .strip_prefix('"')
        .and_then(|age| age.strip_suffix('"'))
        .unwrap_or(age);
    if age.is_empty() || !age.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    if age.len() > MAX_AGE_DIGITS {
        return Some(u64::MAX);
    }
    age.parse().ok()
}

fn header_issue(rule: &'static str, header: &str) -> Issue {
    let mut issue = Issue::new(rule);
    issue.data = json!({ "header": header });
    issue
}

fn csp_issues(headers: &Headers, issues: &mut Vec<Issue>) {
    const HEADER: &str = "content-security-policy";
    let csp = Csp::parse(headers.all(HEADER));
    let value = headers
        .get(HEADER)
        .map(|value| shown(&value, MAX_DETAIL_CHARS));
    let with_value = |issue: Issue| {
        let mut issue = issue;
        issue.data = json!({ "header": HEADER, "value": value });
        issue
    };
    if csp.is_empty() {
        issues.push(header_issue(rules::CSP_MISSING, HEADER));
        return;
    }
    if !csp.restricts_scripts() {
        issues.push(with_value(
            Issue::new(rules::CSP_MISSING).variant("script_src"),
        ));
    }
    let keywords = csp.unsafe_keywords();
    if !keywords.is_empty() {
        issues.push(with_value(
            Issue::new(rules::CSP_UNSAFE).param("keywords", keywords.join(", ")),
        ));
    }
    let wildcards = csp.wildcards();
    if !wildcards.is_empty() {
        issues.push(with_value(
            Issue::new(rules::CSP_WILDCARD).param("sources", wildcards.join(" ")),
        ));
    }
    if !csp.restricts_objects() {
        issues.push(with_value(Issue::new(rules::CSP_OBJECT_SRC)));
    }
    if !csp.has_base_uri() {
        issues.push(with_value(Issue::new(rules::CSP_BASE_URI)));
    }
}

fn hsts_issues(doc: &Document, settings: &Settings, issues: &mut Vec<Issue>) {
    const HEADER: &str = "strict-transport-security";
    if let Some(plain) = &doc.downgrade
        && !loopback(plain)
    {
        // The page left HTTPS on the way: what the final answer sends protects nothing then.
        issues
            .push(header_issue(rules::REDIRECT_DOWNGRADE, "location").param("url", plain.clone()));
    }
    if !https(&doc.url) || loopback(&doc.url) {
        return;
    }
    // A browser reads the first header only.
    match doc.headers.all(HEADER).next().and_then(max_age) {
        None => issues.push(header_issue(rules::HSTS_MISSING, HEADER)),
        Some(age) if age < settings.hsts_min_age => issues.push(
            header_issue(rules::HSTS_SHORT, HEADER)
                .param("maxAge", age.to_string())
                .param("minAge", settings.hsts_min_age.to_string()),
        ),
        Some(_) => {}
    }
}

/// Whether `X-Frame-Options` keeps the page out of foreign frames: every value the same, and
/// `DENY` or `SAMEORIGIN`.
fn frame_options(headers: &Headers) -> bool {
    let Some(value) = headers.get("x-frame-options") else {
        return false;
    };
    let values: Vec<String> = value
        .split(',')
        .map(|part| part.trim().to_ascii_lowercase())
        .collect();
    values
        .first()
        .is_some_and(|first| first == "deny" || first == "sameorigin")
        && values.windows(2).all(|pair| pair[0] == pair[1])
}

fn other_issues(headers: &Headers, issues: &mut Vec<Issue>) {
    if !frame_options(headers)
        && !Csp::parse(headers.all("content-security-policy")).has_frame_ancestors()
    {
        issues.push(header_issue(rules::FRAME_MISSING, "x-frame-options"));
    }
    let nosniff = headers
        .get("x-content-type-options")
        .and_then(|value| value.split(',').next().map(str::to_owned))
        .is_some_and(|first| first.trim().eq_ignore_ascii_case("nosniff"));
    if !nosniff {
        issues.push(header_issue(
            rules::NOSNIFF_MISSING,
            "x-content-type-options",
        ));
    }
    let referrer = headers.get("referrer-policy").and_then(|value| {
        value
            .split(',')
            .map(|token| token.trim().to_ascii_lowercase())
            .rev()
            .find(|token| REFERRER_POLICIES.contains(&token.as_str()))
    });
    match referrer.as_deref() {
        None => issues.push(header_issue(rules::REFERRER_MISSING, "referrer-policy")),
        Some("unsafe-url") => issues.push(header_issue(rules::REFERRER_UNSAFE, "referrer-policy")),
        Some(_) => {}
    }
    let permissions = headers
        .get("permissions-policy")
        .is_some_and(|value| !value.trim().is_empty());
    if !permissions {
        issues.push(header_issue(
            rules::PERMISSIONS_MISSING,
            "permissions-policy",
        ));
    }
    let any_origin = headers
        .get("access-control-allow-origin")
        .is_some_and(|value| value.trim() == "*");
    let credentials = headers
        .get("access-control-allow-credentials")
        .is_some_and(|value| value.trim().eq_ignore_ascii_case("true"));
    if any_origin && credentials {
        issues.push(header_issue(
            rules::CORS_CREDENTIALS,
            "access-control-allow-credentials",
        ));
    }
}

fn cookie_issues(doc: &Document, issues: &mut Vec<Issue>) {
    let secure_page = https(&doc.url);
    for cookie in &doc.cookies {
        let found = cookies::audit(cookie, secure_page);
        let path = format!("cookie:{}", cookie.name);
        if !found.missing.is_empty() {
            let mut issue = Issue::new(rules::COOKIE_FLAGS)
                .param("cookie", cookie.name.clone())
                .param("missing", found.missing.join(", "));
            issue.path = Some(path.clone());
            issue.data = json!({ "cookie": cookie.name, "missing": found.missing });
            issues.push(issue);
        }
        if let Some(prefix) = found.prefix {
            let mut issue = Issue::new(rules::COOKIE_PREFIX)
                .param("cookie", cookie.name.clone())
                .param("prefix", prefix);
            issue.path = Some(path);
            issue.data = json!({ "cookie": cookie.name, "prefix": prefix });
            issues.push(issue);
        }
    }
}

/// Everything `doc` breaks, in the order of [`rules::RULES`] by header, then by cookie.
#[must_use]
pub fn audit(doc: &Document, settings: &Settings) -> Vec<Issue> {
    let mut issues = Vec::new();
    csp_issues(&doc.headers, &mut issues);
    hsts_issues(doc, settings, &mut issues);
    other_issues(&doc.headers, &mut issues);
    cookie_issues(doc, &mut issues);
    issues
}

#[cfg(test)]
#[path = "audit_tests.rs"]
mod tests;
