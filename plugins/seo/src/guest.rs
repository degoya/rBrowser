//! The component: `src/lib.rs` translated into the vocabulary of the contract
//! (`crates/rb-plugin-api/wit`). Built only for `wasm32`; it holds no logic of its own.
//!
//! The component targets a world of its own that includes `check-plugin` and `feature-plugin`
//! (`docs/plugins.md`, "Plugin kinds"): the check for `checks_run` and the findings panel, the
//! feature for the MCP tool `seo_check` and the command. The host it hands the native traits
//! reaches the interfaces the manifest grants - `settings`, `views`, `evaluate`, `http`, `out`
//! and `findings` - and refuses the rest here already, as the real host would.
#![allow(
    unsafe_code,
    reason = "the canonical-ABI glue wit-bindgen generates is the only unsafe code here"
)]

wit_bindgen::generate!({
    inline: "
        package rbrowser:seo;

        world plugin {
            include rbrowser:plugin/check-plugin@0.1.0;
            include rbrowser:plugin/feature-plugin@0.1.0;
        }
    ",
    path: "../../crates/rb-plugin-api/wit",
    world: "rbrowser:seo/plugin",
    // The interfaces come from another package (`rbrowser:plugin`): generate them here.
    generate_all,
});

use exports::rbrowser::plugin::check::{CheckContext, Guest as CheckGuest, RuleInfo};
use exports::rbrowser::plugin::feature::Guest as FeatureGuest;
use rb_plugin_api::check::Check;
use rb_plugin_api::feature::Feature;
use rb_plugin_api::{
    Capability, Failure, FailureKind, Finding, HttpRequest, HttpResponse, PluginHost, Severity,
    ViewId, ViewInfo,
};
use rbrowser::plugin::{evaluate, findings, host, http, out, settings, types, views};

use crate::Seo;

/// The host as the plugin sees it: the granted interfaces; `cdp`, `project` and `sidecar` are
/// not granted and refused here, so the component does not import them.
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
    Failure {
        kind,
        code: failure.code,
        params: failure.params.into_iter().collect(),
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

fn finding(finding: Finding) -> types::Finding {
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

impl PluginHost for GuestHost {
    fn log(&mut self, level: &str, message: &str) {
        host::log(level, message);
    }

    fn now_unix_seconds(&mut self) -> u64 {
        host::now_unix_seconds()
    }

    fn report_progress(&mut self, done: u32, total: u32) {
        host::report_progress(done, total);
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

    fn cdp_send(&mut self, _view: ViewId, _method: &str, _params: &str) -> Result<String, Failure> {
        Err(Capability::Cdp.denied())
    }

    fn fetch(&mut self, request: &HttpRequest) -> Result<HttpResponse, Failure> {
        http::fetch(
            &request.method,
            &request.url,
            &request.headers,
            &request.body,
        )
        .map(|response| HttpResponse {
            status: response.status,
            headers: response.headers,
            body: response.body,
        })
        .map_err(from_wit)
    }

    fn project_read(&mut self, _path: &str) -> Result<Vec<u8>, Failure> {
        Err(Capability::ProjectRead.denied())
    }

    fn project_list(&mut self, _dir: &str) -> Result<Vec<String>, Failure> {
        Err(Capability::ProjectRead.denied())
    }

    fn out_write(&mut self, path: &str, bytes: &[u8]) -> Result<(), Failure> {
        out::write(path, bytes).map_err(from_wit)
    }

    fn report_findings(&mut self, items: &[Finding]) -> Result<(), Failure> {
        let items: Vec<types::Finding> = items.iter().cloned().map(finding).collect();
        findings::report(&items).map_err(from_wit)
    }

    fn sidecar_call(&mut self, _method: &str, _params: &str) -> Result<String, Failure> {
        Err(Capability::Sidecar.denied())
    }
}

struct Component;

impl CheckGuest for Component {
    fn describe() -> Vec<RuleInfo> {
        Seo.describe()
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
        let findings = Seo.run(&mut GuestHost, &ctx).map_err(to_wit)?;
        Ok(findings.into_iter().map(finding).collect())
    }
}

impl FeatureGuest for Component {
    fn activate() -> Result<(), types::Failure> {
        Seo.activate(&mut GuestHost).map_err(to_wit)
    }

    fn run_command(command: String, args: String) -> Result<String, types::Failure> {
        Seo.run_command(&mut GuestHost, &command, &args)
            .map_err(to_wit)
    }

    fn call_tool(tool: String, args: String) -> Result<String, types::Failure> {
        Seo.call_tool(&mut GuestHost, &tool, &args).map_err(to_wit)
    }
}

export!(Component);
