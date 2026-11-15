//! The accessibility check (RB-060-06): axe-core in every view at once, its violations as
//! findings, so the findings panel shows in which breakpoints a problem occurs ("only in mobile,
//! tablet").
//!
//! **How a run works.** The component embeds axe-core (the version `runner/package.json` pins)
//! and the runner (`runner/`); per view it sends one script to the view's isolated world through
//! `evaluate` (`script`), which loads axe, runs the selected rules with `axe.run` and removes axe
//! again. axe never reaches the page's main world, and a page's CSP does not apply to the
//! isolated world. The answer - violations with each element's selector and path - becomes
//! findings (`answer`); the check runner merges them across views and with the seo plugin's
//! where both share an id (`rules::SHARED_IDS`).
//!
//! **Which rules.** The setting `a11y-axe.tags` (`wcag22aa` by default; a WCAG level includes
//! the lower ones), single rules switched by `a11y-axe.rules`, and of these the rules the person
//! left on in `checks.rules` (`check-context.rules`). `rules::select` decides; the runner never
//! widens it.
//!
//! The logic is tested natively against a fake host; `guest.rs` translates it into the WIT
//! vocabulary and exists on `wasm32` alone.

#[cfg(target_arch = "wasm32")]
mod guest;

pub mod answer;
pub mod rules;
pub mod script;
pub mod settings;
pub mod tool;

use rb_plugin_api::check::Check;
use rb_plugin_api::feature::Feature;
use rb_plugin_api::{
    CheckContext, Failure, FailureKind, Finding, PluginHost, RuleInfo, ViewId, codes,
};
use serde::Deserialize;

use crate::answer::Answer;
use crate::script::RunOptions;
use crate::settings::Settings;

/// The plugin's id: the prefix of its keys.
pub const PLUGIN_ID: &str = "a11y-axe";

/// `plugin.a11y-axe.<group>.<name>`.
#[must_use]
pub fn key(group: &str, name: &str) -> String {
    format!("plugin.{PLUGIN_ID}.{group}.{name}")
}

/// `plugin.a11y-axe.errors.bad_args`.
pub fn bad_args(reason: &str) -> Failure {
    Failure::coded(FailureKind::BadData, &key("errors", "bad_args")).with_param("reason", reason)
}

/// What a run asks axe for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    pub options: RunOptions,
}

impl Plan {
    /// The plan of a run under `settings`, with `tags` instead of the setting's when given, of
    /// the rules in `enabled`.
    #[must_use]
    pub fn new(settings: &Settings, tags: Option<&[String]>, enabled: &[String]) -> Self {
        let tags = tags.unwrap_or(&settings.tags);
        let rules = rules::select(tags, &settings.rules, enabled);
        Self {
            options: RunOptions::new(
                rules,
                &settings.include,
                &settings.exclude,
                settings.report_incomplete,
            ),
        }
    }

    /// The rule ids it runs.
    #[must_use]
    pub fn rules(&self) -> &[String] {
        &self.options.rules
    }

    /// Runs the plan in view `view`: the answer and its findings.
    ///
    /// # Errors
    ///
    /// `evaluate` failed, the answer does not read, or axe failed in the view.
    pub fn run_view(
        &self,
        host: &mut dyn PluginHost,
        view: ViewId,
    ) -> Result<(Answer, Vec<Finding>), Failure> {
        let text = host.evaluate(view, &script::script(&self.options))?;
        let answer = answer::parse(view, &text)?;
        let findings = answer.findings(view, self.rules(), self.options.incomplete);
        Ok((answer, findings))
    }
}

/// The arguments of `check.run` this plugin reads: `{"a11y": {"tags": [...]}}` (the arguments
/// are every check plugin's, so its own sit under its MCP prefix); anything else is ignored.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct CheckArgs {
    a11y: Option<CheckOverrides>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct CheckOverrides {
    tags: Option<Vec<String>>,
}

/// The tags `check.run`'s arguments ask for, if any.
///
/// # Errors
///
/// The arguments do not read or name a malformed tag (`errors.bad_args`).
pub fn check_tags(args: &str) -> Result<Option<Vec<String>>, Failure> {
    if args.trim().is_empty() {
        return Ok(None);
    }
    let parsed: CheckArgs =
        serde_json::from_str(args).map_err(|error| bad_args(&error.to_string()))?;
    let tags = parsed.a11y.and_then(|overrides| overrides.tags);
    if let Some(tags) = &tags {
        settings::check_tags("tags", tags)
            .map_err(|_| bad_args("a11y.tags: not a list of axe tags"))?;
    }
    Ok(tags)
}

/// The check.
pub struct A11yAxe;

impl Check for A11yAxe {
    fn describe(&self) -> Vec<RuleInfo> {
        rules::describe()
    }

    fn run(
        &mut self,
        host: &mut dyn PluginHost,
        ctx: &CheckContext,
    ) -> Result<Vec<Finding>, Failure> {
        let settings = Settings::read(host)?;
        let tags = check_tags(&ctx.args)?;
        let plan = Plan::new(&settings, tags.as_deref(), &ctx.rules);
        if plan.rules().is_empty() {
            return Ok(Vec::new());
        }
        let views: Vec<ViewId> = match ctx.view {
            Some(view) => vec![view],
            None => host.views()?.into_iter().map(|view| view.id).collect(),
        };
        let total = u32::try_from(views.len()).unwrap_or(u32::MAX);
        let mut findings = Vec::new();
        let mut first_failure = None;
        let mut checked = 0;
        for (done, view) in (1..).zip(&views) {
            match plan.run_view(host, *view) {
                Ok((_, found)) => {
                    findings.extend(found);
                    checked += 1;
                }
                Err(failure) if ctx.view.is_none() => {
                    host.log("warn", &format!("view {view} not checked: {failure}"));
                    first_failure.get_or_insert(failure);
                }
                Err(failure) => return Err(failure),
            }
            host.report_progress(done, total);
        }
        match first_failure {
            Some(failure) if checked == 0 => Err(failure),
            _ => Ok(findings),
        }
    }
}

fn unknown(name: &str) -> Failure {
    Failure::coded(FailureKind::NotFound, codes::UNKNOWN_CONTRIBUTION).with_param("name", name)
}

impl Feature for A11yAxe {
    fn run_command(
        &mut self,
        host: &mut dyn PluginHost,
        command: &str,
        args: &str,
    ) -> Result<String, Failure> {
        match command {
            "check" => tool::check(host, args, true),
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
            "check" => tool::check(host, args, false),
            other => Err(unknown(other)),
        }
    }
}

#[cfg(test)]
mod fake;

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
