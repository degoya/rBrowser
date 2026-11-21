//! rBrowser's first-party width sweep (RB-093-01, `docs/plugins.md`): one view driven through a
//! range of widths - by default 320 to 1920 px in steps of 10, then pixel by pixel around every
//! change - and the layout breaks between the breakpoints reported as width ranges ("700-740
//! px: horizontal overflow caused by `.hero img`").
//!
//! - **The width** is the host's width lease (`Emulation.setDeviceMetricsOverride` with only a
//!   `width`, which the app turns into the view's own metrics at that width and takes the view
//!   out of sync; `Emulation.clearDeviceMetricsOverride` gives both back). The host also gives
//!   them back when the plugin's call ends however it ends, so a trap or a cancelled run never
//!   leaves a view swept.
//! - **The detection** (`src/detect.js`, built from `web/`) runs through `evaluate` in the
//!   plugin's isolated world at each width and only reads: horizontal overflow of the document
//!   (without the `100vw` false report), text cut off by its box, visible siblings overlapping.
//!   It also notices Escape pressed in the view, which ends the sweep.
//! - **Cancel**: the command and tool `cancel`, a call of its own beside the sweep (two calls at
//!   once, `limits.max_concurrent`), ends every lease of the plugin; the sweep's next width is
//!   refused as cancelled and it stops.
//! - **Show**: the command and tool `show` holds one width - a finding's, from its "Show at N px"
//!   in the findings panel - through the same lease until Escape, a cancel or `showSeconds`
//!   ([`show`]).
//!
//! The findings go to the findings panel (`findings.report`), one per issue and width range; the
//! check world describes their rules for `checks.rules`. A check run of every plugin does not
//! sweep: it would resize every view ([`WidthSweep::run`]).

use rb_plugin_api::check::Check;
use rb_plugin_api::feature::Feature;
use rb_plugin_api::{CheckContext, Failure, FailureKind, Finding, PluginHost, RuleInfo, codes};

pub mod findings;
pub mod plan;
pub mod probe;
pub mod settings;
pub mod show;
pub mod sweep;
pub mod tool;

#[cfg(test)]
mod fake;

#[cfg(target_arch = "wasm32")]
mod guest;

/// The plugin's id.
pub const PLUGIN_ID: &str = "width-sweep";

/// The i18n key `plugin.width-sweep.<name>`.
#[must_use]
pub fn key(name: &str) -> String {
    format!("plugin.{PLUGIN_ID}.{name}")
}

/// The plugin: no state between calls (a store lives for one call).
#[derive(Clone, Copy, Debug, Default)]
pub struct WidthSweep;

impl Check for WidthSweep {
    fn describe(&self) -> Vec<RuleInfo> {
        findings::describe()
    }

    /// Nothing: a sweep changes the view's width for a minute, so it runs only when asked for
    /// (the command `run`, MCP `sweep_run`), never as part of a check run of every plugin.
    fn run(
        &mut self,
        _host: &mut dyn PluginHost,
        _ctx: &CheckContext,
    ) -> Result<Vec<Finding>, Failure> {
        Ok(Vec::new())
    }
}

impl Feature for WidthSweep {
    fn run_command(
        &mut self,
        host: &mut dyn PluginHost,
        command: &str,
        args: &str,
    ) -> Result<String, Failure> {
        self.call_tool(host, command, args)
    }

    fn call_tool(
        &mut self,
        host: &mut dyn PluginHost,
        tool: &str,
        args: &str,
    ) -> Result<String, Failure> {
        let args = tool::parse(args)?;
        let answer = match tool {
            "run" => tool::run(host, &args)?,
            "cancel" => tool::cancel(host)?,
            "show" => show::show(host, &args)?,
            _ => return Err(unknown(tool)),
        };
        Ok(answer.to_string())
    }
}

fn unknown(name: &str) -> Failure {
    Failure::coded(FailureKind::NotFound, codes::UNKNOWN_CONTRIBUTION).with_param("name", name)
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
