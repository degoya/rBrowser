//! The runner's answer (`Answer` in `runner/src/shape.ts`) and the findings it makes, the same
//! for both engines: one per message, the rule id the engine's (`no-dup-id`) or the Nu message
//! type (`nu-error`), its severity, the message key with the engine's English text, and the
//! selector of its element when there is one (the runner names one for every message with a
//! line, so the findings panel can highlight it). The element path - what the stable id is made
//! of, so the same message is one finding across the views (a second message of the same rule
//! at the same place gets `#2`) - depends on the source (RB-094-06): for the served source it is
//! `source:<line>:<column>`, with line and column as parameters; for the rendered DOM, whose
//! lines are those of a serialization, it is the selector, a parameter too, and a message
//! without an element has none. The extract, the engine, the source and the rule's help page go
//! into `data`.

use std::collections::BTreeMap;

use rb_plugin_api::{Failure, FailureKind, Finding, Severity, ViewId};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::settings::{Engine, Settings, Source};
use crate::{key, rules};

/// One message about the served source.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Message {
    pub rule: String,
    pub severity: Severity,
    pub message: String,
    pub line: u32,
    pub column: u32,
    pub selector: Option<String>,
    pub extract: String,
}

/// The answer of one validation.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Answer {
    pub version: String,
    pub messages: Vec<Message>,
    pub truncated: bool,
    /// Set instead of the rest when the validation failed.
    pub error: Option<String>,
    /// The browser refused the request to the Nu checker (the runner's network error).
    pub blocked: bool,
}

fn bad_answer(view: ViewId, reason: &str) -> Failure {
    Failure::coded(FailureKind::BadData, &key("errors", "bad_answer"))
        .with_param("view", view)
        .with_param("reason", reason)
}

/// The answer in `text`, the JSON `evaluate` gave back - the runner's object, or that object as
/// a JSON string when the host hands a value over as text. `address` is the Nu checker's, named
/// in its failure.
///
/// # Errors
///
/// The text is no answer (`errors.bad_answer`); the validation failed: `errors.nu_failed` with
/// the address and the reason for the Nu checker (`errors.nu_blocked` when the browser refused
/// the request), `errors.validate_failed` for html-validate.
pub fn parse(view: ViewId, text: &str, engine: Engine, address: &str) -> Result<Answer, Failure> {
    let value: Value =
        serde_json::from_str(text).map_err(|error| bad_answer(view, &error.to_string()))?;
    let value = match value {
        Value::String(inner) => {
            serde_json::from_str(&inner).map_err(|error| bad_answer(view, &error.to_string()))?
        }
        other => other,
    };
    if !value.is_object() {
        return Err(bad_answer(view, "the answer is no object"));
    }
    let answer: Answer =
        serde_json::from_value(value).map_err(|error| bad_answer(view, &error.to_string()))?;
    if let Some(error) = &answer.error {
        let reason: String = error.chars().take(300).collect();
        return Err(match engine {
            Engine::Nu => Failure::coded(
                FailureKind::Io,
                &key(
                    "errors",
                    if answer.blocked {
                        "nu_blocked"
                    } else {
                        "nu_failed"
                    },
                ),
            )
            .with_param("address", address)
            .with_param("view", view)
            .with_param("reason", reason),
            Engine::Builtin => {
                Failure::coded(FailureKind::Other, &key("errors", "validate_failed"))
                    .with_param("view", view)
                    .with_param("reason", reason)
            }
        });
    }
    Ok(answer)
}

fn finding(
    view: ViewId,
    url: &str,
    (engine, source): (Engine, Source),
    message: &Message,
    path: Option<String>,
) -> Finding {
    let selector = message.selector.clone().filter(|s| !s.is_empty());
    let raw = source == Source::Raw;
    let at = raw && message.line > 0;
    let help_url = rules::find(&message.rule)
        .map(|rule| rule.url.clone())
        .filter(|url| !url.is_empty());
    let data = json!({
        "engine": engine.as_str(),
        "source": source.as_str(),
        "line": raw.then_some(message.line),
        "column": raw.then_some(message.column),
        "extract": message.extract,
        "helpUrl": help_url,
    });
    let mut params = vec![
        ("message".to_owned(), message.message.clone()),
        ("rule".to_owned(), message.rule.clone()),
    ];
    let key_name = if at {
        params.push(("line".to_owned(), message.line.to_string()));
        params.push(("column".to_owned(), message.column.to_string()));
        "at"
    } else if let (Source::Rendered, Some(selector)) = (source, &selector) {
        params.push(("selector".to_owned(), selector.clone()));
        "element"
    } else {
        "document"
    };
    Finding {
        rule: message.rule.clone(),
        severity: message.severity,
        message: key("findings", key_name),
        params,
        view: Some(view),
        url: (!url.is_empty()).then(|| url.to_owned()),
        selector,
        path,
        data: Some(data.to_string()),
    }
}

/// Where a message is, for its stable id: its line and column in the served source, its
/// element's selector in the rendered DOM (none without an element).
fn place(source: Source, message: &Message) -> Option<String> {
    match source {
        Source::Raw => Some(format!("source:{}:{}", message.line, message.column)),
        Source::Rendered => message.selector.clone().filter(|s| !s.is_empty()),
    }
}

impl Answer {
    /// The findings of this answer in view `view` of `url`, validated from `source`: the
    /// messages of the rules the run wants (`rules::wanted`: left on, not switched off) at or
    /// above the lowest severity.
    #[must_use]
    pub fn findings(
        &self,
        view: ViewId,
        url: &str,
        engine: Engine,
        source: Source,
        settings: &Settings,
        enabled: &[String],
    ) -> Vec<Finding> {
        let mut seen: BTreeMap<(String, String), u32> = BTreeMap::new();
        self.messages
            .iter()
            .filter(|message| rules::wanted(&message.rule, settings, enabled))
            .filter(|message| settings.keeps(message.severity))
            .map(|message| {
                let path = place(source, message).map(|place| {
                    let count = seen
                        .entry((message.rule.clone(), place.clone()))
                        .or_insert(0);
                    *count += 1;
                    if *count == 1 {
                        place
                    } else {
                        format!("{place}#{count}")
                    }
                });
                finding(view, url, (engine, source), message, path)
            })
            .collect()
    }
}

#[cfg(test)]
#[path = "answer_tests.rs"]
mod tests;
