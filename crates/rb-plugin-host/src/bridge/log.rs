//! What a page script logs (RB-080-66): `rbPlugin.log(level, ...values)`, and the `console` of
//! the plugin's isolated world, which the prologue sends the same way. A line reaches rBrowser
//! as `l:<json>`, `{"level", "values": [<text>...]}`, and never the page's console: rBrowser's
//! messages are not the page's, and a page's console counted them as its own.

use serde_json::Value;

/// The most texts of one line; the rest are dropped.
pub const MAX_LOG_VALUES: usize = 16;
/// The longest text of a line, in characters; a longer one is cut.
pub const MAX_LOG_CHARS: usize = 1000;
/// The longest stack of an error line, in characters.
pub const MAX_STACK_CHARS: usize = 4000;

/// A line's level, by the console's names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LogLevel {
    Error,
    Warning,
    Info,
    Log,
    Debug,
}

impl LogLevel {
    /// The level named `name` (`error`, `warning`, `info`, `log`, `debug`).
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "error" => Self::Error,
            "warning" => Self::Warning,
            "info" => Self::Info,
            "log" => Self::Log,
            "debug" => Self::Debug,
            _ => return None,
        })
    }

    /// Its name, as [`LogLevel::parse`] reads it.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Info => "info",
            Self::Log => "log",
            Self::Debug => "debug",
        }
    }
}

/// One line a page script logged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageLog {
    pub level: LogLevel,
    /// The values as the prologue wrote them: an error's message, a text, JSON.
    pub values: Vec<String>,
    /// An error line's stack: a page script's throw, or an error it logged (review of
    /// RB-080-66, 4).
    pub stack: Option<String>,
}

impl PageLog {
    /// The line in `value`, at most [`MAX_LOG_VALUES`] texts of at most [`MAX_LOG_CHARS`] and a
    /// stack of at most [`MAX_STACK_CHARS`]; none when it is no object with a known level and a
    /// list of texts, or its stack is no text.
    #[must_use]
    pub fn from_value(value: Value) -> Option<Self> {
        let level = LogLevel::parse(value.get("level")?.as_str()?)?;
        let values = value
            .get("values")?
            .as_array()?
            .iter()
            .map(|item| {
                item.as_str()
                    .map(|text| text.chars().take(MAX_LOG_CHARS).collect())
            })
            .collect::<Option<Vec<String>>>()?;
        let stack = match value.get("stack") {
            None | Some(Value::Null) => None,
            Some(stack) => Some(stack.as_str()?.chars().take(MAX_STACK_CHARS).collect()),
        };
        Some(Self {
            level,
            values: values.into_iter().take(MAX_LOG_VALUES).collect(),
            stack,
        })
    }
}

#[cfg(test)]
#[path = "log_tests.rs"]
mod tests;
