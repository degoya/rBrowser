//! The script a snippet runs as (RB-070-04). The snippet's code is the body of an async function,
//! so `return value` and `await` work. It is called at once inside the evaluation, so its
//! synchronous part is cut by the CDP `timeout` like any endless loop; its promise is raced
//! against the snippet's timeout; the outcome becomes JSON a person can read: cyclic objects
//! marked, DOM nodes described (`<div#app.main>`), errors with their stack.
//!
//! The snippet's function is defined in the script's own scope, beside nothing but the page's
//! globals: the helpers live in the function it is handed to, so a snippet never sees (or
//! shadows) one of them.

/// The longest text of a result, as the panel shows it.
pub const MAX_TEXT_CHARS: usize = 2_000;
/// The largest JSON value of one view's result; a bigger one keeps its text only.
pub const MAX_VALUE_BYTES: usize = 32 * 1024;

/// The wrapper: `__LIMIT__` (milliseconds), `__MAX_TEXT__` and `__MAX_VALUE__` filled in, then
/// the code, then [`EPILOGUE`].
const WRAPPER: &str = include_str!("wrapper.js");

const EPILOGUE: &str = "\n})";

/// The script that runs `code` with a timeout of `timeout_ms` milliseconds.
#[must_use]
pub fn wrap(code: &str, timeout_ms: u32) -> String {
    let wrapper = WRAPPER
        .replace("__LIMIT__", &timeout_ms.to_string())
        .replace("__MAX_TEXT__", &MAX_TEXT_CHARS.to_string())
        .replace("__MAX_VALUE__", &MAX_VALUE_BYTES.to_string());
    let mut script = String::with_capacity(wrapper.len() + code.len() + EPILOGUE.len());
    script.push_str(&wrapper);
    script.push_str(code);
    script.push_str(EPILOGUE);
    script
}

#[cfg(test)]
#[path = "script_tests.rs"]
mod tests;
