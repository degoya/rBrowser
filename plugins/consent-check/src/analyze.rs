//! A run's capture judged: per phase the requests to other sites grouped by host and kind
//! (hosts of `allowHosts` kept but marked), the cookies with their lifetime and whether they
//! count as essential, the storage keys; the differences between before consent and after
//! "Reject all"; and the findings - every request to another site and every cookie that is not
//! essential before consent (a warning), every request to another site and every new cookie
//! that is not essential after "Reject all" (an error).

use std::collections::BTreeSet;

use rb_plugin_api::Finding;
use serde_json::json;

use crate::NOTICE_TEXT;
use crate::capture::{Phase, Run};
use crate::cmp;
use crate::rules::{self, Place};
use crate::settings::Settings;
use crate::site::{self, Kind, Party};

/// Seconds of a day.
const DAY: f64 = 86_400.0;

/// The requests to one host of one kind.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostRow {
    pub host: String,
    pub kind: Kind,
    pub count: u32,
    /// The host is one of `allowHosts`.
    pub allowed: bool,
    /// The first request's URL, without query and fragment.
    pub example: String,
}

/// One cookie as judged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CookieRow {
    pub name: String,
    pub domain: String,
    /// Whole days it is kept (rounded up); `None` for a session cookie.
    pub days: Option<u64>,
    pub essential: bool,
    /// Its domain is another site's.
    pub third_party: bool,
}

/// One phase judged.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PhaseReport {
    pub hosts: Vec<HostRow>,
    /// Requests to the page's own site.
    pub first_party: u32,
    pub cookies: Vec<CookieRow>,
    pub storage: Vec<String>,
}

/// What changed after "Reject all": `host kind`, `name domain` and storage keys.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Diff {
    pub hosts_added: Vec<String>,
    pub hosts_removed: Vec<String>,
    pub cookies_added: Vec<String>,
    pub cookies_removed: Vec<String>,
    pub storage_added: Vec<String>,
    pub storage_removed: Vec<String>,
}

/// A run judged.
#[derive(Clone, Debug, PartialEq)]
pub struct Report {
    /// The page's site.
    pub site: String,
    pub before: PhaseReport,
    pub after: Option<PhaseReport>,
    pub diff: Option<Diff>,
    pub findings: Vec<Finding>,
}

fn days(expires: Option<f64>, now: f64) -> Option<u64> {
    let left = (expires? - now).max(0.0);
    // Whole days, rounded up: non-negative and far below u64's range.
    Some((left / DAY).ceil() as u64)
}

fn judge(phase: &Phase, page_site: &str, settings: &Settings, now: f64) -> PhaseReport {
    let mut report = PhaseReport {
        storage: phase.storage.clone(),
        ..PhaseReport::default()
    };
    for request in &phase.requests {
        let (host, allowed) = match site::party(&request.url, page_site, &settings.allow_hosts) {
            Party::First => {
                report.first_party += 1;
                continue;
            }
            Party::None => continue,
            Party::Third(host) => (host, false),
            Party::Allowed(host) => (host, true),
        };
        let kind = site::kind_of(&request.resource_type);
        match report
            .hosts
            .iter()
            .position(|row| row.host == host && row.kind == kind)
        {
            Some(at) => report.hosts[at].count += 1,
            None => report.hosts.push(HostRow {
                host,
                kind,
                count: 1,
                allowed,
                example: site::shown_url(&request.url),
            }),
        }
    }
    report.cookies = phase
        .cookies
        .iter()
        .map(|cookie| {
            let bare = cookie.domain.trim_start_matches('.');
            CookieRow {
                name: cookie.name.clone(),
                domain: cookie.domain.clone(),
                days: days(cookie.expires, now),
                essential: cmp::essential(&cookie.name, &settings.essential_cookies),
                third_party: site::site_of(bare) != page_site,
            }
        })
        .collect();
    report
}

fn host_key(row: &HostRow) -> String {
    format!("{} {}", row.host, row.kind.as_str())
}

fn cookie_key(row: &CookieRow) -> String {
    format!("{} {}", row.name, row.domain)
}

fn difference(a: &BTreeSet<String>, b: &BTreeSet<String>) -> Vec<String> {
    a.difference(b).cloned().collect()
}

fn diff(before: &PhaseReport, after: &PhaseReport) -> Diff {
    let hosts = |report: &PhaseReport| -> BTreeSet<String> {
        report
            .hosts
            .iter()
            .filter(|row| !row.allowed)
            .map(host_key)
            .collect()
    };
    let cookies = |report: &PhaseReport| -> BTreeSet<String> {
        report.cookies.iter().map(cookie_key).collect()
    };
    let storage =
        |report: &PhaseReport| -> BTreeSet<String> { report.storage.iter().cloned().collect() };
    let (h0, h1) = (hosts(before), hosts(after));
    let (c0, c1) = (cookies(before), cookies(after));
    let (s0, s1) = (storage(before), storage(after));
    Diff {
        hosts_added: difference(&h1, &h0),
        hosts_removed: difference(&h0, &h1),
        cookies_added: difference(&c1, &c0),
        cookies_removed: difference(&c0, &c1),
        storage_added: difference(&s1, &s0),
        storage_removed: difference(&s0, &s1),
    }
}

fn host_findings(
    rule: &str,
    phase: &str,
    report: &PhaseReport,
    place: &Place<'_>,
    out: &mut Vec<Finding>,
) {
    for row in report.hosts.iter().filter(|row| !row.allowed) {
        let kind = row.kind.as_str();
        out.push(rules::finding(
            rule,
            false,
            &[
                ("host", row.host.clone()),
                ("kind", kind.to_owned()),
                ("count", row.count.to_string()),
            ],
            place,
            format!("consent-check:{phase}/request/{}/{kind}", row.host),
            &json!({ "notice": NOTICE_TEXT, "phase": phase, "host": row.host, "kind": kind,
                     "count": row.count, "example": row.example }),
        ));
    }
}

fn cookie_findings<'a>(
    rule: &str,
    phase: &str,
    rows: impl Iterator<Item = &'a CookieRow>,
    place: &Place<'_>,
    out: &mut Vec<Finding>,
) {
    for row in rows.filter(|row| !row.essential) {
        let mut params = vec![("name", row.name.clone()), ("domain", row.domain.clone())];
        if let Some(days) = row.days {
            params.push(("days", days.to_string()));
        }
        out.push(rules::finding(
            rule,
            row.days.is_none(),
            &params,
            place,
            format!("consent-check:{phase}/cookie/{}/{}", row.domain, row.name),
            &json!({ "notice": NOTICE_TEXT, "phase": phase, "name": row.name,
                     "domain": row.domain, "days": row.days, "thirdParty": row.third_party }),
        ));
    }
}

/// `run` judged under `settings`; only the rules of `enabled` make findings (all when empty).
#[must_use]
pub fn analyze(run: &Run, settings: &Settings, enabled: &[String]) -> Report {
    let page_site = rb_util::url::host_of(&run.url)
        .map(|host| site::site_of(&host))
        .unwrap_or_default();
    let before = judge(&run.before, &page_site, settings, run.captured_at);
    let after = run
        .after_reject
        .as_ref()
        .map(|phase| judge(phase, &page_site, settings, run.captured_at));
    let place = Place {
        view: run.view,
        url: &run.url,
    };
    let mut all = Vec::new();
    host_findings(rules::REQUEST_BEFORE, "before", &before, &place, &mut all);
    cookie_findings(
        rules::COOKIE_BEFORE,
        "before",
        before.cookies.iter(),
        &place,
        &mut all,
    );
    let diff = after.as_ref().map(|after| diff(&before, after));
    if let Some(after) = &after {
        host_findings(rules::REQUEST_AFTER, "after", after, &place, &mut all);
        let known: BTreeSet<String> = before.cookies.iter().map(cookie_key).collect();
        let new = after
            .cookies
            .iter()
            .filter(|row| !known.contains(&cookie_key(row)));
        cookie_findings(rules::COOKIE_AFTER, "after", new, &place, &mut all);
    }
    let findings = all
        .into_iter()
        .filter(|finding| rules::enabled(&finding.rule, enabled))
        .collect();
    Report {
        site: page_site,
        before,
        after,
        diff,
        findings,
    }
}

#[cfg(test)]
#[path = "analyze_tests.rs"]
mod tests;
