//! The script one validation sends to a view's isolated world (`evaluate`): the runner
//! (`runner/src/main.ts`, html-validate bundled) and the call with the run's options - the served
//! source among them - all inside one function, so nothing of it is left in the world's globals
//! and none of it reaches the page's main world. The script evaluates to a promise of the
//! runner's answer (`crate::answer`).

use rb_plugin_api::{Failure, FailureKind, MAX_JSON_CHARS, ViewId};
use serde::Serialize;
use serde_json::{Map, Value};

use crate::key;

/// The runner, one IIFE defining `rbHtmlValidate`.
pub const RUNNER_SOURCE: &str = include_str!("../runner/dist/runner.js");

/// The most messages one answer holds.
pub const MAX_MESSAGES: u32 = 1_000;
/// The most characters of a message and of an extract.
pub const MAX_TEXT: u32 = 300;
/// How long a Nu HTML Checker may take to answer, in milliseconds.
pub const NU_TIMEOUT_MS: u32 = 30_000;

/// What the runner is asked for (`StartOptions` in `runner/src/main.ts`).
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "engine", rename_all = "camelCase")]
pub enum Request<'a> {
    /// html-validate with `rules` (html-validate's own configuration form).
    #[serde(rename_all = "camelCase")]
    Builtin {
        source: &'a str,
        rules: &'a Map<String, Value>,
        max_messages: u32,
        max_text: u32,
    },
    /// The Nu HTML Checker at `endpoint`.
    #[serde(rename_all = "camelCase")]
    Nu {
        endpoint: &'a str,
        source: &'a str,
        max_messages: u32,
        max_text: u32,
        timeout_ms: u32,
    },
}

impl<'a> Request<'a> {
    /// html-validate on `source` with `rules`.
    #[must_use]
    pub fn builtin(source: &'a str, rules: &'a Map<String, Value>) -> Self {
        Self::Builtin {
            source,
            rules,
            max_messages: MAX_MESSAGES,
            max_text: MAX_TEXT,
        }
    }

    /// The Nu HTML Checker at `endpoint` on `source`.
    #[must_use]
    pub fn nu(endpoint: &'a str, source: &'a str) -> Self {
        Self::Nu {
            endpoint,
            source,
            max_messages: MAX_MESSAGES,
            max_text: MAX_TEXT,
            timeout_ms: NU_TIMEOUT_MS,
        }
    }
}

/// The script of one validation in `view`. The options go in as JSON - a JSON text is a
/// JavaScript expression, so the source can never leave its string.
///
/// # Errors
///
/// The script would be longer than the host takes (`errors.too_large`).
pub fn script(view: ViewId, request: &Request<'_>) -> Result<String, Failure> {
    let options = serde_json::to_string(request).expect("the options always serialize");
    let script =
        format!("(() => {{\n{RUNNER_SOURCE}\n;\nreturn rbHtmlValidate.start({options});\n}})()");
    if script.len() > MAX_JSON_CHARS {
        return Err(
            Failure::coded(FailureKind::Limit, &key("errors", "too_large"))
                .with_param("view", view)
                .with_param("size", script.len().div_ceil(1024))
                .with_param("limit", MAX_JSON_CHARS / 1024),
        );
    }
    Ok(script)
}

#[cfg(test)]
#[path = "script_tests.rs"]
mod tests;
