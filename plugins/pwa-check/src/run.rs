//! Which views a call covers and the check of their pages: each page once, by the first Chromium
//! view showing it - a PWA is the same in every breakpoint - with its findings copied to the
//! other views showing it, so the findings panel names every breakpoint. Firefox and WebKit
//! views have no service worker API to read over CDP and no installability verdict: skipped and
//! named.

use rb_plugin_api::{
    CheckContext, Failure, FailureKind, Finding, PluginHost, ViewId, ViewInfo, codes,
};
use serde::Serialize;
use serde_json::Value;

use crate::audit::{self, PageReport};
use crate::collect;
use crate::settings::Settings;

/// A view that gave no check, and why (a failure code).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Skipped {
    pub view: ViewId,
    pub reason: String,
}

/// The pages of one call.
#[derive(Clone, Debug, Default)]
pub struct Run {
    pub pages: Vec<PageReport>,
    /// Each page's findings once, in the view that checked it.
    pub findings: Vec<Finding>,
    pub skipped: Vec<Skipped>,
}

impl Run {
    /// The findings with a copy for every other view showing the same page.
    #[must_use]
    pub fn findings_per_view(&self) -> Vec<Finding> {
        let mut all = Vec::new();
        for finding in &self.findings {
            all.push(finding.clone());
            let page = self
                .pages
                .iter()
                .find(|page| Some(page.view) == finding.view);
            for other in page.map(|page| page.views.as_slice()).unwrap_or_default() {
                if Some(*other) != finding.view {
                    let mut copy = finding.clone();
                    copy.view = Some(*other);
                    all.push(copy);
                }
            }
        }
        all
    }
}

/// The views `target` names: none or `"all"` for every view, a view id for that one.
///
/// # Errors
///
/// `plugins.error.not_found` for a view the window does not have, `plugins.error.invalid` for
/// any other target.
pub fn targets(host: &mut dyn PluginHost, target: &Value) -> Result<Vec<ViewInfo>, Failure> {
    let views = host.views()?;
    match target {
        Value::Null => Ok(views),
        Value::String(all) if all == "all" => Ok(views),
        Value::Number(number) => {
            let id = number
                .as_u64()
                .and_then(|id| ViewId::try_from(id).ok())
                .ok_or_else(|| invalid("target"))?;
            views
                .into_iter()
                .find(|view| view.id == id)
                .map(|view| vec![view])
                .ok_or_else(|| {
                    Failure::coded(FailureKind::NotFound, codes::NOT_FOUND).with_param("view", id)
                })
        }
        _ => Err(invalid("target")),
    }
}

/// `plugins.error.invalid` with `reason`.
#[must_use]
pub fn invalid(reason: &str) -> Failure {
    Failure::coded(FailureKind::BadData, codes::INVALID).with_param("reason", reason)
}

/// The check of the pages `views` show, each URL once.
pub fn run(
    host: &mut dyn PluginHost,
    settings: &Settings,
    views: &[ViewInfo],
    enabled: &[String],
) -> Run {
    let mut run = Run::default();
    let chromium: Vec<&ViewInfo> = views
        .iter()
        .filter(|view| view.engine == "chromium")
        .collect();
    for view in views.iter().filter(|view| view.engine != "chromium") {
        run.skipped.push(Skipped {
            view: view.id,
            reason: codes::NOT_AVAILABLE.to_owned(),
        });
    }
    let total = u32::try_from(chromium.len()).unwrap_or(u32::MAX);
    for (done, view) in chromium.iter().enumerate() {
        if let Some(page) = run.pages.iter_mut().find(|page| page.url == view.url) {
            page.views.push(view.id);
            continue;
        }
        match collect::collect(host, view.id) {
            Ok(state) => {
                let checked = audit::check(host, settings, view.id, &state, enabled);
                let mut report = checked.report;
                // The view's own URL groups the views; the page's may differ by a fragment.
                report.url.clone_from(&view.url);
                run.pages.push(report);
                run.findings.extend(checked.findings);
            }
            Err(failure) => run.skipped.push(Skipped {
                view: view.id,
                reason: failure.code().unwrap_or(codes::HOST_FAILED).to_owned(),
            }),
        }
        host.report_progress(u32::try_from(done + 1).unwrap_or(u32::MAX), total);
    }
    run
}

/// The check run of `checks_run`: the view the run names (every view when none), its findings
/// in every view showing the page.
///
/// # Errors
///
/// The view the run names is gone; a Firefox or WebKit view named alone answers
/// `Failure::engine_unserved` (the runner shows it skipped), one of all gives no findings.
pub fn check(host: &mut dyn PluginHost, ctx: &CheckContext) -> Result<Vec<Finding>, Failure> {
    let settings = Settings::read(host);
    let all = host.views()?;
    let chosen: Vec<ViewInfo> = match ctx.view {
        Some(id) => {
            // Gone: not found; another engine: the runner shows it skipped, not "nothing
            // found" (RB-080-98).
            vec![ViewInfo::chromium_of(&all, id, "cdp")?]
        }
        None => all,
    };
    Ok(run(host, &settings, &chosen, &ctx.rules).findings_per_view())
}

#[cfg(test)]
#[path = "run_tests.rs"]
mod tests;
