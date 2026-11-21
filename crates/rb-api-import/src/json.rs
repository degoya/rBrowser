//! Small readers over `serde_json::Value`: every format's file is read leniently, a missing or
//! mistyped field is empty rather than a failure.

use serde_json::Value;

/// The text at `key`: a string as it is, a number or boolean as its text, else empty.
pub fn text<'a>(value: &'a Value, key: &str) -> std::borrow::Cow<'a, str> {
    as_text(&value[key])
}

/// `value` as text: a string as it is, a number or boolean as its text, else empty.
pub fn as_text(value: &Value) -> std::borrow::Cow<'_, str> {
    match value {
        Value::String(text) => std::borrow::Cow::Borrowed(text.as_str()),
        Value::Number(number) => std::borrow::Cow::Owned(number.to_string()),
        Value::Bool(flag) => std::borrow::Cow::Owned(flag.to_string()),
        _ => std::borrow::Cow::Borrowed(""),
    }
}

/// The array at `key`, empty when there is none.
pub fn list<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    value[key].as_array().map_or(&[], Vec::as_slice)
}

/// Whether `key` is `true`.
pub fn flag(value: &Value, key: &str) -> bool {
    value[key].as_bool() == Some(true)
}

/// JSON text or, failing that, YAML.
///
/// # Errors
///
/// Neither reads.
pub fn json_or_yaml(text: &str) -> Result<Value, String> {
    match serde_json::from_str(text) {
        Ok(value) => Ok(value),
        Err(json) => {
            let trimmed = text.trim_start();
            if trimmed.starts_with('{') || trimmed.starts_with('[') {
                Err(format!("not JSON: {json}"))
            } else {
                crate::yaml::parse(text)
            }
        }
    }
}
