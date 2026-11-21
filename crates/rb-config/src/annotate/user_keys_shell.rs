//! The last user settings keys - the shell's strips: the activity bar (RB-090-67), the view
//! footer and the label strips, then the file name time and the title bar (RB-094-19), the
//! Safari, Android and iOS engine keys, the portal's presentation over the internet (RB-096-05)
//! and the framework DevTools - split from `user_keys.rs` (500-line rule); `USER_KEYS` lists
//! them after the others, joined by [`concat`] at compile time.

use rb_settings::Category;

use crate::annotate::{KeyDef, key, protected};

/// `first`, then `second`, as one array of `N` keys (`N` their lengths together).
pub(super) const fn concat<const N: usize>(first: &[KeyDef], second: &[KeyDef]) -> [KeyDef; N] {
    let mut all = [key("", Category::General, ""); N];
    let mut index = 0;
    while index < first.len() {
        all[index] = first[index];
        index += 1;
    }
    let mut next = 0;
    while next < second.len() {
        all[first.len() + next] = second[next];
        next += 1;
    }
    all
}

/// The keys of the shell's strips, the file name time and Safari's, in their order.
pub const SHELL_KEYS: &[KeyDef] = &[
    // The activity bar while something runs (RB-090-67), beside the toolbar's switches.
    key("ui.activityBar.position", Category::General, "appearance"),
    key("ui.activityBar.mode", Category::General, "appearance"),
    key("ui.activityBar.show.video", Category::General, "appearance"),
    key(
        "ui.activityBar.show.recorder",
        Category::General,
        "appearance",
    ),
    key("ui.activityBar.show.flows", Category::General, "appearance"),
    key(
        "ui.activityBar.show.checks",
        Category::General,
        "appearance",
    ),
    key(
        "ui.activityBar.show.portal",
        Category::General,
        "appearance",
    ),
    key(
        "ui.activityBar.show.desync",
        Category::General,
        "appearance",
    ),
    key("ui.activityBar.show.time", Category::General, "appearance"),
    key(
        "ui.activityBar.show.plugins",
        Category::General,
        "appearance",
    ),
    key(
        "ui.activityBar.show.forcedStates",
        Category::General,
        "appearance",
    ),
    // The vitals bar under every view (RB-090-71, the label strip's vitals of RB-090-27 before),
    // beside the breakpoint bar's keys; what the label strips show beside the name (RB-090-27).
    key("ui.viewFooter.vitals", Category::Layout, "grid"),
    // The Lighthouse bar below the vitals bar (RB-090-73).
    key("ui.viewFooter.lighthouse", Category::Layout, "grid"),
    key("ui.cellLabel.engineStatus", Category::Layout, "grid"),
    key("ui.cellLabel.address", Category::Layout, "grid"),
    // The address field of a decoupled view's label strip (RB-092-03).
    key("ui.cellLabel.addressEditable", Category::Layout, "grid"),
    // An action's error at the top as well (RB-094-20), beside the status bar's keys.
    key(
        "ui.notifications.errorToast",
        Category::Layout,
        "status-bar",
    ),
    // The time zone of the names a person reads (RB-090-69), beside the language.
    key("ui.fileNameTime", Category::General, "language"),
    // The main window's title: the version and the credit line (RB-094-19), beside the theme.
    key("ui.titleBar.showVersion", Category::General, "appearance"),
    key("ui.titleBar.showCredit", Category::General, "appearance"),
    // Safari over safaridriver on macOS (RB-095-03); its section follows WebKit's.
    key("engines.safari.enabled", Category::Engines, "safari"),
    key(
        "engines.safari.technologyPreview",
        Category::Engines,
        "safari",
    ),
    // Chrome on an Android device over ADB (RB-095-02): what may run on the machine.
    protected("engines.android.enabled", Category::Engines, "android"),
    protected("engines.android.adbPath", Category::Engines, "android"),
    // The device reaches this machine's loopback port (RB-096-12): it widens what reaches a
    // local server, so only the person sets it.
    protected(
        "engines.android.reverseLocalhost",
        Category::Engines,
        "android",
    ),
    // iOS over safaridriver on macOS (RB-095-04): Simulator or device, after Safari's section.
    key("engines.ios.mode", Category::Engines, "ios"),
    key("engines.ios.defaultSimulator", Category::Engines, "ios"),
    key("engines.ios.bootTimeoutSec", Category::Engines, "ios"),
    // The presentation over the internet (RB-096-05), after the portal's own keys: each one
    // widens what reaches the internet, so none is MCP's to set.
    protected("portal.presentation.enabled", Category::Security, "portal"),
    protected(
        "portal.presentation.consented",
        Category::Security,
        "portal",
    ),
    protected(
        "portal.presentation.linkTtlSec",
        Category::Security,
        "portal",
    ),
    protected(
        "portal.presentation.maxViewers",
        Category::Security,
        "portal",
    ),
    protected(
        "portal.presentation.allowPaths",
        Category::Security,
        "portal",
    ),
    protected(
        "portal.presentation.denyPaths",
        Category::Security,
        "portal",
    ),
    protected("portal.presentation.pointer", Category::Security, "portal"),
    protected("tools.cloudflaredPath", Category::Security, "portal"),
    // The Vue and React DevTools backends in the views' pages (RB-080-24).
    key(
        "frameworkDevtools.vue",
        Category::Devtools,
        "framework-devtools",
    ),
    key(
        "frameworkDevtools.react",
        Category::Devtools,
        "framework-devtools",
    ),
];
