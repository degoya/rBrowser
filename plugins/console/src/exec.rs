//! The command line: a JavaScript expression run in the views, one result per distinct value.
//!
//! The expression runs through the `evaluate` capability, in each view's isolated world - the DOM
//! is the page's (`document.title`, `document.querySelectorAll(...)`), the page's own globals are
//! not. [`script`] wraps it so that a throw is a result too and every value comes back as a short
//! text, the way a console prints it; equal results of several views are one line with their
//! views ([`run`]).

use rb_plugin_api::{Failure, FailureKind, PluginHost, ViewId};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::normalize::cut;

/// The longest expression.
pub const MAX_EXPRESSION_CHARS: usize = 10_000;
/// The longest result text kept.
pub const MAX_RESULT_CHARS: usize = 2000;

/// Where a command runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    All,
    Active,
    View(ViewId),
}

impl Target {
    /// The target `value` names (`"all"`, `"active"` or a view id); `default` without one.
    ///
    /// # Errors
    ///
    /// Another value.
    pub fn parse(value: &Value, default: Self) -> Result<Self, Failure> {
        match value {
            Value::Null => Ok(default),
            Value::String(name) if name == "all" => Ok(Self::All),
            Value::String(name) if name == "active" => Ok(Self::Active),
            Value::Number(number) => number
                .as_u64()
                .and_then(|id| ViewId::try_from(id).ok())
                .map(Self::View)
                .ok_or_else(|| crate::bad_args("target")),
            _ => Err(crate::bad_args("target")),
        }
    }
}

/// The wrapper run in each view: `{ ok, type, text }` - the value's type and its console text,
/// or the error's name and message.
const WRAPPER: &str = r"(() => {
  const MAX = 2000;
  const show = (value) => {
    const type = typeof value;
    if (value === undefined) return 'undefined';
    if (value === null) return 'null';
    if (type === 'string') return JSON.stringify(value);
    if (type === 'number' || type === 'boolean' || type === 'bigint' || type === 'symbol') return String(value);
    if (type === 'function') return 'function ' + (value.name || '(anonymous)') + '()';
    if (typeof Element !== 'undefined' && value instanceof Element) {
      const id = value.id ? '#' + value.id : '';
      const cls = typeof value.className === 'string' && value.className ? '.' + value.className.trim().split(/\s+/).join('.') : '';
      return '<' + value.tagName.toLowerCase() + id + cls + '>';
    }
    if (typeof Promise !== 'undefined' && value instanceof Promise) return 'Promise';
    try {
      const text = JSON.stringify(value);
      if (text !== undefined) return text;
    } catch (_) {}
    return Object.prototype.toString.call(value);
  };
  try {
    const value = (
__EXPRESSION__
    );
    return { ok: true, type: value === null ? 'null' : typeof value, text: show(value).slice(0, MAX) };
  } catch (error) {
    const text = error && error.name ? error.name + ': ' + error.message : String(error);
    return { ok: false, type: 'error', text: text.slice(0, MAX) };
  }
})()";

/// The script that runs `expression` (trailing `;` and whitespace dropped).
#[must_use]
pub fn script(expression: &str) -> String {
    let expression = expression.trim().trim_end_matches(';').trim_end();
    WRAPPER.replace("__EXPRESSION__", expression)
}

#[derive(Deserialize)]
struct Outcome {
    ok: bool,
    #[serde(default, rename = "type")]
    kind: String,
    #[serde(default)]
    text: String,
}

/// Runs `expression` in the views of `target`: `{ expression, results: [{ ok, type, text,
/// views }] }`, one result per distinct outcome in the order the views answered. A view that
/// cannot run it (an engine without `evaluate`, a view gone) answers its failure's code.
///
/// # Errors
///
/// No expression, one too long, no active view for `Active`, an unknown view, or the views
/// cannot be read.
pub fn run(host: &mut dyn PluginHost, expression: &str, target: Target) -> Result<Value, Failure> {
    if expression.trim().is_empty() {
        return Err(crate::coded(FailureKind::BadData, "errors.no_expression"));
    }
    if expression.chars().count() > MAX_EXPRESSION_CHARS {
        return Err(
            crate::coded(FailureKind::Limit, "errors.expression_too_long")
                .with_param("limit", MAX_EXPRESSION_CHARS),
        );
    }
    let known: Vec<ViewId> = host.views()?.iter().map(|view| view.id).collect();
    let views = match target {
        Target::All => known,
        Target::Active => vec![
            host.active_view()?
                .ok_or_else(|| crate::coded(FailureKind::NotFound, "errors.no_active_view"))?,
        ],
        Target::View(view) if known.contains(&view) => vec![view],
        Target::View(view) => {
            return Err(
                crate::coded(FailureKind::NotFound, "errors.unknown_view").with_param("view", view)
            );
        }
    };
    let script = script(expression);
    let mut results: Vec<(bool, String, String, Vec<ViewId>)> = Vec::new();
    for view in views {
        let (ok, kind, text) = match host.evaluate(view, &script) {
            Ok(answer) => match serde_json::from_str::<Outcome>(&answer) {
                Ok(outcome) => (
                    outcome.ok,
                    outcome.kind,
                    cut(&outcome.text, MAX_RESULT_CHARS),
                ),
                Err(_) => (true, "unknown".to_owned(), cut(&answer, MAX_RESULT_CHARS)),
            },
            Err(failure) => (
                false,
                "failure".to_owned(),
                failure
                    .code
                    .unwrap_or_else(|| "plugins.error.host_failed".to_owned()),
            ),
        };
        match results
            .iter_mut()
            .find(|result| result.0 == ok && result.2 == text)
        {
            Some(result) => result.3.push(view),
            None => results.push((ok, kind, text, vec![view])),
        }
    }
    let results: Vec<Value> = results
        .into_iter()
        .map(|(ok, kind, text, views)| json!({ "ok": ok, "type": kind, "text": text, "views": views }))
        .collect();
    Ok(json!({ "expression": cut(expression.trim(), 200), "results": results }))
}

#[cfg(test)]
#[path = "exec_tests.rs"]
mod tests;
