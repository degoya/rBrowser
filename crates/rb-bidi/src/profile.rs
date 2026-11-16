//! The Firefox profile rBrowser starts with, a folder the app chooses (in rBrowser's local
//! folder, keyed by the project - never inside the project, whose files a clone controls) or
//! `engines.firefox.profileDir`. It is rBrowser's own: a folder is used only when it is new,
//! empty or carries the marker [`MARKER`], so the user's real Firefox profile is never touched,
//! and neither the folder nor `user.js` nor the marker may be a link (a symlink, on Windows also
//! a junction): rBrowser never writes through one. `user.js` is written on every start: the
//! person's `engines.firefox.extraPrefs` first, then rBrowser's own prefs (no first-run pages,
//! no update, no telemetry), which win. `chrome/userChrome.css` collapses the tabs and toolbars
//! ([`USER_CHROME_CSS`]); whether Firefox reads it is the pref
//! `toolkit.legacyUserProfileCustomizations.stylesheets`, on for a view in its grid cell
//! (`engines.external.embed`, RB-080-39). The pref `layout.css.devPixelsPerPx` scales every
//! window's rendering to the cells ([`user_js`]'s `render_scale`, RB-080-40, ADR 0003): one
//! factor per profile, read at start, so a changed factor starts Firefox anew.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::Value;

/// The file that marks a folder as rBrowser's Firefox profile.
pub const MARKER: &str = "rbrowser-profile.txt";
/// The pref that makes Firefox read `chrome/userChrome.css`.
const USER_CHROME_PREF: &str = "toolkit.legacyUserProfileCustomizations.stylesheets";
/// The pref that scales every window's rendering (CSS px to device px); `"-1.0"` is the
/// display's own ratio.
pub const RENDER_SCALE_PREF: &str = "layout.css.devPixelsPerPx";
/// `chrome/userChrome.css`: no tabs, address bar or bookmarks toolbar - the window is the page
/// (a view in its grid cell, RB-080-39).
pub const USER_CHROME_CSS: &str = "/* Written by rBrowser on every start; edits are lost. */\n\
#TabsToolbar, #nav-bar, #PersonalToolbar { visibility: collapse !important; }\n";

/// Why the profile cannot be used.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ProfileError {
    /// The folder holds data rBrowser did not make (perhaps a real Firefox profile).
    #[error(
        "{} is not an rBrowser Firefox profile and is not empty; choose an empty folder in \
         engines.firefox.profileDir",
        .0.display()
    )]
    NotOurs(PathBuf),
    /// The folder, `user.js` or the marker is a link (a symlink or junction).
    #[error("{} is a link; rBrowser's Firefox profile must be a plain folder", .0.display())]
    Link(PathBuf),
    /// Reading or writing failed.
    #[error("the Firefox profile cannot be prepared: {0}")]
    Io(String),
    /// An entry of `engines.firefox.extraPrefs` cannot be written.
    #[error("engines.firefox.extraPrefs: {0}")]
    InvalidPref(String),
}

/// rBrowser's own prefs: a quiet, private, stable browser for testing.
const OWN_PREFS: &[(&str, &str)] = &[
    ("browser.shell.checkDefaultBrowser", "false"),
    ("browser.aboutwelcome.enabled", "false"),
    ("browser.startup.homepage_override.mstone", "\"ignore\""),
    ("startup.homepage_welcome_url", "\"\""),
    ("startup.homepage_welcome_url.additional", "\"\""),
    ("browser.startup.page", "0"),
    ("browser.tabs.warnOnClose", "false"),
    ("browser.warnOnQuit", "false"),
    ("browser.sessionstore.resume_from_crash", "false"),
    ("app.update.auto", "false"),
    ("app.update.disabledForTesting", "true"),
    ("app.normandy.enabled", "false"),
    ("extensions.update.enabled", "false"),
    ("datareporting.policy.dataSubmissionEnabled", "false"),
    ("datareporting.healthreport.uploadEnabled", "false"),
    ("toolkit.telemetry.enabled", "false"),
    ("toolkit.telemetry.unified", "false"),
    ("toolkit.telemetry.reportingpolicy.firstRun", "false"),
    ("browser.discovery.enabled", "false"),
];

/// `user.js`: `extra` first, rBrowser's own prefs last (the last line of a pref wins);
/// `hide_chrome` switches `userChrome.css` on (a view in its cell) or off (a window of its own);
/// `render_scale` is the factor every window renders CSS px to device px at (the views scaled
/// into their cells, RB-080-40), `None` the display's own ratio.
pub fn user_js(
    extra: &BTreeMap<String, Value>,
    hide_chrome: bool,
    render_scale: Option<f64>,
) -> Result<String, ProfileError> {
    let mut text = String::from("// Written by rBrowser on every start; edits are lost.\n");
    for (name, value) in extra {
        let value = pref_value(name, value)?;
        text.push_str(&format!("user_pref(\"{name}\", {value});\n"));
    }
    for (name, value) in OWN_PREFS {
        text.push_str(&format!("user_pref(\"{name}\", {value});\n"));
    }
    text.push_str(&format!(
        "user_pref(\"{USER_CHROME_PREF}\", {hide_chrome});\n"
    ));
    let scale =
        render_scale.map_or_else(|| "-1.0".to_owned(), rb_engine::external::scale::pref_value);
    text.push_str(&format!(
        "user_pref(\"{RENDER_SCALE_PREF}\", \"{scale}\");\n"
    ));
    Ok(text)
}

/// Makes `dir` rBrowser's profile (when it is new, empty or already ours) and writes `user.js`
/// and `chrome/userChrome.css`; `hide_chrome` says whether Firefox reads the latter,
/// `render_scale` how every window renders ([`user_js`]).
pub fn prepare(
    dir: &Path,
    extra: &BTreeMap<String, Value>,
    hide_chrome: bool,
    render_scale: Option<f64>,
) -> Result<(), ProfileError> {
    let user_js = user_js(extra, hide_chrome, render_scale)?;
    let io = |error: std::io::Error| ProfileError::Io(error.kind().to_string());
    let chrome = dir.join("chrome");
    refuse_link(dir)?;
    refuse_link(&dir.join(MARKER))?;
    refuse_link(&dir.join("user.js"))?;
    refuse_link(&chrome)?;
    refuse_link(&chrome.join("userChrome.css"))?;
    if !dir.join(MARKER).is_file() {
        let empty = match std::fs::read_dir(dir) {
            Ok(mut entries) => entries.next().is_none(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => true,
            Err(error) => return Err(io(error)),
        };
        if !empty {
            return Err(ProfileError::NotOurs(dir.to_path_buf()));
        }
        std::fs::create_dir_all(dir).map_err(io)?;
        std::fs::write(
            dir.join(MARKER),
            "The Firefox profile of rBrowser (RB-040-02). Delete the folder to start afresh.\n",
        )
        .map_err(io)?;
    }
    std::fs::create_dir_all(&chrome).map_err(io)?;
    std::fs::write(chrome.join("userChrome.css"), USER_CHROME_CSS).map_err(io)?;
    std::fs::write(dir.join("user.js"), user_js).map_err(io)
}

/// Refuses `path` when it is a link: a symlink, or on Windows any name-surrogate reparse point
/// (a junction too - `FileType::is_symlink` covers both). A missing path is fine.
fn refuse_link(path: &Path) -> Result<(), ProfileError> {
    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => Err(ProfileError::Link(path.to_path_buf())),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(ProfileError::Io(error.kind().to_string())),
    }
}

/// The JavaScript literal of one extra pref: a boolean, a 32-bit integer or a string.
fn pref_value(name: &str, value: &Value) -> Result<String, ProfileError> {
    let valid_name = !name.is_empty()
        && name.len() <= 200
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
    if !valid_name {
        return Err(ProfileError::InvalidPref(
            "a pref name has only letters, digits, '.', '_' and '-'".to_owned(),
        ));
    }
    if name.starts_with("remote.") {
        return Err(ProfileError::InvalidPref(format!(
            "{name}: the remote agent's prefs are rBrowser's"
        )));
    }
    match value {
        Value::Bool(flag) => Ok(flag.to_string()),
        Value::Number(number) => number
            .as_i64()
            .and_then(|n| i32::try_from(n).ok())
            .map(|n| n.to_string())
            .ok_or_else(|| ProfileError::InvalidPref(format!("{name}: a 32-bit integer"))),
        // A JSON string is a JavaScript string literal.
        Value::String(_) => Ok(value.to_string()),
        _ => Err(ProfileError::InvalidPref(format!(
            "{name}: true, false, an integer or a string"
        ))),
    }
}

#[cfg(test)]
#[path = "profile_tests.rs"]
mod tests;
