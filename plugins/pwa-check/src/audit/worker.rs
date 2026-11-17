//! The service worker: a secure context, a registration, an active worker that controls the
//! page, a `fetch` handler in its script (read over `http` from the page's origin), and the
//! offline probe of the start URL.

use rb_plugin_api::{PluginHost, ViewId};
use serde::Serialize;

use crate::collect::{self, PageState};
use crate::fetch::{self, ACCEPT_SCRIPT, Budget, Fetched};
use crate::rules::{
    OFFLINE_NOT_CACHED, Place, SW_FETCH_HANDLER, SW_INSECURE, SW_MISSING, SW_NOT_ACTIVE,
    SW_NOT_CONTROLLING, Sink,
};
use crate::url::Url;

/// The service worker as the report shows it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkerReport {
    pub secure: bool,
    pub supported: bool,
    pub registered: bool,
    pub active: bool,
    pub controlled: bool,
    pub scope: Option<String>,
    pub script: Option<String>,
    pub state: Option<String>,
    /// Whether the script handles `fetch`; `None` when it could not be told (not read, or it
    /// imports other scripts).
    pub fetch_handler: Option<bool>,
}

/// The offline test as the report shows it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OfflineReport {
    pub tested: bool,
    pub url: Option<String>,
    /// Whether the Cache Storage holds the start URL; `None` when it could not be told.
    pub cached: Option<bool>,
}

/// Whether `script` registers a `fetch` handler: `addEventListener('fetch', ...)`, `onfetch`,
/// or Workbox routing; `None` when it imports other scripts that may hold one.
#[must_use]
pub fn fetch_handler(script: &str) -> Option<bool> {
    let compact: String = script.chars().filter(|c| !c.is_whitespace()).collect();
    let handles = ["'fetch'", "\"fetch\"", "`fetch`"]
        .iter()
        .any(|name| compact.contains(&format!("addEventListener({name}")))
        || compact.contains("onfetch=")
        || compact.contains("registerRoute(")
        || compact.contains("precacheAndRoute(");
    if handles {
        Some(true)
    } else if compact.contains("importScripts(")
        || compact.contains("import(")
        || script
            .lines()
            .any(|line| line.trim_start().starts_with("import "))
    {
        None
    } else {
        Some(false)
    }
}

/// The service worker rules of the page `state` describes.
pub fn check(
    host: &mut dyn PluginHost,
    budget: &Budget,
    page: &Url,
    state: &PageState,
    sink: &mut Sink<'_>,
) -> WorkerReport {
    let worker = &state.sw;
    let mut report = WorkerReport {
        secure: state.secure,
        supported: worker.supported,
        controlled: worker.controlled,
        ..WorkerReport::default()
    };
    if !state.secure {
        sink.emit(SW_INSECURE, Place::ServiceWorker, &[]);
        return report;
    }
    let Some(registration) = &worker.registration else {
        sink.emit(SW_MISSING, Place::ServiceWorker, &[]);
        return report;
    };
    report.registered = true;
    report.active = registration.active;
    report.scope.clone_from(&registration.scope);
    report.script.clone_from(&registration.script);
    report.state.clone_from(&registration.state);
    let state_text = registration.state.clone().unwrap_or_else(|| "-".to_owned());
    if !registration.active {
        sink.emit(
            SW_NOT_ACTIVE,
            Place::ServiceWorker,
            &[("state", state_text)],
        );
    } else if !worker.controlled {
        sink.emit(SW_NOT_CONTROLLING, Place::ServiceWorker, &[]);
    }
    let script = registration.script.as_deref().and_then(Url::parse);
    if let Some(script) = script
        && sink.wants(SW_FETCH_HANDLER)
        && let Fetched::Ok(response) = fetch::get(host, budget, page, &script, ACCEPT_SCRIPT)
        && response.status == 200
    {
        report.fetch_handler = fetch_handler(&String::from_utf8_lossy(&response.body));
        if report.fetch_handler == Some(false) {
            sink.emit(
                SW_FETCH_HANDLER,
                Place::ServiceWorker,
                &[("script", script.to_string())],
            );
        }
    }
    report
}

/// The offline test: with an active service worker, whether its caches hold `start_url`.
pub fn offline(
    host: &mut dyn PluginHost,
    view: ViewId,
    enabled: bool,
    worker: &WorkerReport,
    start_url: Option<&Url>,
    sink: &mut Sink<'_>,
) -> OfflineReport {
    let Some(start_url) = start_url else {
        return OfflineReport::default();
    };
    if !enabled || !worker.active || !sink.wants(OFFLINE_NOT_CACHED) {
        return OfflineReport::default();
    }
    let cached = collect::cached(host, view, &start_url.to_string());
    if cached == Some(false) {
        sink.emit(
            OFFLINE_NOT_CACHED,
            Place::Offline,
            &[("url", start_url.to_string())],
        );
    }
    OfflineReport {
        tested: true,
        url: Some(start_url.to_string()),
        cached,
    }
}

#[cfg(test)]
#[path = "worker_tests.rs"]
mod tests;
