//! Which views a call looks at, and each view's images measured at its own density: the run of
//! `checks_run` (one view or every view) and the measure the tool and the command share.

use rb_plugin_api::{CheckContext, Failure, FailureKind, Finding, PluginHost, ViewInfo, codes};
use serde_json::Value;

use crate::page::{self, PageImages};
use crate::rules;
use crate::settings::Settings;

/// One view and what its page answered.
#[derive(Clone, Debug)]
pub struct Measured {
    pub view: ViewInfo,
    pub page: Result<PageImages, Failure>,
}

/// `plugins.error.invalid` with `reason`.
#[must_use]
pub fn invalid(reason: &str) -> Failure {
    Failure::coded(FailureKind::BadData, codes::INVALID).with_param("reason", reason)
}

fn not_found(id: impl ToString) -> Failure {
    Failure::coded(FailureKind::NotFound, codes::NOT_FOUND).with_param("view", id.to_string())
}

/// The views `target` names: a view id for that one; none or `"all"` for every view.
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
                .and_then(|id| u32::try_from(id).ok())
                .ok_or_else(|| invalid("target"))?;
            views
                .into_iter()
                .find(|view| view.id == id)
                .map(|view| vec![view])
                .ok_or_else(|| not_found(id))
        }
        _ => Err(invalid("target")),
    }
}

/// Each of `views` measured at its own density, the progress reported view by view.
pub fn measure(
    host: &mut dyn PluginHost,
    views: Vec<ViewInfo>,
    settings: &Settings,
) -> Vec<Measured> {
    let total = u32::try_from(views.len()).unwrap_or(u32::MAX);
    let mut out = Vec::with_capacity(views.len());
    for (done, view) in views.into_iter().enumerate() {
        let page = page::collect(host, view.id, view.dpr, settings);
        out.push(Measured { view, page });
        host.report_progress(u32::try_from(done + 1).unwrap_or(u32::MAX), total);
    }
    out
}

/// The findings of `measured`, of the rules `enabled` names; a view that could not be measured
/// gives none.
#[must_use]
pub fn findings(measured: &[Measured], enabled: &[String]) -> Vec<Finding> {
    measured
        .iter()
        .filter_map(|one| {
            one.page
                .as_ref()
                .ok()
                .map(|page| rules::findings(one.view.id, page, enabled))
        })
        .flatten()
        .collect()
}

/// The check run of `checks_run`: the view the run names (every view when none), each measured
/// at its own density.
///
/// # Errors
///
/// The view the run names is gone or cannot be measured (a Firefox or WebKit view answers
/// `plugins.error.not_available`); in a run over every view such a view gives no findings.
pub fn check(host: &mut dyn PluginHost, ctx: &CheckContext) -> Result<Vec<Finding>, Failure> {
    let settings = Settings::read(host);
    let views = match ctx.view {
        Some(id) => targets(host, &Value::from(id))?,
        None => host.views()?,
    };
    let measured = measure(host, views, &settings);
    if ctx.view.is_some()
        && let Some(Measured {
            page: Err(failure), ..
        }) = measured.first()
    {
        return Err(failure.clone());
    }
    Ok(findings(&measured, &ctx.rules))
}

#[cfg(test)]
#[path = "run_tests.rs"]
mod tests;
