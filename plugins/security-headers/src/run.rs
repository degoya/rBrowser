//! The check run of `checks_run`: headers do not depend on the breakpoint, so each distinct URL
//! of the views the run names is fetched once and its findings name the URL, no view - the
//! findings run gives them the run's view and merges every view showing the page into one
//! finding.

use rb_plugin_api::{
    CheckContext, Failure, FailureKind, Finding, PluginHost, codes, rules as host_rules,
};

use crate::environments;
use crate::levels::RuleLevels;
use crate::page;
use crate::settings::Settings;

fn not_found(id: impl ToString) -> Failure {
    Failure::coded(FailureKind::NotFound, codes::NOT_FOUND).with_param("view", id.to_string())
}

/// The URLs the run checks: the URL it asks for, else the page of its view, else the distinct
/// pages of every view; web pages only.
fn urls(host: &mut dyn PluginHost, ctx: &CheckContext) -> Result<Vec<String>, Failure> {
    if let Some(url) = ctx
        .url
        .as_ref()
        .filter(|url| host_rules::check_http_url(url).is_ok())
    {
        return Ok(vec![url.clone()]);
    }
    let views = host.views()?;
    let mut urls: Vec<String> = Vec::new();
    match ctx.view {
        Some(id) => {
            let view = views
                .into_iter()
                .find(|view| view.id == id)
                .ok_or_else(|| not_found(id))?;
            urls.push(view.url);
        }
        None => {
            for view in views {
                let url = view.url.split('#').next().unwrap_or_default().to_owned();
                if !urls.contains(&url) {
                    urls.push(url);
                }
            }
        }
    }
    urls.retain(|url| host_rules::check_http_url(url).is_ok());
    Ok(urls)
}

/// The findings of the run.
///
/// # Errors
///
/// The view the run names is gone, or its page cannot be fetched; with every view, a page that
/// cannot be fetched gives no findings.
pub fn check(host: &mut dyn PluginHost, ctx: &CheckContext) -> Result<Vec<Finding>, Failure> {
    let settings = Settings::read(host);
    let environments = environments::read(host);
    let urls = urls(host, ctx)?;
    let single = ctx.view.is_some() || ctx.url.is_some();
    // The findings run applies the person's levels itself.
    let levels = RuleLevels::default();
    let total = u32::try_from(urls.len()).unwrap_or(u32::MAX);
    let mut findings = Vec::new();
    for (done, url) in urls.iter().enumerate() {
        match page::check_url(host, url, &environments, &settings, true) {
            Ok(checked) => findings.extend(checked.findings(&ctx.rules, &levels)),
            Err(failure) if single => return Err(failure),
            Err(_) => {}
        }
        host.report_progress(u32::try_from(done + 1).unwrap_or(u32::MAX), total);
    }
    Ok(findings)
}

#[cfg(test)]
#[path = "run_tests.rs"]
mod tests;
