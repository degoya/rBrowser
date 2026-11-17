//! The component: `src/lib.rs` translated into the vocabulary of the contract
//! (`crates/rb-plugin-api/wit`). Built only for `wasm32`; it holds no logic of its own.
//!
//! The component targets a world of its own that includes `analyzer-plugin` and
//! `feature-plugin` (`docs/plugins.md`, "Plugin kinds"). The host it hands the native traits
//! reaches only the `host` interface: the plugin is granted no capability, and a component
//! that calls no other interface imports none.
#![allow(
    unsafe_code,
    reason = "the canonical-ABI glue wit-bindgen generates is the only unsafe code here"
)]

wit_bindgen::generate!({
    inline: "
        package rbrowser:breakpoint-analyzer;

        world plugin {
            include rbrowser:plugin/analyzer-plugin@0.1.0;
            include rbrowser:plugin/feature-plugin@0.1.0;
        }
    ",
    path: "../../crates/rb-plugin-api/wit",
    world: "rbrowser:breakpoint-analyzer/plugin",
    // The interfaces come from another package (`rbrowser:plugin`): generate them here.
    generate_all,
});

use exports::rbrowser::plugin::analyzer::{Guest as AnalyzerGuest, Proposal};
use exports::rbrowser::plugin::feature::Guest as FeatureGuest;
use rb_plugin_api::analyzer::Analyzer;
use rb_plugin_api::feature::Feature;
use rb_plugin_api::{
    Capability, Failure, FailureKind, Finding, HttpRequest, HttpResponse, PluginHost, ViewId,
    ViewInfo,
};
use rbrowser::plugin::host;
use rbrowser::plugin::types;

use crate::BreakpointAnalyzer;

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

struct Component;

impl AnalyzerGuest for Component {
    fn analyze(args: String) -> Result<Vec<Proposal>, types::Failure> {
        let proposals = BreakpointAnalyzer
            .analyze(&mut GuestHost, &args)
            .map_err(to_wit)?;
        Ok(proposals
            .into_iter()
            .map(|proposal| Proposal {
                pointer: proposal.pointer,
                value: proposal.value,
                reason: proposal.reason,
                params: proposal.params,
            })
            .collect())
    }
}

impl FeatureGuest for Component {
    fn activate() -> Result<(), types::Failure> {
        BreakpointAnalyzer.activate(&mut GuestHost).map_err(to_wit)
    }

    fn run_command(command: String, args: String) -> Result<String, types::Failure> {
        BreakpointAnalyzer
            .run_command(&mut GuestHost, &command, &args)
            .map_err(to_wit)
    }

    fn call_tool(tool: String, args: String) -> Result<String, types::Failure> {
        BreakpointAnalyzer
            .call_tool(&mut GuestHost, &tool, &args)
            .map_err(to_wit)
    }
}

export!(Component);
