//! `engines.external`: the external windows of Firefox and WebKit (RB-040-01);
//! `engines.firefox`: the installed Firefox over WebDriver BiDi (RB-040-02); `engines.webkit`:
//! WebKit through Playwright (RB-040-04); `engines.cookies`: copying Chromium's cookies into
//! them (RB-040-05).

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::EngineSettings;

/// `engines.external`: whether the windows of Firefox and WebKit views sit in their grid cells
/// (RB-080-39), where a window of its own goes, whether a crashed one restarts, and whether
/// the grid shows their placeholder.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct ExternalWindowSettings {
    /// Where a new external window goes: `beside` the main window, on `monitor:<n>` (1-based),
    /// or where it was last (`remember`); further windows cascade.
    #[schemars(regex(pattern = r"^(beside|remember|monitor:[1-9][0-9]?)$"))]
    pub placement: String,
    /// An external window whose engine crashed starts again by itself with its last page, at
    /// most three times in a row with a growing wait; a window you closed stays closed until
    /// you open it again (RB-080-38; `autoRestart` before 0.8).
    #[serde(alias = "autoRestart")]
    pub restart_on_crash: bool,
    /// The grid shows each external view's status with the buttons "Show window" and "Restart".
    pub show_tiles: bool,
    /// A Firefox or WebKit view sits in its grid cell like a Chromium view: its window is owned
    /// by the main window, has no frame, tabs or address bar and follows the cell (RB-080-39).
    /// Off (the default since the owner's test of 2026-11-13: the window shows the page at
    /// 100 % cut to the cell, and WebKit's shell keeps its chrome), every such view is a window
    /// of its own beside the main window.
    pub embed: bool,
    /// A view in its cell shows its page at the breakpoint's width scaled to the cell, as a
    /// Chromium view does (RB-080-40): WebKit through its context's scale factor, every
    /// Firefox view at the one factor of the profile. Off, the page is shown at 100 % and cut
    /// to the cell. Nothing without `embed`.
    pub scale_to_cell: bool,
}

impl Default for ExternalWindowSettings {
    fn default() -> Self {
        Self {
            placement: "beside".to_owned(),
            restart_on_crash: true,
            show_tiles: true,
            embed: false,
            scale_to_cell: true,
        }
    }
}

/// `engines.firefox`: the installed Firefox rBrowser drives over WebDriver BiDi (RB-040-02).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct FirefoxSettings {
    /// The Firefox executable; found automatically when empty (the registry's App Paths entry,
    /// then the Mozilla Firefox folder under Program Files or the local app data).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// The folder of rBrowser's Firefox profile, absolute or relative to rBrowser's local folder;
    /// a folder per project in the local folder when empty, never inside the project. A folder
    /// that is not empty and not rBrowser's (a profile of your own Firefox), or a link, is refused.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_dir: Option<String>,
    /// The Firefox windows are a sync source too: a click in one makes it the active view and
    /// the other views follow it.
    pub sync_source: bool,
    /// Extra Firefox prefs written to the profile's `user.js` at every start, name to `true`,
    /// `false`, an integer or a string - for what WebDriver BiDi cannot emulate per view
    /// (`ui.prefersReducedMotion: 1`). The remote agent's `remote.*` prefs are refused.
    pub extra_prefs: BTreeMap<String, Value>,
}

impl Default for FirefoxSettings {
    fn default() -> Self {
        Self {
            path: None,
            profile_dir: None,
            sync_source: true,
            extra_prefs: BTreeMap::new(),
        }
    }
}

/// `engines.webkit`: WebKit through Playwright (RB-040-04). The label "WebKit (not Safari)" has
/// no switch.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct WebkitSettings {
    /// Breakpoints on WebKit open in a WebKit window (Playwright's WebKit build, not Safari);
    /// off, they stay with Chromium. `engine.webkit.toggle` switches it for the session.
    pub enabled: bool,
    /// Input in a WebKit window is synced to the other views while it is the active view;
    /// off, WebKit windows only follow.
    pub sync_source: bool,
}

impl Default for WebkitSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            sync_source: true,
        }
    }
}

/// `engines.cookies`: copying the cookies of a Chromium session into the Firefox and WebKit
/// views of that session (RB-040-05), so a login in the grid holds there too.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct CookieSettings {
    /// When the cookies are copied: by hand (`manual`, the command "Copy cookies"), into each
    /// Firefox or WebKit view as it starts (`onStart`), or after each page load of a Chromium
    /// view into the Firefox and WebKit views of its session (`onNavigate`).
    #[schemars(
        regex(pattern = r"^(manual|onStart|onNavigate)$"),
        extend("enum" = ["manual", "onStart", "onNavigate"])
    )]
    pub copy: String,
    /// Which cookies: those of the sites the views show and of the current environment
    /// (`environment`), or every cookie of the session (`all`).
    #[schemars(
        regex(pattern = r"^(environment|all)$"),
        extend("enum" = ["environment", "all"])
    )]
    pub domain_filter: String,
    /// Cookies never copied: a cookie name (`*` matches any text, `_ga*`) or a domain, which
    /// covers its subdomains (`tracker.example`).
    pub exclude: Vec<String>,
    /// The Firefox and WebKit views that got cookies load their page again, so it sees them.
    pub reload: bool,
}

impl Default for CookieSettings {
    fn default() -> Self {
        Self {
            copy: "manual".to_owned(),
            domain_filter: "environment".to_owned(),
            exclude: Vec::new(),
            reload: true,
        }
    }
}

impl EngineSettings {
    /// The Firefox executable set by the person: `engines.firefox.path`, else the key before
    /// RB-040-02, `engines.firefoxPath` - trimmed, blank is unset. The one reading of the
    /// setting, so About, the start and the list agree on the file (RB-090-54).
    pub fn firefox_executable(&self) -> Option<&str> {
        [&self.firefox.path, &self.firefox_path]
            .into_iter()
            .find_map(|path| {
                path.as_deref()
                    .map(str::trim)
                    .filter(|path| !path.is_empty())
            })
    }
}

#[cfg(test)]
#[path = "user_engines_tests.rs"]
mod tests;
