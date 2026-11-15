//! What a plugin author runs against their own package before publishing it: "it worked on my
//! machine" and "this rBrowser will run it" become one question.
//!
//! Copied and adapted from rCommander's `crates/rc-plugin-host/src/conformance.rs` (itself from
//! rDownloader's): the package verifies as on install, the component exports its declared worlds,
//! and one behavioural question per kind, answered without a malfunction - a feature activates,
//! a check plugin describes well-formed rules and runs them all, an analyzer answers on empty
//! arguments, an export plugin lists a format and renders an empty recording. A failure the
//! plugin answers on purpose passes; a trap, a timeout or a malformed answer does not.
//!
//! Deliberately not here: whether the plugin is right. The report says what was verified.

use rb_plugin_types::{Failure, PluginKind, codes};
use serde::Serialize;

use crate::{
    PluginVerifier, VerifyError,
    plugin::{LoadedPlugin, PluginOptions},
};

/// One verified property.
#[derive(Debug, Serialize)]
pub struct ConformanceCheck {
    /// Stable identifier, so a CI job can act on a specific failure.
    pub id: &'static str,
    pub passed: bool,
    /// Why it failed, or what it found.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// The result of one conformance run.
#[derive(Debug, Serialize)]
pub struct ConformanceReport {
    pub plugin_id: Option<String>,
    pub version: Option<String>,
    pub passed: bool,
    pub checks: Vec<ConformanceCheck>,
}

impl ConformanceReport {
    fn push(&mut self, id: &'static str, outcome: Result<String, String>) {
        let (passed, detail) = match outcome {
            Ok(found) => (true, (!found.is_empty()).then_some(found)),
            Err(why) => (false, Some(why)),
        };
        self.passed &= passed;
        self.checks.push(ConformanceCheck { id, passed, detail });
    }
}

/// Runs the conformance checks on `package` (the bytes of a `.rbplugin`).
pub async fn run_conformance(package: &[u8], verifier: &PluginVerifier) -> ConformanceReport {
    let mut report = ConformanceReport {
        plugin_id: None,
        version: None,
        passed: true,
        checks: Vec::new(),
    };
    let verified = match verifier.verify_bytes(package) {
        Ok(verified) => verified,
        Err(error) => {
            let detail = match &error {
                VerifyError::UntrustedKey { .. } => error.to_string(),
                VerifyError::Other(inner) => format!("{}: {inner:#}", error.code()),
            };
            report.push("package", Err(detail));
            return report;
        }
    };
    report.plugin_id = Some(verified.manifest.id.clone());
    report.version = Some(verified.manifest.version.to_string());
    report.push("package", Ok(String::new()));
    let plugin = match LoadedPlugin::load(&verified, PluginOptions::default()) {
        Ok(plugin) => plugin,
        Err(failure) => {
            report.push("world", Err(describe(&failure)));
            return report;
        }
    };
    report.push("world", Ok(String::new()));
    for kind in verified.manifest.kinds.clone() {
        let outcome = question(&plugin, kind).await;
        report.push(kind.world(), outcome);
    }
    report
}

async fn question(plugin: &LoadedPlugin, kind: PluginKind) -> Result<String, String> {
    match kind {
        PluginKind::Feature => answered(plugin.activate().await.map(|()| "activated".to_owned())),
        PluginKind::Check => {
            let rules = plugin
                .describe()
                .await
                .map_err(|failure| describe(&failure))?;
            if rules.is_empty() {
                return Err("describes no rule".to_owned());
            }
            let ctx = rb_plugin_types::CheckContext {
                rules: rules.into_iter().map(|rule| rule.id).collect(),
                args: "{}".to_owned(),
                ..rb_plugin_types::CheckContext::default()
            };
            answered(
                plugin
                    .run_check(&ctx)
                    .await
                    .map(|found| format!("{} findings", found.len())),
            )
        }
        PluginKind::Analyzer => answered(
            plugin
                .analyze("{}")
                .await
                .map(|found| format!("{} proposals", found.len())),
        ),
        PluginKind::Export => {
            let formats = plugin
                .formats()
                .await
                .map_err(|failure| describe(&failure))?;
            let first = formats.first().ok_or("lists no format")?;
            answered(
                plugin
                    .render(&first.id, "{}")
                    .await
                    .map(|output| output.file_name),
            )
        }
    }
}

/// An answer, or a failure the plugin gave on purpose: both pass. A malfunction does not.
fn answered(outcome: Result<String, Failure>) -> Result<String, String> {
    match outcome {
        Ok(found) => Ok(found),
        Err(failure) if is_malfunction(&failure) => Err(describe(&failure)),
        Err(failure) => Ok(format!("answered the failure {}", describe(&failure))),
    }
}

fn is_malfunction(failure: &Failure) -> bool {
    matches!(
        failure.code(),
        Some(
            codes::CRASHED
                | codes::TIMED_OUT
                | codes::OUT_OF_FUEL
                | codes::OUT_OF_MEMORY
                | codes::BAD_OUTPUT
                | codes::HOST_FAILED
        )
    )
}

fn describe(failure: &Failure) -> String {
    let params: Vec<String> = failure
        .params
        .iter()
        .map(|(name, value)| format!("{name}={value}"))
        .collect();
    if params.is_empty() {
        failure.to_string()
    } else {
        format!("{failure} ({})", params.join(", "))
    }
}

#[cfg(test)]
#[path = "conformance_tests.rs"]
mod tests;
