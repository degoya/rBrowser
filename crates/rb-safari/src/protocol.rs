//! What `safaridriver` answers (W3C WebDriver, "Handling errors"): `{"value": ...}` with a 2xx
//! status on success, `{"value": {"error", "message", "stacktrace"}}` otherwise. The stack trace
//! is dropped and the message cut short; a refused new session is sorted into "remote automation
//! is off" ([`SafariError::NotEnabled`]), "another session runs" ([`SafariError::SessionBusy`])
//! or a plain refusal. Every answer is untrusted input.

use serde_json::Value;

use crate::error::SafariError;
use crate::http::{Answer, is_plain_segment};

/// The key of a web element reference (W3C WebDriver, "Elements").
pub const ELEMENT_KEY: &str = "element-6066-11e4-a52e-4f735466cecf";
/// The longest driver message kept, in characters.
pub const MAX_MESSAGE: usize = 300;

/// The value of `answer` to `command`, or why it failed.
pub fn outcome(command: &str, answer: Answer) -> Result<Value, SafariError> {
    let Answer { status, mut body } = answer;
    let value = match &mut body {
        Value::Object(object) => object.remove("value").unwrap_or(Value::Null),
        _ => Value::Null,
    };
    if (200..300).contains(&status) {
        return Ok(value);
    }
    let text = |key: &str| value.get(key).and_then(Value::as_str).map(str::to_owned);
    let error = text("error").unwrap_or_else(|| "unknown error".to_owned());
    let message = rb_util::text::cut_ellipsis(&text("message").unwrap_or_default(), MAX_MESSAGE);
    if error == "session not created" {
        if remote_automation_off(&message) {
            return Err(SafariError::NotEnabled);
        }
        if another_session(&message) {
            return Err(SafariError::SessionBusy);
        }
    }
    Err(SafariError::Command {
        command: command.to_owned(),
        error,
        message,
    })
}

/// Whether a refused session says remote automation is off (`safaridriver --enable` never ran,
/// or Develop > Allow Remote Automation is off).
fn remote_automation_off(message: &str) -> bool {
    let message = message.to_ascii_lowercase();
    message.contains("remote automation") || message.contains("--enable")
}

/// Whether a refused session says Safari runs another automated session.
fn another_session(message: &str) -> bool {
    let message = message.to_ascii_lowercase();
    message.contains("already paired")
        || message.contains("another session")
        || (message.contains("already") && message.contains("automat"))
}

/// The id of a web element reference, when it is plain enough for a request path.
pub fn element_id(value: &Value) -> Option<String> {
    value
        .get(ELEMENT_KEY)
        .and_then(Value::as_str)
        .filter(|id| is_plain_segment(id))
        .map(str::to_owned)
}

#[cfg(test)]
#[path = "protocol_tests.rs"]
mod tests;
