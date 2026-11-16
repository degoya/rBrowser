//! The facts of the About page and of MCP's `app_info`: which build runs, on which engines and
//! API versions, and where it keeps its data. One function builds them, so the page, the copied
//! version info and an agent never disagree.
//!
//! The version is the workspace's (`CARGO_PKG_VERSION`); the commit and the build time are what
//! `build.rs` stamped (`RB_BUILD_COMMIT`, `RB_BUILD_TIME`); CEF and Chromium come from the pinned
//! CEF build `build.rs` embeds (`RB_CEF_BUILD_VERSION`, `156.0.26+g8ea0c73+chromium-156.0.8078.88`),
//! which is the `libcef.dll` the package ships beside the binary (its SHA-256 is pinned in
//! `scripts/lib/tool-versions.sh`). Firefox is added by [`super::with_firefox`] where a core
//! knows the settings (RB-090-49); WebKit/Playwright and the Node runtime (RB-040-03) are absent
//! until their jobs detect them.

use std::collections::BTreeMap;
use std::path::Path;

use rb_ipc::AboutInfo;

/// The build stamp of this binary.
pub const BUILD: Build<'static> = Build {
    version: env!("CARGO_PKG_VERSION"),
    commit: env!("RB_BUILD_COMMIT"),
    built: env!("RB_BUILD_TIME"),
    cef: env!("RB_CEF_BUILD_VERSION"),
};

/// The plugin contract's package name (`rb_plugin_api::PACKAGE`).
const PLUGIN_PACKAGE: &str = "rbrowser:plugin";

/// What a binary was built with, as `build.rs` stamps it.
#[derive(Clone, Copy, Debug)]
pub struct Build<'a> {
    /// The workspace version.
    pub version: &'a str,
    /// Eight characters of the commit (`-dirty` when the tree differed), or `unknown`.
    pub commit: &'a str,
    /// The build time, UTC, or `unknown`.
    pub built: &'a str,
    /// The pinned CEF build, `<cef>+chromium-<chromium>`, or empty.
    pub cef: &'a str,
}

/// The folders the facts name.
#[derive(Clone, Debug, Default)]
pub struct Dirs {
    /// The roaming folder: user settings, the trust store.
    pub data: Option<String>,
    /// The local folder: the browser profile, crash dumps.
    pub local: Option<String>,
    /// The log files.
    pub logs: Option<String>,
}

impl Dirs {
    /// The folders of this machine (the portable layout included, RB-010-15).
    pub fn of_this_machine() -> Self {
        let local = rb_config::local_dir();
        Self {
            data: rb_config::roaming_dir().map(|dir| display(&dir)),
            logs: local
                .as_deref()
                .map(|dir| display(&rb_diag::DiagPaths::under(dir).logs)),
            local: local.as_deref().map(display),
        }
    }
}

fn display(path: &Path) -> String {
    path.display().to_string()
}

/// The facts of this binary on this machine.
pub fn facts() -> AboutInfo {
    facts_from(BUILD, &Dirs::of_this_machine())
}

/// The facts of `build` with the folders `dirs`.
pub fn facts_from(build: Build<'_>, dirs: &Dirs) -> AboutInfo {
    let (cef, chromium) = split_cef(build.cef);
    let mut apis = BTreeMap::new();
    apis.insert("plugin".to_owned(), plugin_api());
    apis.insert(
        "configSchema".to_owned(),
        rb_config::CURRENT_VERSION.to_string(),
    );
    let mut folders = BTreeMap::new();
    for (name, dir) in [
        ("data", &dirs.data),
        ("local", &dirs.local),
        ("logs", &dirs.logs),
    ] {
        if let Some(dir) = dir {
            folders.insert(name.to_owned(), dir.clone());
        }
    }
    AboutInfo {
        version: build.version.to_owned(),
        commit: known(build.commit).unwrap_or_else(|| UNKNOWN.to_owned()),
        cef,
        chromium,
        firefox: None,
        webkit: None,
        node: None,
        built: known(build.built),
        platform: Some(platform()),
        firefox_path: None,
        apis,
        dirs: folders,
    }
}

/// What a fact nobody knows reads as.
pub const UNKNOWN: &str = "unknown";

/// `value`, or `None` for what `build.rs` writes when it does not know (`unknown`) or nothing.
fn known(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty() && value != UNKNOWN).then(|| value.to_owned())
}

/// CEF's and Chromium's version from the CEF build string `156.0.26+g8ea0c73+chromium-156.0.8078.88`;
/// `unknown` for what it does not name.
pub fn split_cef(build: &str) -> (String, String) {
    let build = build.trim();
    match build.split_once("+chromium-") {
        Some((cef, chromium)) if !cef.is_empty() && !chromium.is_empty() => {
            (cef.to_owned(), chromium.to_owned())
        }
        _ => (
            known(build).unwrap_or_else(|| UNKNOWN.to_owned()),
            UNKNOWN.to_owned(),
        ),
    }
}

/// Operating system and architecture, `windows x86_64`.
pub fn platform() -> String {
    format!("{} {}", std::env::consts::OS, std::env::consts::ARCH)
}

/// The plugin contracts this host runs, `rbrowser:plugin@0.1.0`.
fn plugin_api() -> String {
    rb_plugin_host::SUPPORTED_API_VERSIONS
        .iter()
        .map(|version| format!("{PLUGIN_PACKAGE}@{version}"))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
#[path = "facts_tests.rs"]
mod tests;
