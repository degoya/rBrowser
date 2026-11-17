//! A copy run (RB-040-05): per session of the [`Plan`], the cookies are read once through one
//! of its views, filtered, mapped per engine and set - once into Firefox's jar of the session,
//! once into each WebKit view's own context - and the views that got some load again. The
//! report counts; the cookies themselves go nowhere else.

use std::collections::BTreeMap;

use async_trait::async_trait;
use rb_bidi::storage::Cookie as BidiCookie;
use rb_config::EngineKind;
use rb_engine::ViewId;

use super::filter::Filter;
use super::mapping::{PlaywrightCookie, check, to_bidi, to_playwright};
use super::model::{ChromeCookie, CopyReport, SkipReason, ViewReport};
use super::plan::{Group, Plan};

/// What an engine took: set and refused.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Taken {
    /// Set.
    pub set: usize,
    /// Refused.
    pub refused: usize,
}

/// Where a copy reads and writes. Errors are texts for the person, never with a cookie in them.
#[async_trait]
pub trait Jars: Send + Sync {
    /// The cookies of `view`'s session, read through `view` (CDP `Network.getAllCookies`).
    async fn read(&self, view: ViewId) -> Result<Vec<ChromeCookie>, String>;
    /// Sets `cookies` in the running Firefox's jar of `session`.
    async fn set_firefox(&self, session: &str, cookies: &[BidiCookie]) -> Result<Taken, String>;
    /// Adds `cookies` to the WebKit view `view`.
    async fn set_webkit(&self, view: ViewId, cookies: &[PlaywrightCookie])
    -> Result<Taken, String>;
    /// Loads the page of the external view `view` again.
    async fn reload(&self, view: ViewId) -> Result<(), String>;
}

/// How a run filters and finishes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Options {
    /// Only the cookies of these sites (MCP's `domains`); before the domain filter.
    pub domains: Option<Vec<String>>,
    /// `engines.cookies.domainFilter` is `all`.
    pub all_domains: bool,
    /// The current environment's host, a site of the filter besides the views' own.
    pub environment_host: Option<String>,
    /// `engines.cookies.exclude`.
    pub exclude: Vec<String>,
    /// `engines.cookies.reload`.
    pub reload: bool,
    /// Now, in seconds since the epoch.
    pub now: u64,
}

/// Runs `plan` over `jars`.
pub async fn run(jars: &dyn Jars, plan: Plan, options: &Options) -> CopyReport {
    let mut views = plan.other_session;
    for group in &plan.groups {
        views.extend(run_group(jars, group, options).await);
    }
    views.sort_by_key(|view| view.view);
    CopyReport {
        copied: views.iter().map(|view| view.copied).sum(),
        views,
        message: None,
    }
}

async fn run_group(jars: &dyn Jars, group: &Group, options: &Options) -> Vec<ViewReport> {
    let blank = |view: ViewId, engine: EngineKind| ViewReport {
        view,
        engine,
        session: group.session.clone(),
        copied: 0,
        refused: 0,
        skipped: BTreeMap::new(),
        reloaded: false,
        error: None,
    };
    let targets: Vec<(ViewId, EngineKind)> = group
        .firefox
        .iter()
        .map(|view| (*view, EngineKind::Firefox))
        .chain(group.webkit.iter().map(|view| (*view, EngineKind::Webkit)))
        .collect();
    let cookies = match read(jars, group).await {
        Ok(cookies) => cookies,
        Err(error) => {
            return targets
                .into_iter()
                .map(|(view, engine)| ViewReport {
                    error: Some(error.clone()),
                    ..blank(view, engine)
                })
                .collect();
        }
    };
    let (chosen, skipped) = choose(&cookies, &filter(group, options), options.now);
    let mut reports = Vec::new();
    if !group.firefox.is_empty() {
        let bidi: Vec<BidiCookie> = chosen
            .iter()
            .filter_map(|cookie| to_bidi(cookie, options.now).ok())
            .collect();
        let taken = if bidi.is_empty() {
            Ok(Taken::default())
        } else {
            jars.set_firefox(&group.session, &bidi).await
        };
        for view in &group.firefox {
            reports.push(report(blank(*view, EngineKind::Firefox), &taken, &skipped));
        }
    }
    for view in &group.webkit {
        let playwright: Vec<PlaywrightCookie> = chosen
            .iter()
            .filter_map(|cookie| to_playwright(cookie, options.now).ok())
            .collect();
        let taken = if playwright.is_empty() {
            Ok(Taken::default())
        } else {
            jars.set_webkit(*view, &playwright).await
        };
        reports.push(report(blank(*view, EngineKind::Webkit), &taken, &skipped));
    }
    if options.reload {
        for report in reports.iter_mut().filter(|report| report.copied > 0) {
            match jars.reload(report.view).await {
                Ok(()) => report.reloaded = true,
                Err(error) => tracing::debug!(view = %report.view, %error, "not reloaded"),
            }
        }
    }
    reports
}

/// The session's cookies through the first of its views that answers.
async fn read(jars: &dyn Jars, group: &Group) -> Result<Vec<ChromeCookie>, String> {
    let mut first_error = None;
    for view in &group.readers {
        match jars.read(*view).await {
            Ok(cookies) => return Ok(cookies),
            Err(error) => {
                first_error.get_or_insert(error);
            }
        }
    }
    Err(rb_i18n::t_with(
        "cookies.error.unreadable",
        &[
            ("session", group.session.as_str()),
            ("error", first_error.as_deref().unwrap_or_default()),
        ],
    ))
}

/// The filter of `group`'s copy.
fn filter(group: &Group, options: &Options) -> Filter {
    let hosts = match (&options.domains, options.all_domains) {
        (Some(domains), _) => Some(domains.clone()),
        (None, true) => None,
        (None, false) => Some(
            group
                .hosts
                .iter()
                .cloned()
                .chain(options.environment_host.clone())
                .collect(),
        ),
    };
    Filter::new(hosts, &options.exclude)
}

/// The cookies to copy and how many were left out, by reason.
fn choose<'a>(
    cookies: &'a [ChromeCookie],
    filter: &Filter,
    now: u64,
) -> (Vec<&'a ChromeCookie>, BTreeMap<SkipReason, usize>) {
    let mut chosen = Vec::new();
    let mut skipped = BTreeMap::new();
    for cookie in cookies {
        match filter.check(cookie).and_then(|()| check(cookie, now)) {
            Ok(()) => chosen.push(cookie),
            Err(reason) => *skipped.entry(reason).or_insert(0) += 1,
        }
    }
    (chosen, skipped)
}

fn report(
    blank: ViewReport,
    taken: &Result<Taken, String>,
    skipped: &BTreeMap<SkipReason, usize>,
) -> ViewReport {
    match taken {
        Ok(taken) => ViewReport {
            copied: taken.set,
            refused: taken.refused,
            skipped: skipped.clone(),
            ..blank
        },
        Err(error) => ViewReport {
            error: Some(error.clone()),
            skipped: skipped.clone(),
            ..blank
        },
    }
}

#[cfg(test)]
#[path = "copy_tests.rs"]
mod tests;
