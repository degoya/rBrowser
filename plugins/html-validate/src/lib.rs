//! The HTML validation check (RB-090-97, RB-094-06): the page source validated per view, its
//! messages as findings - by rule, and by line and column or by element.
//!
//! **How a run works.** Per view, `source` reads the page: by default (`html-validate.source` =
//! `raw`) the document as the server sent it, as the W3C validator reads it, through CDP
//! (`Page.getResourceContent` of the main frame: no second request, the view's own session); with
//! `rendered` the DOM after rendering, serialized in the view's isolated world, whose findings
//! name their element by selector. `script` sends the runner (`runner/`, html-validate bundled)
//! with that source to the view's isolated world through `evaluate`. By default html-validate
//! validates it there, offline, with the rules of `rules` (`html-validate:standard`: standards
//! conformance, no style rules) as the settings and a trusted project's
//! `.rbrowser/htmlvalidate.json` (`project`) set them. With `html-validate.engine` = `nu` the
//! runner POSTs the source to the Nu HTML Checker at `html-validate.nuAddress` instead - a user
//! setting, empty by default, so nothing leaves the device unless the person sets it; a failure
//! of that address fails the view with its reason, never falls back. Views with the same source
//! share one validation. Every message with a place names the selector of its element, which the
//! findings panel highlights in the view (`highlightFinding`).
//!
//! **Why in the view.** html-validate is JavaScript; the component is WebAssembly. The view's
//! isolated world runs it with the capabilities the plugin contract already has (`evaluate`), and
//! the Nu checker's CORS answer lets the same world reach it - no new host capability, no `http`
//! grant, and the plugin itself never sends anything.
//!
//! The logic is tested natively against a fake host; `guest.rs` translates it into the WIT
//! vocabulary and exists on `wasm32` alone.

#[cfg(target_arch = "wasm32")]
mod guest;

pub mod answer;
pub mod project;
pub mod rules;
pub mod script;
pub mod settings;
pub mod source;
pub mod tool;
pub mod unserved;

use rb_plugin_api::check::Check;
use rb_plugin_api::feature::Feature;
use rb_plugin_api::{
    CheckContext, Failure, FailureKind, Finding, PluginHost, RuleInfo, ViewId, codes,
};
use serde_json::{Map, Value};

use crate::answer::Answer;
use crate::script::Request;
use crate::settings::{Engine, Settings, Source};

/// The plugin's id: the prefix of its keys.
pub const PLUGIN_ID: &str = "html-validate";

/// `plugin.html-validate.<group>.<name>`.
#[must_use]
pub fn key(group: &str, name: &str) -> String {
    format!("plugin.{PLUGIN_ID}.{group}.{name}")
}

/// `plugin.html-validate.errors.bad_args`.
pub fn bad_args(reason: &str) -> Failure {
    Failure::coded(FailureKind::BadData, &key("errors", "bad_args")).with_param("reason", reason)
}

/// The validations of one run, by source: views serving the same source share one.
pub type Cache = Vec<(String, Result<Answer, Failure>)>;

/// The settings of a run with the project's rule file (`project::load`).
///
/// # Errors
///
/// A setting or the rule file does not read.
pub fn run_settings(host: &mut dyn PluginHost) -> Result<Settings, Failure> {
    let mut settings = Settings::read(host)?;
    settings.project = project::load(host)?;
    Ok(settings)
}

/// What a run validates with.
#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    pub settings: Settings,
    pub engine: Engine,
    /// The rules the person left on (`checks.rules`), or every rule.
    pub enabled: Vec<String>,
    /// html-validate's rule configuration of the run.
    config: Map<String, Value>,
    /// The checked address and its endpoint, for the Nu engine.
    nu: Option<(String, String)>,
}

impl Plan {
    /// The plan of a run under `settings`, with `engine` instead of the setting's when given, of
    /// the rules in `enabled`.
    ///
    /// # Errors
    ///
    /// The engine is the Nu checker and no address is set (`errors.nu_address_missing`) or the
    /// address is refused (`settings::check_address`).
    pub fn new(
        settings: Settings,
        engine: Option<Engine>,
        enabled: Vec<String>,
    ) -> Result<Self, Failure> {
        let engine = engine.unwrap_or(settings.engine);
        let nu = match engine {
            Engine::Builtin => None,
            Engine::Nu => {
                let address =
                    settings::check_address(settings.nu_address.as_deref().unwrap_or_default())?
                        .ok_or_else(|| {
                            Failure::coded(
                                FailureKind::NotSupported,
                                &key("errors", "nu_address_missing"),
                            )
                        })?;
                let endpoint = settings::nu_endpoint(&address);
                Some((address, endpoint))
            }
        };
        let config = rules::builtin_config(&settings, &enabled);
        Ok(Self {
            settings,
            engine,
            enabled,
            config,
            nu,
        })
    }

    /// Whether the run has any rule to report.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        match self.engine {
            Engine::Builtin => self.config.is_empty(),
            Engine::Nu => !rules::all()
                .iter()
                .any(|rule| rules::is_nu(&rule.id) && self.wants(&rule.id, rule.severity)),
        }
    }

    fn wants(&self, id: &str, severity: rb_plugin_api::Severity) -> bool {
        rules::wanted(id, &self.settings, &self.enabled) && self.settings.keeps(severity)
    }

    fn validate(
        &self,
        host: &mut dyn PluginHost,
        view: ViewId,
        source: &str,
    ) -> Result<Answer, Failure> {
        let (request, address) = match &self.nu {
            Some((address, endpoint)) => (Request::nu(endpoint, source), address.as_str()),
            None => (Request::builtin(source, &self.config), ""),
        };
        let text = host.evaluate(view, &script::script(view, &request)?)?;
        answer::parse(view, &text, self.engine, address)
    }

    /// Validates the source of `view` (served or rendered, `settings.source`), once per distinct
    /// source of the run (`cache`): its findings.
    ///
    /// # Errors
    ///
    /// The view has no source (`source::served`), the validation failed or did not answer.
    pub fn run_view(
        &self,
        host: &mut dyn PluginHost,
        view: ViewId,
        cache: &mut Cache,
    ) -> Result<Vec<Finding>, Failure> {
        let served = match self.settings.source {
            Source::Raw => source::served(host, view)?,
            Source::Rendered => source::rendered(host, view)?,
        };
        let known = cache
            .iter()
            .find(|(text, _)| *text == served.text)
            .map(|(_, result)| result.clone());
        let result = match known {
            Some(result) => result.map_err(|failure| failure.with_param("view", view)),
            None => {
                let result = self.validate(host, view, &served.text);
                cache.push((served.text, result.clone()));
                result
            }
        };
        let answer = result?;
        Ok(answer.findings(
            view,
            &served.url,
            self.engine,
            self.settings.source,
            &self.settings,
            &self.enabled,
        ))
    }
}

/// The check.
pub struct HtmlValidate;

impl Check for HtmlValidate {
    fn describe(&self) -> Vec<RuleInfo> {
        rules::describe()
    }

    fn run(
        &mut self,
        host: &mut dyn PluginHost,
        ctx: &CheckContext,
    ) -> Result<Vec<Finding>, Failure> {
        let plan = Plan::new(run_settings(host)?, None, ctx.rules.clone())?;
        if plan.is_empty() {
            return Ok(Vec::new());
        }
        let views: Vec<ViewId> = match ctx.view {
            Some(view) => vec![view],
            None => host.views()?.into_iter().map(|view| view.id).collect(),
        };
        let total = u32::try_from(views.len()).unwrap_or(u32::MAX);
        let mut cache = Cache::new();
        let mut findings = Vec::new();
        let mut first_failure = None;
        let mut checked = 0;
        for (done, view) in (1..).zip(&views) {
            match plan.run_view(host, *view, &mut cache) {
                Ok(found) => {
                    findings.extend(found);
                    checked += 1;
                }
                Err(failure) if ctx.view.is_none() => {
                    host.log("warn", &format!("view {view} not validated: {failure}"));
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

impl Feature for HtmlValidate {
    fn run_command(
        &mut self,
        host: &mut dyn PluginHost,
        command: &str,
        args: &str,
    ) -> Result<String, Failure> {
        match command {
            "validate" => tool::validate(host, args, true),
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
            "validate" => tool::validate(host, args, false),
            other => Err(unknown(other)),
        }
    }
}

#[cfg(test)]
mod fake;

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
