//! Which view a call looks at and the inspection of its page: the collector's document read into
//! the tree and held to the vocabulary; and the check run of `checks_run`, each distinct document
//! validated once with its findings copied to every other view showing the same document.

use rb_plugin_api::{CheckContext, Failure, FailureKind, Finding, PluginHost, ViewInfo, codes};
use schema_org::rich;
use schema_org::vocab::Vocabulary;
use serde_json::Value;

use crate::page::{self, Document, Page};
use crate::rules::{self, Scope};
use crate::settings::Settings;
use crate::validate::{self, Issue, Options};

/// One view's page, inspected.
#[derive(Clone, Debug)]
pub struct Inspected {
    pub view: ViewInfo,
    pub page: Page,
    pub issues: Vec<Issue>,
    /// The vocabulary's schema.org release.
    pub vocabulary: String,
}

impl Inspected {
    /// The findings the settings and `enabled` want.
    #[must_use]
    pub fn findings(&self, settings: &Settings, enabled: &[String]) -> Vec<Finding> {
        let scope = Scope {
            enabled,
            min: settings.min_severity,
            view: Some(self.view.id),
            url: &self.page.url,
        };
        rules::findings(&self.issues, &scope)
    }
}

/// `plugins.error.invalid` with `reason`.
#[must_use]
pub fn invalid(reason: &str) -> Failure {
    Failure::coded(FailureKind::BadData, codes::INVALID).with_param("reason", reason)
}

fn not_found(id: impl ToString) -> Failure {
    Failure::coded(FailureKind::NotFound, codes::NOT_FOUND).with_param("view", id.to_string())
}

/// The view `target` names: a view id for that one; none or `"all"` for the active view (the
/// first view when none is active).
///
/// # Errors
///
/// `plugins.error.not_found` for a view the window does not have (or no view at all),
/// `plugins.error.invalid` for any other target.
pub fn target(host: &mut dyn PluginHost, target: &Value) -> Result<ViewInfo, Failure> {
    let views = host.views()?;
    let id = match target {
        Value::Null => None,
        Value::String(all) if all == "all" => None,
        Value::Number(number) => Some(
            number
                .as_u64()
                .and_then(|id| u32::try_from(id).ok())
                .ok_or_else(|| invalid("target"))?,
        ),
        _ => return Err(invalid("target")),
    };
    let id = match id {
        Some(id) => Some(id),
        None => host.active_view()?,
    };
    match id {
        Some(id) => views
            .into_iter()
            .find(|view| view.id == id)
            .ok_or_else(|| not_found(id)),
        None => views.into_iter().next().ok_or_else(|| not_found("none")),
    }
}

/// The page `view` shows, read and validated.
///
/// # Errors
///
/// The collector's failure ([`page::collect`]).
pub fn inspect(
    host: &mut dyn PluginHost,
    view: &ViewInfo,
    settings: &Settings,
) -> Result<Inspected, Failure> {
    let page = page::collect(host, view.id)?;
    Ok(validated(view, page, settings))
}

/// `page`, the page `view` shows, held to the vocabulary.
fn validated(view: &ViewInfo, page: Page, settings: &Settings) -> Inspected {
    let vocab = Vocabulary::load();
    let options = Options {
        rich: settings.rich,
    };
    let issues = validate::validate(&page, &vocab, &rich::table(), options);
    Inspected {
        view: view.clone(),
        page,
        issues,
        vocabulary: vocab.version,
    }
}

/// The check run of `checks_run`: the view the run names (every view when none), each view's
/// document read, each distinct document (its URL and its rendered HTML) validated once - a
/// breakpoint that renders other structured data is checked on its own - and every view's
/// progress reported, read or not.
///
/// # Errors
///
/// The view the run names is gone or cannot be read; a view of all that cannot be read gives no
/// findings.
pub fn check(host: &mut dyn PluginHost, ctx: &CheckContext) -> Result<Vec<Finding>, Failure> {
    let settings = Settings::read(host);
    let all = host.views()?;
    let chosen: Vec<ViewInfo> = match ctx.view {
        Some(id) => vec![
            all.iter()
                .find(|view| view.id == id)
                .cloned()
                .ok_or_else(|| not_found(id))?,
        ],
        None => all,
    };
    let mut findings = Vec::new();
    // The documents validated so far, with their findings.
    let mut checked: Vec<(Document, Vec<Finding>)> = Vec::new();
    let total = u32::try_from(chosen.len()).unwrap_or(u32::MAX);
    for (done, view) in chosen.iter().enumerate() {
        match page::fetch(host, view.id) {
            Ok(document) => {
                let known = checked
                    .iter()
                    .find(|(seen, _)| *seen == document)
                    .map(|(_, found)| found.clone());
                let found = match known {
                    Some(found) => found,
                    None => {
                        let page = page::read(&document.url, &document.html);
                        let found =
                            validated(view, page, &settings).findings(&settings, &ctx.rules);
                        checked.push((document, found.clone()));
                        found
                    }
                };
                findings.extend(found.into_iter().map(|mut finding| {
                    finding.view = Some(view.id);
                    finding
                }));
            }
            Err(failure) if ctx.view.is_some() => return Err(failure),
            Err(_) => {}
        }
        host.report_progress(u32::try_from(done + 1).unwrap_or(u32::MAX), total);
    }
    Ok(findings)
}

#[cfg(test)]
#[path = "run_tests.rs"]
mod tests;
