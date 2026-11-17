//! What an outside client may send. Most commands pass unchanged; a few are rBrowser's alone.
//!
//! **Emulation is rBrowser's** (ADR 0001 Decision 3, RB-010-04): every view's device metrics
//! are its breakpoint (width, `scale = min(1, cellWidth / breakpoint)`), re-applied on resize
//! and reconnect. A client that set them would break the grid's layout and every input mapping
//! until the next resize, so the metric commands ([`OWNED`]) are answered with an empty result
//! and never sent: a Playwright script keeps running, the view keeps its breakpoint. That is
//! why `page.setViewportSize` (and a context `viewport`) has no effect through rBrowser - pick
//! the view of the size instead. The media features are rBrowser's too (RB-040-06): Playwright
//! sets `prefers-color-scheme: light` on every page when it connects, which would turn a view
//! emulating dark mode light, so `Emulation.setEmulatedMedia` is answered the same way - set
//! emulation in rBrowser or with MCP `emulation_set`. So is the download behaviour
//! (`Browser.setDownloadBehavior`): it is browser-wide, and Playwright sets it to `allowAndName`
//! into its own temp folder when it connects without `noDefaults`, which would take rBrowser's
//! downloads with it. Other emulation a client sets (touch, user agent) is undone when its
//! connection ends: the app re-applies each touched view's emulation.
//!
//! **The app's own surface** is refused ([`verdict`] names why): closing or crashing the
//! browser, making or closing targets outside the grid, a second browser session or a page
//! binding that would see past the filter, non-flat sessions, port forwarding.

/// What happens to a client's command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Sent on the client's session.
    Forward,
    /// rBrowser's: answered `{}` and not sent.
    Owned,
    /// Refused with the reason.
    Refuse(&'static str),
}

/// The device-metric, media and download commands rBrowser owns.
pub const OWNED: &[&str] = &[
    "Emulation.setEmulatedMedia",
    "Emulation.setDeviceMetricsOverride",
    "Emulation.clearDeviceMetricsOverride",
    "Emulation.setVisibleSize",
    "Page.setDeviceMetricsOverride",
    "Page.clearDeviceMetricsOverride",
    "Browser.setWindowBounds",
    "Browser.setContentsSize",
    "Browser.setDownloadBehavior",
];

/// The refused commands and why.
const REFUSED: &[(&str, &str)] = &[
    ("Browser.close", "rBrowser stays open: disconnect instead"),
    ("Browser.crash", "not allowed through rBrowser's proxy"),
    (
        "Browser.crashGpuProcess",
        "not allowed through rBrowser's proxy",
    ),
    (
        "Target.createTarget",
        "rBrowser makes its views itself: add a view in rBrowser",
    ),
    (
        "Target.closeTarget",
        "rBrowser's views are closed in rBrowser",
    ),
    (
        "Target.createBrowserContext",
        "new contexts cannot get pages: use the default context's views",
    ),
    (
        "Target.disposeBrowserContext",
        "not allowed through rBrowser's proxy",
    ),
    (
        "Target.attachToBrowserTarget",
        "this connection is a browser session already",
    ),
    (
        "Target.exposeDevToolsProtocol",
        "not allowed through rBrowser's proxy",
    ),
    (
        "Target.sendMessageToTarget",
        "only flat sessions are supported (attachToTarget with flatten: true)",
    ),
    (
        "Target.setRemoteLocations",
        "not allowed through rBrowser's proxy",
    ),
    ("Tethering.bind", "rBrowser opens no ports for clients"),
];

/// What happens to `method`.
#[must_use]
pub fn verdict(method: &str) -> Verdict {
    if OWNED.contains(&method) {
        return Verdict::Owned;
    }
    REFUSED
        .iter()
        .find(|(name, _)| *name == method)
        .map_or(Verdict::Forward, |(_, reason)| Verdict::Refuse(reason))
}

#[cfg(test)]
#[path = "policy_tests.rs"]
mod tests;
