//! `x-restart` (RB-094-23): the keys whose change applies only at the next start of rBrowser,
//! each for a reason the running app cannot get round - every other key applies at once. The
//! settings window shows such a key's "Applies after a restart" with its "Restart now" button;
//! `scripts/lib/settings_restart.py` holds every key to one of the two classes (a text that says
//! the next start needs the annotation, and the annotation needs a key of this list).

/// The schema annotation of a key that applies at the next start.
pub const X_RESTART: &str = "x-restart";

/// The keys that apply at the next start, with the reason.
pub const RESTART_KEYS: &[&str] = &[
    // CEF's process sandbox is chosen when the browser process starts (`CefSettings`).
    "engines.chromium.sandbox",
    // Extensions load only through `--load-extension` before CEF initialises.
    "engines.chromium.extensions.enabled",
    "engines.chromium.extensions.unpacked",
    // `CefSettings.persist_session_cookies`, fixed when the request contexts are made.
    "sessions.persistSessionCookies",
    // Chromium command-line switches (`launch::browser_switches`), read once at start.
    "network.hostResolverRules",
    "network.proxy",
    "network.ignoreCertificateErrors",
];
