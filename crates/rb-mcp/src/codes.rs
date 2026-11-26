//! The failure codes' style (RB-099-16): every code a tool, prompt, resource, script or the HTTP
//! endpoint answers is dotted, `area.reason` (`evaluate.disabled`, `settings.rejected.protected`),
//! as the i18n keys are. The flat codes of before (`evaluate_disabled`) stay readable for one
//! phase, until 1.0: an answer whose code replaced a flat one also carries that one as
//! `legacyCode` ([`legacy`], [`add_legacy`]). A client matches `code`; `legacyCode` goes with
//! the alias phase.

use serde_json::{Map, Value};

/// The code of a failure inside rBrowser itself (a task that did not finish, an answer that
/// does not encode).
pub const INTERNAL: &str = "mcp.internal";

/// The field that carries the flat code of the alias phase.
pub const LEGACY_FIELD: &str = "legacyCode";

/// Each dotted code that replaced a flat one, with that flat one.
pub const RENAMED: &[(&str, &str)] = &[
    ("answer.too_large", "answer_too_large"),
    ("args.invalid", "invalid_arguments"),
    ("capture.failed", "capture_failed"),
    ("cdp.failed", "cdp_failed"),
    ("command.failed", "command_failed"),
    ("command.not_implemented", "not_implemented"),
    ("command.unknown", "unknown_command"),
    ("config.breakpoint_not_found", "breakpoint_not_found"),
    ("config.key_protected", "config_key_protected"),
    ("evaluate.disabled", "evaluate_disabled"),
    ("findings.none", "findings_none"),
    ("http.host_rejected", "host_rejected"),
    ("http.nonce_required", "nonce_required"),
    ("http.origin_rejected", "origin_rejected"),
    ("http.unauthorized", "unauthorized"),
    ("mcp.internal", "internal"),
    ("mcp.no_instance", "no_instance"),
    ("metrics.not_ready", "not_ready"),
    ("network.compose_disabled", "compose_disabled"),
    ("network.held_not_found", "held_not_found"),
    ("network.request_not_found", "request_not_found"),
    ("prompts.disabled", "prompts_disabled"),
    ("prompts.not_found", "prompt_not_found"),
    ("resources.disabled", "resources_disabled"),
    ("resources.not_found", "resource_not_found"),
    ("screenshots.none", "screenshots_none"),
    ("script.disabled", "scripts_disabled"),
    ("settings.category_not_found", "category_not_found"),
    ("settings.key_not_found", "key_not_found"),
    ("settings.section_not_found", "section_not_found"),
    ("target.empty_selection", "empty_selection"),
    ("target.no_breakpoint", "view_without_breakpoint"),
    ("target.not_single", "target_not_single"),
    ("tool.failed", "tool_failed"),
    ("tool.not_found", "tool_not_found"),
    ("tool.unknown", "unknown_tool"),
    ("update.unfinished", "unfinished"),
    ("view.not_attached", "view_not_attached"),
    ("view.not_found", "view_not_found"),
    ("window.closed", "closed"),
];

/// The flat code `code` replaced, while the alias phase lasts.
#[must_use]
pub fn legacy(code: &str) -> Option<&'static str> {
    RENAMED
        .iter()
        .find(|(dotted, _)| *dotted == code)
        .map(|(_, flat)| *flat)
}

/// Adds [`LEGACY_FIELD`] to `body` when its `code` replaced a flat one.
pub fn add_legacy(body: &mut Map<String, Value>) {
    let flat = body.get("code").and_then(Value::as_str).and_then(legacy);
    if let Some(flat) = flat {
        body.insert(LEGACY_FIELD.to_owned(), Value::from(flat));
    }
}

/// Whether `code` has the dotted style: two or more lower-case words of letters, digits and `_`
/// joined by dots.
#[must_use]
pub fn is_dotted(code: &str) -> bool {
    let words: Vec<&str> = code.split('.').collect();
    words.len() > 1
        && words.iter().all(|word| {
            !word.is_empty()
                && word
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        })
}

#[cfg(test)]
#[path = "codes_tests.rs"]
mod tests;
