//! The check of one page: what its view tells (the collector), the manifest it links, the
//! manifest's members, start URL and icons, the service worker, the offline probe, Chromium's
//! installability errors and the head's `theme-color` and `apple-touch-icon` - the findings and
//! the report the panel and MCP show.

use rb_plugin_api::{Finding, PluginHost, ViewId};
use serde::Serialize;

use crate::collect::PageState;
use crate::fetch::Budget;
use crate::rules::{META_APPLE_TOUCH_ICON, META_THEME_COLOR, Place, Sink};
use crate::settings::Settings;
use crate::url::Url;

pub mod icons;
pub mod install;
pub mod members;
pub mod worker;

use icons::IconReport;
use install::InstallReport;
use members::ManifestReport;
use worker::{OfflineReport, WorkerReport};

/// The check of one page, as the panel, MCP and the out file show it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageReport {
    /// The view that ran the check.
    pub view: ViewId,
    /// Every view showing the page.
    pub views: Vec<ViewId>,
    pub url: String,
    pub manifest: Option<ManifestReport>,
    pub icons: Vec<IconReport>,
    pub service_worker: WorkerReport,
    pub offline: OfflineReport,
    pub installability: InstallReport,
    pub theme_color: bool,
    pub apple_touch_icon: Option<String>,
}

/// What one check yields.
#[derive(Clone, Debug, Default)]
pub struct Checked {
    pub report: PageReport,
    pub findings: Vec<Finding>,
}

/// The check of the page `state` describes, shown by `view`, for the rules `enabled` (every rule
/// when empty).
pub fn check(
    host: &mut dyn PluginHost,
    settings: &Settings,
    view: ViewId,
    state: &PageState,
    enabled: &[String],
) -> Checked {
    let mut report = PageReport {
        view,
        views: vec![view],
        url: state.url.clone(),
        theme_color: state.theme_color,
        apple_touch_icon: state.apple_touch_icon.clone(),
        ..PageReport::default()
    };
    let Some(page) = Url::parse(&state.url) else {
        return Checked {
            report,
            findings: Vec::new(),
        };
    };
    let budget = Budget::new(host, settings.timeout_ms);
    let mut sink = Sink::new(enabled, &state.url, Some(view));
    let loaded = members::check(host, &budget, &page, state, &mut sink);
    report.manifest = loaded.report;
    if let Some(manifest) = &loaded.manifest {
        report.icons = icons::check(host, &budget, &page, manifest, &mut sink);
    }
    report.service_worker = worker::check(host, &budget, &page, state, &mut sink);
    let start_url = loaded.manifest.as_ref().map(|manifest| &manifest.start_url);
    report.offline = worker::offline(
        host,
        view,
        settings.offline,
        &report.service_worker,
        start_url,
        &mut sink,
    );
    report.installability = install::check(host, view, &mut sink);
    if !state.theme_color {
        sink.emit(META_THEME_COLOR, Place::Meta("theme-color"), &[]);
    }
    if state.apple_touch_icon.is_none() {
        sink.emit(META_APPLE_TOUCH_ICON, Place::Meta("apple-touch-icon"), &[]);
    }
    Checked {
        report,
        findings: sink.findings,
    }
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
