//! Lighthouse per breakpoint (RB-060-07): the Node sidecar runs Lighthouse for each Chromium
//! breakpoint with that breakpoint's emulation, and its failed audits become findings that name
//! the breakpoints they failed in ("only on mobile").
//!
//! **How a run works.** `plan` turns the views into one target per breakpoint - size, device
//! pixel ratio, mobile and user agent from the view and the project's breakpoint, the URL of the
//! view - and skips Firefox and WebKit with the reason. For each target in turn, `sidecar`
//! starts a Lighthouse job (`lighthouse.start`), polls it until it ends and fetches the JSON
//! result (LHR) and the HTML report in slices: the sidecar runs Lighthouse in a headless browser
//! of its own (`sidecar/src/modules/lighthouse/`), so the views never navigate. `lhr` reads the
//! scores and the failed audits; `run` saves the reports and a summary under
//! `.rbrowser/out/lighthouse/<run>/` and answers the scores and the paths.
//!
//! **The history** (RB-091-02): `history` lists the runs on disk and compares two of one
//! breakpoint; `reports` answers the tool `reports` and the commands `open_report` and
//! `delete_run`, asking the host to open a report in its report window or to remove a run.
//!
//! **Which categories.** The project's `checks.lighthouse.categories` (`.rbrowser/config.json`,
//! read through `project-read`), else all four; a call's `categories` wins. Throttling is the
//! plugin's setting `lighthouse.throttling`.
//!
//! The logic is tested natively against a fake host; `guest.rs` translates it into the WIT
//! vocabulary and exists on `wasm32` alone.

#[cfg(target_arch = "wasm32")]
mod guest;

pub mod history;
pub mod lhr;
pub mod plan;
pub mod project;
pub mod reports;
pub mod rules;
pub mod run;
pub mod settings;
pub mod sidecar;
pub mod tool;

use rb_plugin_api::check::Check;
use rb_plugin_api::feature::Feature;
use rb_plugin_api::{CheckContext, Failure, FailureKind, Finding, PluginHost, RuleInfo, codes};

/// The plugin's id: the prefix of its keys and its folder below `.rbrowser/out/`.
pub const PLUGIN_ID: &str = "lighthouse";

/// `plugin.lighthouse.<group>.<name>`.
#[must_use]
pub fn key(group: &str, name: &str) -> String {
    format!("plugin.{PLUGIN_ID}.{group}.{name}")
}

/// The failure `plugin.lighthouse.errors.<name>`.
#[must_use]
pub fn error(kind: FailureKind, name: &str) -> Failure {
    Failure::coded(kind, &key("errors", name))
}

/// `plugin.lighthouse.errors.bad_args`.
#[must_use]
pub fn bad_args(reason: &str) -> Failure {
    error(FailureKind::BadData, "bad_args").with_param("reason", reason)
}

/// The check and the feature.
pub struct Lighthouse;

impl Check for Lighthouse {
    fn describe(&self) -> Vec<RuleInfo> {
        rules::describe()
    }

    fn run(
        &mut self,
        host: &mut dyn PluginHost,
        ctx: &CheckContext,
    ) -> Result<Vec<Finding>, Failure> {
        run::check(host, ctx)
    }
}

fn unknown(name: &str) -> Failure {
    Failure::coded(FailureKind::NotFound, codes::UNKNOWN_CONTRIBUTION).with_param("name", name)
}

impl Feature for Lighthouse {
    fn run_command(
        &mut self,
        host: &mut dyn PluginHost,
        command: &str,
        args: &str,
    ) -> Result<String, Failure> {
        match command {
            // RB-099-10: `run_visible`'s manifest scope is `visible`: the host feeds the breakpoints.
            "run" | "run_visible" => tool::run(host, args, true),
            "open_report" => reports::open_command(host, args),
            "delete_run" => reports::delete_command(host, args),
            other => Err(unknown(other)),
        }
    }

    fn call_tool(
        &mut self,
        host: &mut dyn PluginHost,
        tool_name: &str,
        args: &str,
    ) -> Result<String, Failure> {
        match tool_name {
            "run" => tool::run(host, args, false),
            "status" => tool::status(host),
            "cancel" => tool::cancel(host),
            "reports" => reports::tool(host, args),
            other => Err(unknown(other)),
        }
    }
}

#[cfg(test)]
mod fake;

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
