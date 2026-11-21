//! The checks of a request (RB-096-04): what its answer must show - a status, a value at a
//! JSONPath of the JSON body, a header, the time - each one `# @rb-check <line>` in the `.http`
//! file, which VS Code REST Client and the JetBrains HTTP Client skip as a comment:
//!
//! ```text
//! # @rb-check status 2xx                      a code (201) or a class (2xx)
//! # @rb-check json $.user.name == "Ada"       the value equals (JSON, or the text as it is)
//! # @rb-check json $.tags contains admin      a text holds it, a list has it, an object has the key
//! # @rb-check header Content-Type contains json
//! # @rb-check time < 500                      the whole exchange under 500 ms
//! ```
//!
//! The values are literal, never templates: a check compares what came back, and a secret has
//! no place in a checked-in file. [`crate::verdict`] judges an answer against them.

use serde::{Deserialize, Serialize};

use crate::request::{MAX_TIMEOUT_MS, is_token};

/// The most checks of one request.
pub const MAX_CHECKS: usize = 50;
/// The longest JSONPath or value of a check, in bytes.
pub const MAX_CHECK_TEXT: usize = 1000;

/// How a value is compared.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Compare {
    /// `==`: the same value.
    #[default]
    Equals,
    /// `contains`: a text holds it, a list has it as an element, an object has it as a key.
    Contains,
}

impl Compare {
    /// The operator in the line: `==` or `contains`.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::Equals => "==",
            Self::Contains => "contains",
        }
    }
}

/// One check of an answer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Check {
    /// The status: a code (`201`) or a class (`2xx`).
    Status { expected: String },
    /// The value at `path` (a JSONPath of [`crate::jsonpath`]) of the JSON body; `value` is
    /// JSON (`42`, `"Ada"`, `true`), or a text as it is when it does not read as JSON.
    Json {
        path: String,
        #[serde(default)]
        compare: Compare,
        value: String,
    },
    /// A header of the answer (the name in any case; several of one name joined by `, `).
    Header {
        name: String,
        #[serde(default)]
        compare: Compare,
        #[serde(default)]
        value: String,
    },
    /// The whole exchange under `below_ms` milliseconds.
    Time { below_ms: u64 },
}

fn one_line(text: &str) -> bool {
    !text.contains(['\r', '\n'])
}

/// The value and its operator in `rest` (`<operand> == <value>`), split at the first operator.
fn split_compare(rest: &str) -> Option<(&str, Compare, &str)> {
    let found = [Compare::Equals, Compare::Contains]
        .into_iter()
        .filter_map(|compare| {
            let marker = format!(" {} ", compare.token());
            let padded = format!("{rest} ");
            padded.find(&marker).map(|at| (at, compare, marker.len()))
        })
        .min_by_key(|(at, _, _)| *at)?;
    let (at, compare, len) = found;
    let operand = rest[..at].trim();
    let value = rest.get(at + len..).unwrap_or_default().trim();
    (!operand.is_empty()).then_some((operand, compare, value))
}

impl Check {
    /// The check's line after `@rb-check `.
    #[must_use]
    pub fn to_line(&self) -> String {
        match self {
            Self::Status { expected } => format!("status {expected}"),
            Self::Json {
                path,
                compare,
                value,
            } => format!("json {path} {} {value}", compare.token())
                .trim_end()
                .to_owned(),
            Self::Header {
                name,
                compare,
                value,
            } => format!("header {name} {} {value}", compare.token())
                .trim_end()
                .to_owned(),
            Self::Time { below_ms } => format!("time < {below_ms}"),
        }
    }

    /// The check a line after `@rb-check ` names.
    ///
    /// # Errors
    ///
    /// The reason: `check kind`, `check status`, `check operator`, `check time`.
    pub fn from_line(line: &str) -> Result<Self, &'static str> {
        let line = line.trim();
        let (kind, rest) = line.split_once(' ').unwrap_or((line, ""));
        let rest = rest.trim();
        match kind {
            "status" if rest.is_empty() => Err("check status"),
            "status" => Ok(Self::Status {
                expected: rest.to_owned(),
            }),
            "json" => {
                let (path, compare, value) = split_compare(rest).ok_or("check operator")?;
                Ok(Self::Json {
                    path: path.to_owned(),
                    compare,
                    value: value.to_owned(),
                })
            }
            "header" => {
                let (name, compare, value) = split_compare(rest).ok_or("check operator")?;
                Ok(Self::Header {
                    name: name.to_owned(),
                    compare,
                    value: value.to_owned(),
                })
            }
            "time" => {
                let ms = rest.strip_prefix('<').ok_or("check time")?.trim();
                let below_ms = ms.parse().map_err(|_| "check time")?;
                Ok(Self::Time { below_ms })
            }
            _ => Err("check kind"),
        }
    }

    /// Checks the form: a status code or class, a JSONPath from `$`, a header name that is a
    /// token, one-line texts within [`MAX_CHECK_TEXT`], a time within the longest timeout.
    ///
    /// # Errors
    ///
    /// The reason.
    pub fn validate(&self) -> Result<(), &'static str> {
        let text_ok = |text: &str| one_line(text) && text.len() <= MAX_CHECK_TEXT;
        match self {
            Self::Status { expected } => {
                let bytes = expected.as_bytes();
                let ok = bytes.len() == 3
                    && (b'1'..=b'5').contains(&bytes[0])
                    && (bytes[1..].iter().all(u8::is_ascii_digit) || &bytes[1..] == b"xx");
                ok.then_some(()).ok_or("check status")
            }
            Self::Json { path, value, .. } => {
                if !path.trim_start().starts_with('$') || !text_ok(path) {
                    return Err("check path");
                }
                text_ok(value).then_some(()).ok_or("check value")
            }
            Self::Header { name, value, .. } => {
                if !is_token(name) {
                    return Err("check header");
                }
                text_ok(value).then_some(()).ok_or("check value")
            }
            Self::Time { below_ms } => (1..=MAX_TIMEOUT_MS)
                .contains(below_ms)
                .then_some(())
                .ok_or("check time"),
        }
    }
}

#[cfg(test)]
#[path = "checks_tests.rs"]
mod tests;
