//! Chromium's own verdict: CDP `Page.getInstallabilityErrors` (no parameters, reads only), each
//! error a finding of `install.error` with its id and arguments. Not asked when the run does not
//! want the rule; a view without it (Firefox, WebKit) or a refusal is reported, not a failure.

use rb_plugin_api::{PluginHost, ViewId, codes};
use serde::{Deserialize, Serialize};

use crate::rules::{INSTALL_ERROR, Place, Sink};

/// The CDP method.
pub const METHOD: &str = "Page.getInstallabilityErrors";
/// The most errors taken.
const MAX_ERRORS: usize = 32;

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Answer {
    installability_errors: Vec<RawError>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct RawError {
    error_id: String,
    error_arguments: Vec<RawArgument>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RawArgument {
    name: String,
    value: String,
}

/// One installability error.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct InstallError {
    pub id: String,
    /// `(name, value)` of its arguments.
    pub arguments: Vec<(String, String)>,
}

/// The installability verdict as the report shows it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallReport {
    pub checked: bool,
    pub errors: Vec<InstallError>,
    /// Why it was not asked or not answered (the host's failure code).
    pub failure: Option<String>,
}

/// Chromium's installability errors for the page `view` shows.
pub fn check(host: &mut dyn PluginHost, view: ViewId, sink: &mut Sink<'_>) -> InstallReport {
    if !sink.wants(INSTALL_ERROR) {
        return InstallReport::default();
    }
    let answer = match host.cdp_send(view, METHOD, "{}") {
        Ok(answer) => answer,
        Err(failure) => {
            return InstallReport {
                failure: Some(failure.code().unwrap_or(codes::HOST_FAILED).to_owned()),
                ..InstallReport::default()
            };
        }
    };
    let Ok(answer) = serde_json::from_str::<Answer>(&answer) else {
        return InstallReport {
            failure: Some(codes::BAD_OUTPUT.to_owned()),
            ..InstallReport::default()
        };
    };
    let errors: Vec<InstallError> = answer
        .installability_errors
        .into_iter()
        .filter(|error| !error.error_id.is_empty())
        .take(MAX_ERRORS)
        .map(|error| InstallError {
            id: error.error_id,
            arguments: error
                .error_arguments
                .into_iter()
                .take(8)
                .map(|argument| (argument.name, argument.value))
                .collect(),
        })
        .collect();
    for error in &errors {
        let details: Vec<String> = error
            .arguments
            .iter()
            .map(|(name, value)| format!("{name}: {value}"))
            .collect();
        let params = [("error", error.id.clone()), ("details", details.join(", "))];
        sink.emit(INSTALL_ERROR, Place::Install(error.id.clone()), &params);
    }
    InstallReport {
        checked: true,
        errors,
        failure: None,
    }
}

#[cfg(test)]
#[path = "install_tests.rs"]
mod tests;
