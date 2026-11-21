//! An answer judged against a request's checks (RB-096-04, [`crate::checks`]): one [`Verdict`]
//! per check, in order, with what was found. Equal JSON numbers are equal in any form (`7`,
//! `7.0`); a header's `==` is exact, its `contains` ignores case. What a verdict holds of the
//! answer may hold a secret: the caller masks it before it leaves the core.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::checks::{Check, Compare};
use crate::jsonpath;

/// The most characters of a found value a verdict keeps.
pub const MAX_ACTUAL: usize = 200;

/// The part of an answer the checks read.
#[derive(Clone, Copy, Debug)]
pub struct Observed<'a> {
    pub status: u16,
    pub headers: &'a [(String, String)],
    pub body: &'a [u8],
    /// The whole exchange, in milliseconds.
    pub total_ms: f64,
}

/// Why a check found nothing to compare.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Problem {
    /// Nothing at the path, or no such header.
    Missing,
    /// The body is no JSON.
    NotJson,
    /// The path is no JSONPath of the subset.
    BadPath,
}

/// One check's outcome.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Verdict {
    /// The check's line (`status 2xx`).
    pub check: String,
    pub passed: bool,
    /// What was found, cut to [`MAX_ACTUAL`] characters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actual: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub problem: Option<Problem>,
}

fn cut(text: &str) -> String {
    if text.chars().count() <= MAX_ACTUAL {
        return text.to_owned();
    }
    let mut out: String = text.chars().take(MAX_ACTUAL).collect();
    out.push('\u{2026}');
    out
}

/// `a` and `b` the same JSON, numbers compared by value.
fn same(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x == y || x.as_f64() == y.as_f64(),
        (Value::Array(x), Value::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(x, y)| same(x, y))
        }
        (Value::Object(x), Value::Object(y)) => {
            x.len() == y.len()
                && x.iter()
                    .all(|(key, value)| y.get(key).is_some_and(|other| same(value, other)))
        }
        _ => a == b,
    }
}

/// The expected value: JSON when it reads as JSON, else the text.
fn expected_of(text: &str) -> Value {
    serde_json::from_str(text).unwrap_or_else(|_| Value::String(text.to_owned()))
}

fn shown(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

fn compared(actual: &Value, compare: Compare, text: &str) -> bool {
    let expected = expected_of(text);
    match compare {
        Compare::Equals => same(actual, &expected),
        Compare::Contains => {
            let needle = match &expected {
                Value::String(needle) => needle.clone(),
                other => other.to_string(),
            };
            match actual {
                Value::String(hay) => hay.contains(&needle),
                Value::Array(items) => items.iter().any(|item| same(item, &expected)),
                Value::Object(map) => map.contains_key(&needle),
                _ => false,
            }
        }
    }
}

fn verdict(
    check: &Check,
    passed: bool,
    actual: Option<String>,
    problem: Option<Problem>,
) -> Verdict {
    Verdict {
        check: check.to_line(),
        passed,
        actual: actual.map(|text| cut(&text)),
        problem,
    }
}

fn judge_one(check: &Check, answer: &Observed<'_>) -> Verdict {
    match check {
        Check::Status { expected } => {
            let status = answer.status.to_string();
            let passed = match expected.strip_suffix("xx") {
                Some(class) => status.starts_with(class) && status.len() == 3,
                None => *expected == status,
            };
            verdict(check, passed, Some(status), None)
        }
        Check::Json {
            path,
            compare,
            value,
        } => {
            let Ok(body) = serde_json::from_slice::<Value>(answer.body) else {
                return verdict(check, false, None, Some(Problem::NotJson));
            };
            let Some(found) = jsonpath::select(&body, path) else {
                return verdict(check, false, None, Some(Problem::BadPath));
            };
            let actual = match found.as_slice() {
                [] => return verdict(check, false, None, Some(Problem::Missing)),
                [one] => (*one).clone(),
                many => Value::Array(many.iter().map(|value| (*value).clone()).collect()),
            };
            let passed = compared(&actual, *compare, value);
            verdict(check, passed, Some(shown(&actual)), None)
        }
        Check::Header {
            name,
            compare,
            value,
        } => {
            let found: Vec<&str> = answer
                .headers
                .iter()
                .filter(|(header, _)| header.eq_ignore_ascii_case(name))
                .map(|(_, value)| value.as_str())
                .collect();
            if found.is_empty() {
                return verdict(check, false, None, Some(Problem::Missing));
            }
            let actual = found.join(", ");
            let passed = match compare {
                Compare::Equals => actual == *value,
                Compare::Contains => actual
                    .to_ascii_lowercase()
                    .contains(&value.to_ascii_lowercase()),
            };
            verdict(check, passed, Some(actual), None)
        }
        Check::Time { below_ms } => {
            // At most ten minutes in milliseconds: exact in an f64.
            let bound = *below_ms as f64;
            let passed = answer.total_ms < bound;
            let actual = format!("{} ms", answer.total_ms.round());
            verdict(check, passed, Some(actual), None)
        }
    }
}

/// One verdict per check of `checks`, in order.
#[must_use]
pub fn judge(checks: &[Check], answer: &Observed<'_>) -> Vec<Verdict> {
    checks
        .iter()
        .map(|check| judge_one(check, answer))
        .collect()
}

/// Whether every verdict passed (`true` for none).
#[must_use]
pub fn all_passed(verdicts: &[Verdict]) -> bool {
    verdicts.iter().all(|verdict| verdict.passed)
}

#[cfg(test)]
#[path = "verdict_tests.rs"]
mod tests;
