//! Collecting the outlines of the views a call targets, and the check run of `checks_run`.

use rb_plugin_api::{
    CheckContext, Failure, FailureKind, Finding, PluginHost, ViewId, ViewInfo, codes,
};
use serde_json::Value;

use crate::links::{self, LinkResult};
use crate::page::{self, PageOutline};
use crate::rules;
use crate::settings::Settings;

/// The outlines of the views, and why the others gave none.
#[derive(Debug, Default)]
pub struct Collected {
    pub outlines: Vec<(ViewId, PageOutline)>,
    /// `(view, failure code)`: a Firefox or WebKit view (`not_available`), a view that went away.
    pub failures: Vec<(ViewId, String)>,
}

impl Collected {
    #[must_use]
    pub fn outline(&self, view: ViewId) -> Option<&PageOutline> {
        self.outlines
            .iter()
            .find(|(id, _)| *id == view)
            .map(|(_, outline)| outline)
    }
}

/// The views `target` names: `"all"` or none for every view, a view id for that one.
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
                .ok_or_else(|| Failure::coded(FailureKind::BadData, codes::INVALID))?;
            views
                .into_iter()
                .find(|view| view.id == id)
                .map(|view| vec![view])
                .ok_or_else(|| {
                    Failure::coded(FailureKind::NotFound, codes::NOT_FOUND).with_param("view", id)
                })
        }
        _ => {
            Err(Failure::coded(FailureKind::BadData, codes::INVALID).with_param("reason", "target"))
        }
    }
}

/// The outlines of `views` for `sections` (every page section when empty).
pub fn collect(
    host: &mut dyn PluginHost,
    views: &[ViewInfo],
    sections: &[String],
    settings: &Settings,
) -> Collected {
    let mut collected = Collected::default();
    for (done, view) in views.iter().enumerate() {
        match page::collect(host, view.id, sections, &settings.custom) {
            Ok(outline) => collected.outlines.push((view.id, outline)),
            Err(failure) => collected.failures.push((
                view.id,
                failure.code().unwrap_or(codes::HOST_FAILED).to_owned(),
            )),
        }
        let total = u32::try_from(views.len()).unwrap_or(u32::MAX);
        host.report_progress(u32::try_from(done + 1).unwrap_or(u32::MAX), total);
    }
    collected
}

/// The links of every outline of `collected`, in view order.
#[must_use]
pub fn hrefs(collected: &Collected) -> Vec<String> {
    collected
        .outlines
        .iter()
        .flat_map(|(_, outline)| outline.links.iter().map(|link| link.href.clone()))
        .collect()
}

/// The check run of one view (or of every view when the run names none): the findings of its
/// outline, and - when `outline.linkCheck.enabled` and the run wants `link-broken` - the broken
/// links. The links of a page are checked once: by the first view showing that URL, over the
/// links of every view showing it (a link only the desktop shows is checked too).
///
/// # Errors
///
/// The host's failure for the one view the run names; a Firefox or WebKit view named alone
/// answers `Failure::engine_unserved` (the runner shows it skipped), left out of a run over all.
pub fn check(host: &mut dyn PluginHost, ctx: &CheckContext) -> Result<Vec<Finding>, Failure> {
    let settings = Settings::read(host);
    let all = host.views()?;
    let chosen: Vec<ViewInfo> = match ctx.view {
        Some(id) => {
            // Gone: not found; another engine: the runner shows it skipped, not "nothing
            // found" (RB-080-98).
            vec![ViewInfo::chromium_of(&all, id, "evaluate")?]
        }
        None => all.clone(),
    };
    let wants_links = settings.link_check
        && (ctx.rules.is_empty() || ctx.rules.iter().any(|rule| rule == rules::LINK_BROKEN));
    let mut findings = Vec::new();
    for view in &chosen {
        let outline = match page::collect(host, view.id, &[], &settings.custom) {
            Ok(outline) => outline,
            Err(failure) if failure.code() == Some(codes::NOT_AVAILABLE) => continue,
            Err(failure) if ctx.view.is_some() => return Err(failure),
            Err(_) => continue,
        };
        let checked = if wants_links {
            lead_links(host, &settings, &all, view, &outline)
        } else {
            Vec::new()
        };
        findings.extend(rules::findings(
            &ctx.rules,
            view.id,
            &outline,
            &[],
            &checked,
        ));
    }
    Ok(findings)
}

/// The link check of `view`'s page when `view` leads it (the first Chromium view on that URL).
fn lead_links(
    host: &mut dyn PluginHost,
    settings: &Settings,
    all: &[ViewInfo],
    view: &ViewInfo,
    outline: &PageOutline,
) -> Vec<LinkResult> {
    let same: Vec<&ViewInfo> = all
        .iter()
        .filter(|other| other.engine == "chromium" && other.url == view.url)
        .collect();
    if same.first().is_some_and(|lead| lead.id != view.id) {
        return Vec::new();
    }
    let mut hrefs: Vec<String> = outline.links.iter().map(|link| link.href.clone()).collect();
    let links_only = ["links".to_owned()];
    for other in same.iter().filter(|other| other.id != view.id) {
        if let Ok(theirs) = page::collect(host, other.id, &links_only, &[]) {
            hrefs.extend(theirs.links.into_iter().map(|link| link.href));
        }
    }
    links::check(host, settings, &hrefs, Some(view.id))
}

#[cfg(test)]
#[path = "run_tests.rs"]
mod tests;
