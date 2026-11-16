//! The component: `src/lib.rs` translated into the vocabulary of the contract
//! (`crates/rb-plugin-api/wit`). Built only for `wasm32`; it holds no logic of its own.
//!
//! The host it hands the native trait reaches `host`, `settings` and the six interfaces the
//! manifest grants - `views`, `evaluate`, `cdp`, `findings`, `project` (`project-read`) and
//! `out` (`out-write`); a component that calls no other interface imports none, so the rest
//! stay refused here.
#![allow(
    unsafe_code,
    reason = "the canonical-ABI glue wit-bindgen generates is the only unsafe code here"
)]

wit_bindgen::generate!({
    path: "../../crates/rb-plugin-api/wit",
    world: "feature-plugin",
});

use exports::rbrowser::plugin::feature::Guest;
use rb_plugin_api::feature::Feature;
use rb_plugin_api::{
    Capability, Failure, FailureKind, Finding, HttpRequest, HttpResponse, PluginHost, Severity,
    ViewId, ViewInfo,
};
use rbrowser::plugin::{cdp, evaluate, findings, host, out, project, settings, types, views};

use crate::Overlays;

fn severity(severity: Severity) -> types::Severity {
    match severity {
        Severity::Info => types::Severity::Info,
        Severity::Warning => types::Severity::Warning,
        Severity::Error => types::Severity::Error,
    }
}

fn finding_to_wit(finding: Finding) -> types::Finding {
    types::Finding {
        rule: finding.rule,
        severity: severity(finding.severity),
        message: finding.message,
        params: finding.params,
        view: finding.view,
        url: finding.url,
        selector: finding.selector,
        path: finding.path,
        data: finding.data,
    }
}

/// The host as the plugin sees it.
struct GuestHost;

fn from_wit(failure: types::Failure) -> Failure {
    let kind = match failure.kind {
        types::FailureKind::NotFound => FailureKind::NotFound,
        types::FailureKind::AccessDenied => FailureKind::AccessDenied,
        types::FailureKind::NotSupported => FailureKind::NotSupported,
        types::FailureKind::BadData => FailureKind::BadData,
        types::FailureKind::Cancelled => FailureKind::Cancelled,
        types::FailureKind::Io => FailureKind::Io,
        types::FailureKind::Limit => FailureKind::Limit,
        types::FailureKind::Other => FailureKind::Other,
    };
    let mut native = match failure.code {
        Some(code) => Failure::coded(kind, &code),
        None => Failure::new(kind),
    };
    for (key, value) in failure.params {
        native = native.with_param(&key, value);
    }
    native
}

fn to_wit(failure: Failure) -> types::Failure {
    let kind = match failure.kind {
        FailureKind::NotFound => types::FailureKind::NotFound,
        FailureKind::AccessDenied => types::FailureKind::AccessDenied,
        FailureKind::NotSupported => types::FailureKind::NotSupported,
        FailureKind::BadData => types::FailureKind::BadData,
        FailureKind::Cancelled => types::FailureKind::Cancelled,
        FailureKind::Io => types::FailureKind::Io,
        FailureKind::Limit => types::FailureKind::Limit,
        FailureKind::Other => types::FailureKind::Other,
    };
    types::Failure {
        kind,
        code: failure.code,
        params: failure.params.into_iter().collect(),
    }
}

impl PluginHost for GuestHost {
    fn log(&mut self, level: &str, message: &str) {
        host::log(level, message);
    }

    fn now_unix_seconds(&mut self) -> u64 {
        host::now_unix_seconds()
    }

    fn setting(&mut self, key: &str) -> Option<String> {
        settings::get(key)
    }

    fn views(&mut self) -> Result<Vec<ViewInfo>, Failure> {
        Ok(views::list()
            .into_iter()
            .map(|view| ViewInfo {
                id: view.id,
                label: view.label,
                breakpoint: view.breakpoint,
                engine: view.engine,
                width: view.width,
                height: view.height,
                dpr: view.dpr,
                url: view.url,
                title: view.title,
            })
            .collect())
    }

    fn active_view(&mut self) -> Result<Option<ViewId>, Failure> {
        Ok(views::active())
    }

    fn evaluate(&mut self, view: ViewId, script: &str) -> Result<String, Failure> {
        evaluate::evaluate(view, script).map_err(from_wit)
    }

    fn cdp_send(&mut self, view: ViewId, method: &str, params: &str) -> Result<String, Failure> {
        cdp::send(view, method, params).map_err(from_wit)
    }

    fn fetch(&mut self, _request: &HttpRequest) -> Result<HttpResponse, Failure> {
        Err(Capability::Http.denied())
    }

    fn project_read(&mut self, path: &str) -> Result<Vec<u8>, Failure> {
        project::read(path).map_err(from_wit)
    }

    fn project_list(&mut self, dir: &str) -> Result<Vec<String>, Failure> {
        project::list(dir).map_err(from_wit)
    }

    fn out_write(&mut self, path: &str, bytes: &[u8]) -> Result<(), Failure> {
        out::write(path, bytes).map_err(from_wit)
    }

    fn report_findings(&mut self, items: &[Finding]) -> Result<(), Failure> {
        let items: Vec<types::Finding> = items.iter().cloned().map(finding_to_wit).collect();
        findings::report(&items).map_err(from_wit)
    }

    fn sidecar_call(&mut self, _method: &str, _params: &str) -> Result<String, Failure> {
        Err(Capability::Sidecar.denied())
    }
}

struct Component;

impl Guest for Component {
    fn activate() -> Result<(), types::Failure> {
        Overlays.activate(&mut GuestHost).map_err(to_wit)
    }

    fn run_command(command: String, args: String) -> Result<String, types::Failure> {
        Overlays
            .run_command(&mut GuestHost, &command, &args)
            .map_err(to_wit)
    }

    fn call_tool(tool: String, args: String) -> Result<String, types::Failure> {
        Overlays
            .call_tool(&mut GuestHost, &tool, &args)
            .map_err(to_wit)
    }
}

export!(Component);
