//! The component: `src/lib.rs` translated into the vocabulary of the contract
//! (`crates/rb-plugin-api/wit`), world `check-plugin`. Built only for `wasm32`; it holds no logic
//! of its own. The host it hands the native trait reaches only the `host` interface: the plugin
//! is granted no capability, and a component that calls no other interface imports none.
#![allow(
    unsafe_code,
    reason = "the canonical-ABI glue wit-bindgen generates is the only unsafe code here"
)]

wit_bindgen::generate!({
    path: "../../crates/rb-plugin-api/wit",
    world: "check-plugin",
});

use exports::rbrowser::plugin::check::{CheckContext, Guest, RuleInfo};
use rb_plugin_api::check::Check;
use rb_plugin_api::{
    Capability, Failure, FailureKind, Finding, HttpRequest, HttpResponse, PluginHost, Severity,
    ViewId, ViewInfo,
};
use rbrowser::plugin::host;
use rbrowser::plugin::types;

use crate::VisualRegression;

/// The host as the plugin sees it: logging and the clock; every capability is refused here
/// already, as the real host would refuse it.
struct GuestHost;

impl PluginHost for GuestHost {
    fn log(&mut self, level: &str, message: &str) {
        host::log(level, message);
    }

    fn now_unix_seconds(&mut self) -> u64 {
        host::now_unix_seconds()
    }

    fn setting(&mut self, _key: &str) -> Option<String> {
        None
    }

    fn views(&mut self) -> Result<Vec<ViewInfo>, Failure> {
        Err(Capability::Views.denied())
    }

    fn active_view(&mut self) -> Result<Option<ViewId>, Failure> {
        Err(Capability::Views.denied())
    }

    fn evaluate(&mut self, _view: ViewId, _script: &str) -> Result<String, Failure> {
        Err(Capability::Evaluate.denied())
    }

    fn cdp_send(&mut self, _view: ViewId, _method: &str, _params: &str) -> Result<String, Failure> {
        Err(Capability::Cdp.denied())
    }

    fn fetch(&mut self, _request: &HttpRequest) -> Result<HttpResponse, Failure> {
        Err(Capability::Http.denied())
    }

    fn project_read(&mut self, _path: &str) -> Result<Vec<u8>, Failure> {
        Err(Capability::ProjectRead.denied())
    }

    fn project_list(&mut self, _dir: &str) -> Result<Vec<String>, Failure> {
        Err(Capability::ProjectRead.denied())
    }

    fn out_write(&mut self, _path: &str, _bytes: &[u8]) -> Result<(), Failure> {
        Err(Capability::OutWrite.denied())
    }

    fn report_findings(&mut self, _items: &[Finding]) -> Result<(), Failure> {
        Err(Capability::Findings.denied())
    }

    fn sidecar_call(&mut self, _method: &str, _params: &str) -> Result<String, Failure> {
        Err(Capability::Sidecar.denied())
    }
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

fn severity(severity: Severity) -> types::Severity {
    match severity {
        Severity::Info => types::Severity::Info,
        Severity::Warning => types::Severity::Warning,
        Severity::Error => types::Severity::Error,
    }
}

struct Component;

impl Guest for Component {
    fn describe() -> Vec<RuleInfo> {
        VisualRegression
            .describe()
            .into_iter()
            .map(|rule| RuleInfo {
                id: rule.id,
                title: rule.title,
                severity: severity(rule.severity),
                shared_id: rule.shared_id,
                help_url: rule.help_url,
            })
            .collect()
    }

    fn run(ctx: CheckContext) -> Result<Vec<types::Finding>, types::Failure> {
        let ctx = rb_plugin_api::CheckContext {
            view: ctx.view,
            url: ctx.url,
            rules: ctx.rules,
            args: ctx.args,
        };
        let findings = VisualRegression.run(&mut GuestHost, &ctx).map_err(to_wit)?;
        Ok(findings
            .into_iter()
            .map(|finding| types::Finding {
                rule: finding.rule,
                severity: severity(finding.severity),
                message: finding.message,
                params: finding.params,
                view: finding.view,
                url: finding.url,
                selector: finding.selector,
                path: finding.path,
                data: finding.data,
            })
            .collect())
    }
}

export!(Component);
