//! What the host accepts of a plugin's answers: everything bounded, every text an i18n key of the
//! plugin's own where it is shown, every pointer, file name and JSON well formed. A broken answer
//! is `plugins.error.bad_output` naming the rule, and counts towards suspension.
//!
//! Adapted from rCommander's `*/validate.rs` (one check per answer, before anything shows it).

use rb_plugin_api::{
    MAX_JSON_CHARS,
    analyzer::MAX_PROPOSALS,
    analyzer::valid_pointer,
    check::{MAX_FINDINGS, MAX_RULES, valid_rule_id},
    export::MAX_EXPORT_BYTES,
    export::valid_file_name,
};
use rb_plugin_types::{
    ExportFormat, ExportOutput, Failure, FailureKind, Finding, Proposal, RuleInfo,
};

use crate::{bindings::rbrowser::plugin::types, failure::bad_output};

/// The most checks or formats one plugin offers.
const MAX_OFFERS: usize = 256;
/// The most parameters of one finding, proposal or failure.
const MAX_PARAMS: usize = 16;
/// The longest short text (an id, a rule, a parameter) in characters.
const MAX_SHORT_CHARS: usize = 256;
/// The longest URL, selector or path of a finding.
const MAX_LONG_CHARS: usize = 8 * 1024;
/// The longest `data` of one finding, in bytes of JSON text.
const MAX_DATA_BYTES: usize = 64 * 1024;
/// The longest help URL.
const MAX_HELP_URL_CHARS: usize = 2048;

fn short(text: &str) -> bool {
    !text.is_empty() && text.chars().count() <= MAX_SHORT_CHARS
}

fn own_key(key: &str, prefix: &str) -> bool {
    key.starts_with(prefix) && rb_i18n::is_well_formed_key(key)
}

fn params_ok(params: &[(String, String)]) -> bool {
    params.len() <= MAX_PARAMS
        && params
            .iter()
            .all(|(name, value)| short(name) && value.chars().count() <= MAX_SHORT_CHARS)
}

/// A plugin's failure as the host passes it on: its code and parameters cut to size.
pub(crate) fn from_wit_failure(failure: types::Failure) -> Failure {
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
    let mut native = Failure::new(kind);
    native.code = failure
        .code
        .filter(|code| rb_i18n::is_well_formed_key(code) && short(code));
    for (name, value) in failure.params.into_iter().take(MAX_PARAMS) {
        let value: String = value.chars().take(MAX_SHORT_CHARS).collect();
        native = native.with_param(&name.chars().take(64).collect::<String>(), value);
    }
    native
}

/// A command's or tool's answer: JSON text of bounded size.
pub(crate) fn json(answer: String) -> Result<String, Failure> {
    if answer.len() > MAX_JSON_CHARS {
        return Err(bad_output("answer_too_large"));
    }
    serde_json::from_str::<serde::de::IgnoredAny>(&answer)
        .map_err(|_| bad_output("answer_not_json"))?;
    Ok(answer)
}

/// A help page: `http(s)`, no whitespace or control characters, bounded.
fn help_url_ok(url: &str) -> bool {
    (url.starts_with("https://") || url.starts_with("http://"))
        && url.chars().count() <= MAX_HELP_URL_CHARS
        && !url.chars().any(|c| c.is_whitespace() || c.is_control())
}

/// The rules a check plugin describes.
pub(crate) fn rules(rules: Vec<RuleInfo>, prefix: &str) -> Result<Vec<RuleInfo>, Failure> {
    if rules.len() > MAX_RULES {
        return Err(bad_output("too_many_rules"));
    }
    let mut ids = std::collections::BTreeSet::new();
    for rule in &rules {
        if !valid_rule_id(&rule.id)
            || !ids.insert(rule.id.as_str())
            || !own_key(&rule.title, prefix)
            || !rule.shared_id.as_deref().is_none_or(valid_rule_id)
            || !rule.help_url.as_deref().is_none_or(help_url_ok)
        {
            return Err(bad_output("rule_info"));
        }
    }
    Ok(rules)
}

/// The findings of one check run or report.
pub(crate) fn findings(findings: Vec<Finding>, prefix: &str) -> Result<Vec<Finding>, Failure> {
    if findings.len() > MAX_FINDINGS {
        return Err(bad_output("too_many_findings"));
    }
    for finding in &findings {
        let long_ok = |text: &Option<String>| {
            text.as_ref()
                .is_none_or(|text| text.chars().count() <= MAX_LONG_CHARS)
        };
        let data_ok = finding.data.as_ref().is_none_or(|data| {
            data.len() <= MAX_DATA_BYTES
                && serde_json::from_str::<serde::de::IgnoredAny>(data).is_ok()
        });
        if !valid_rule_id(&finding.rule)
            || !own_key(&finding.message, prefix)
            || !params_ok(&finding.params)
            || !long_ok(&finding.url)
            || !long_ok(&finding.selector)
            || !long_ok(&finding.path)
            || !data_ok
        {
            return Err(bad_output("finding"));
        }
    }
    Ok(findings)
}

/// The proposals of one analysis.
pub(crate) fn proposals(proposals: Vec<Proposal>, prefix: &str) -> Result<Vec<Proposal>, Failure> {
    if proposals.len() > MAX_PROPOSALS {
        return Err(bad_output("too_many_proposals"));
    }
    for proposal in &proposals {
        let value_ok = proposal.value.len() <= MAX_LONG_CHARS
            && serde_json::from_str::<serde::de::IgnoredAny>(&proposal.value).is_ok();
        if !valid_pointer(&proposal.pointer)
            || proposal.pointer.len() > MAX_SHORT_CHARS
            || !value_ok
            || !own_key(&proposal.reason, prefix)
            || !params_ok(&proposal.params)
        {
            return Err(bad_output("proposal"));
        }
    }
    Ok(proposals)
}

/// The formats an export plugin writes.
pub(crate) fn formats(
    formats: Vec<ExportFormat>,
    prefix: &str,
) -> Result<Vec<ExportFormat>, Failure> {
    if formats.len() > MAX_OFFERS {
        return Err(bad_output("too_many_formats"));
    }
    for format in &formats {
        let extension_ok = (1..=16).contains(&format.extension.len())
            && format.extension.chars().all(|c| c.is_ascii_alphanumeric());
        if !short(&format.id) || !own_key(&format.title, prefix) || !extension_ok {
            return Err(bad_output("export_format"));
        }
    }
    Ok(formats)
}

/// What an export plugin rendered.
pub(crate) fn export_output(output: ExportOutput) -> Result<ExportOutput, Failure> {
    if !valid_file_name(&output.file_name) {
        return Err(bad_output("export_file_name"));
    }
    if output.text.len() > MAX_EXPORT_BYTES {
        return Err(bad_output("export_too_large"));
    }
    Ok(output)
}

#[cfg(test)]
#[path = "answers_tests.rs"]
mod tests;
