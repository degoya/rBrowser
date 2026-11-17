//! Settings that name a program rBrowser starts, checked when they are saved (RB-080-58): an
//! absolute path to a file that exists, so a wrong choice is refused with a sentence at once
//! instead of failing at the next start. Empty or a reset leaves the choice to rBrowser. The
//! sidecar's start checks again (`rb_sidecar::process::lighthouse_browser`).

use std::path::Path;

use serde_json::Value;

/// `engines.lighthouseBrowserPath`.
pub const LIGHTHOUSE_BROWSER_KEY: &str = rb_sidecar::settings::KEY_LIGHTHOUSE_BROWSER;

/// The i18n keys of the refusals.
pub mod reason {
    /// Not an absolute path.
    pub const NOT_ABSOLUTE: &str = "settings.rejected.program_not_absolute";
    /// No file at the path.
    pub const NOT_A_FILE: &str = "settings.rejected.program_not_a_file";
}

/// Why `value` cannot be saved at `key`, when `key` names a program; `is_file` looks at the disk.
pub fn refusal(key: &str, value: &Value, is_file: impl Fn(&Path) -> bool) -> Option<&'static str> {
    if key != LIGHTHOUSE_BROWSER_KEY {
        return None;
    }
    let path = value
        .as_str()
        .map(str::trim)
        .filter(|path| !path.is_empty())?;
    let path = Path::new(path);
    if !path.is_absolute() {
        Some(reason::NOT_ABSOLUTE)
    } else if !is_file(path) {
        Some(reason::NOT_A_FILE)
    } else {
        None
    }
}

#[cfg(test)]
#[path = "program_tests.rs"]
mod tests;
