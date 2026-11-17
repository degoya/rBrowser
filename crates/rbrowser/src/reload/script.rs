//! The scripts hot reload runs in a page with `Runtime.evaluate`: the swap of stylesheets and
//! images (`swap.js`), the probe for a dev server's own HMR client and the probe for typing.
//! The arguments go in as JSON, so no file name is ever read as code. The swap runs in an
//! isolated world of its own ([`RELOAD_WORLD`]), so page code can neither patch nor read it; the
//! probes only read the page's own globals and run in it.

use serde_json::{Value, json};

/// The isolated world the swap runs in (`Page.createIsolatedWorld`, made once per document and
/// found again by its name).
pub const RELOAD_WORLD: &str = "rbrowser-reload";

/// The swap function; called with `{ css, images, allCss }`, it returns the number swapped.
const SWAP: &str = include_str!("swap.js");

/// Whether the page has an HMR client of its own (Vite, webpack), which updates it on change.
pub const HMR_PROBE: &str = "(function () {\n  \
  if (document.querySelector('script[src*=\"/@vite/client\"]')) return true;\n  \
  if (document.querySelector('script[src*=\"webpack-dev-server\"]')) return true;\n  \
  if (window.__vite_plugin_react_preamble_installed__ !== undefined) return true;\n  \
  if (window.__webpack_dev_server_client__ !== undefined || window.__webpack_hmr !== undefined) \
  return true;\n  \
  for (var key in window) { if (key.indexOf('webpackHotUpdate') === 0) return true; }\n  \
  return false;\n})()";

/// Whether someone is typing in the page: it has focus and a text field, a select or an editable
/// element is focused.
pub const TYPING_PROBE: &str = "(function () {\n  \
  var el = document.activeElement;\n  \
  if (!el || !document.hasFocus()) return false;\n  \
  if (el.isContentEditable || el.tagName === 'TEXTAREA' || el.tagName === 'SELECT') return true;\n  \
  if (el.tagName !== 'INPUT') return false;\n  \
  var type = (el.type || '').toLowerCase();\n  \
  return ['button', 'submit', 'reset', 'checkbox', 'radio', 'image', 'hidden', 'range', \
  'color', 'file'].indexOf(type) < 0;\n})()";

/// The expression that swaps `css` and `images` (file names); `all_css` swaps every
/// stylesheet whatever its name ("reload CSS only").
#[must_use]
pub fn swap_expression(css: &[String], images: &[String], all_css: bool) -> String {
    let args = json!({ "css": css, "images": images, "allCss": all_css });
    format!("({})({args})", SWAP.trim_end())
}

/// The parameters of `Runtime.evaluate` for `expression`, its value returned as JSON.
#[must_use]
pub fn evaluate_params(expression: &str) -> Value {
    json!({
        "expression": expression,
        "returnByValue": true,
        "awaitPromise": false,
        "silent": true,
    })
}

/// The value of a `Runtime.evaluate` result; `None` when the script threw.
#[must_use]
pub fn evaluate_value(result: &Value) -> Option<&Value> {
    if result.get("exceptionDetails").is_some() {
        return None;
    }
    result.get("result")?.get("value")
}

#[cfg(test)]
#[path = "script_tests.rs"]
mod tests;
