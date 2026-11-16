//! An rBrowser export plugin: writes the pages a recording visited as a Playwright test.
//!
//! What the host promises an export plugin (docs/plugins.md):
//!
//! - `render` gets one of the formats `formats` lists and the recording as JSON; it answers one
//!   file - a name without folders and its text, at most 8 MiB. The host writes it where the
//!   person chooses.
//!
//! The recording's format is the recorder's (RB-050-02); this template reads only the `url` of
//! each step of `{"steps": [{"url": "..."}]}`.
//!
//! The logic lives here and is tested natively (`cargo test`); `src/guest.rs` only translates it
//! into the vocabulary of `wit/rbrowser-plugin.wit` and exists on `wasm32` alone.

#[cfg(target_arch = "wasm32")]
mod guest;

/// The one format.
pub const PLAYWRIGHT: &str = "playwright";

/// The test for `recording`, or why it cannot be read.
///
/// # Errors
///
/// The recording is no JSON.
pub fn render(recording: &str) -> Result<String, String> {
    let value: serde_json::Value =
        serde_json::from_str(recording).map_err(|error| error.to_string())?;
    let mut text = String::from(
        "import { test } from '@playwright/test';\n\ntest('recorded', async ({ page }) => {\n",
    );
    let steps = value["steps"].as_array().cloned().unwrap_or_default();
    for url in steps.iter().filter_map(|step| step["url"].as_str()) {
        text.push_str(&format!(
            "  await page.goto({});\n",
            serde_json::Value::String(url.to_owned())
        ));
    }
    text.push_str("});\n");
    Ok(text)
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
