//! The key table of the user settings (`settings.json`), moved out of `annotate` and
//! re-exported there.

use rb_settings::Category;

use super::{KeyDef, key, protected};

/// The keys of the user settings (`settings.json`).
pub const USER_KEYS: &[KeyDef] = &[
    key("ui.language", Category::General, "language"),
    key("ui.theme", Category::General, "appearance"),
    // `project_open` opens one of them: MCP may not add a folder of its choice.
    protected("recentProjects", Category::General, "recent-projects"),
    // Start behaviour (RB-010-11).
    key("startup.openLastProject", Category::General, "startup"),
    key("startup.focusOnOpen", Category::General, "startup"),
    // The setup wizard at the first start and the "Set up project" hint (RB-091-05).
    key("ui.firstRunWizard", Category::General, "startup"),
    // Zoom and focus mode of the grid (RB-010-12).
    key("grid.fitToWindow", Category::Layout, "zoom"),
    key("grid.defaultZoom", Category::Layout, "zoom"),
    key("focusMode.arrowKeys", Category::Layout, "focus-mode"),
    // The toolbar keeps the focus while the arrow keys step (RB-080-17).
    key("focusMode.keepFocus", Category::Layout, "focus-mode"),
    // Horizontal, split and canvas (RB-080-04).
    key("layoutModes.snap", Category::Layout, "modes"),
    key("layoutModes.minimap", Category::Layout, "modes"),
    key("layoutModes.minimapPosition", Category::Layout, "modes"),
    key("layoutModes.horizontalHeight", Category::Layout, "modes"),
    key("layoutModes.splitStep", Category::Layout, "modes"),
    // The status bar at the bottom of the window (RB-080-15).
    key("ui.statusBar.visible", Category::Layout, "status-bar"),
    key(
        "ui.statusBar.infoTimeoutSec",
        Category::Layout,
        "status-bar",
    ),
    key("ui.statusBar.historySize", Category::Layout, "status-bar"),
    key("layoutModes.spacePan", Category::Layout, "modes"),
    key("layoutModes.savePresetTo", Category::Layout, "modes"),
    // The active view's label strip in the accent colour (RB-080-04).
    key("ui.highlightActiveView", Category::Layout, "grid"),
    // The engine badge at the right end of every label strip (RB-080-33).
    key("ui.engineBadge", Category::Layout, "grid"),
    // A view in a window of its own (RB-080-14).
    key("popOut.keepPlaceholder", Category::Layout, "pop-out"),
    key("popOut.fitToScreen", Category::Layout, "pop-out"),
    key("popOut.rememberBounds", Category::Layout, "pop-out"),
    // Throttling and the memory warning (RB-020-09).
    key("performance.throttleHidden", Category::Layout, "grid"),
    key("performance.inactiveFps", Category::Layout, "grid"),
    key(
        "performance.memoryWarningMb",
        Category::Advanced,
        "experimental",
    ),
    // Self-update (RB-090-02): every key widens what reaches the network or replaces the
    // program, so none is MCP's to set.
    protected("updates.enabled", Category::General, "updates"),
    protected("updates.channel", Category::General, "updates"),
    protected("updates.check", Category::General, "updates"),
    protected("updates.autoInstall", Category::General, "updates"),
    // After the update keys, at the place RB-010-10 gave it.
    key("updates.cefAgeWarning", Category::General, "updates").at(10),
    protected("cdp.enabled", Category::Automation, "cdp-proxy"),
    protected("cdp.port", Category::Automation, "cdp-proxy"),
    protected("mcp.enabled", Category::Automation, "mcp"),
    // The server's port and wait (RB-030-01); every `mcp.*` key is protected from MCP itself.
    protected("mcp.port", Category::Automation, "mcp"),
    protected("mcp.navigationTimeoutMs", Category::Automation, "mcp"),
    protected("mcp.consoleLogLimit", Category::Automation, "mcp"),
    protected("mcp.networkRequestLimit", Category::Automation, "mcp"),
    protected("mcp.errorLimit", Category::Automation, "mcp"),
    protected("mcp.maxAnswerBytes", Category::Automation, "mcp"),
    // `rbrowser mcp` and the audit notice (RB-030-02, RB-030-03).
    protected("mcp.stdioAutoStart", Category::Automation, "mcp"),
    protected("mcp.notifyOnSettingChange", Category::Automation, "mcp"),
    protected("mcp.allowEvaluate", Category::Security, "evaluate"),
    // The portal (RB-080-05): every key widens what reaches the LAN, so none is MCP's to set.
    protected("portal.enabled", Category::Security, "portal"),
    protected("portal.interface", Category::Security, "portal"),
    protected("portal.port", Category::Security, "portal"),
    protected("portal.pairingTtlSec", Category::Security, "portal"),
    protected("portal.maxDevices", Category::Security, "portal"),
    // RB-090-71: the offer leads to a write of the project config, a person's to allow.
    protected("portal.offerAssetOrigins", Category::Security, "portal"),
    // Flows (RB-040-07): the folder decides which project code runs, so MCP cannot move it.
    protected("automation.flows.dir", Category::Automation, "playwright"),
    key(
        "automation.flows.parallel",
        Category::Automation,
        "playwright",
    ),
    key(
        "automation.flows.timeoutMs",
        Category::Automation,
        "playwright",
    ),
    // RB-090-76: one action's wait, below the flow's.
    key(
        "automation.flows.actionTimeoutMs",
        Category::Automation,
        "playwright",
    ),
    key(
        "automation.flows.syncDuringRun",
        Category::Automation,
        "playwright",
    ),
    key(
        "automation.flows.screenshotOnFailure",
        Category::Automation,
        "playwright",
    ),
    // RB-090-84: what a run does with the views a layout hides, and the layout it may show.
    key(
        "automation.flows.hiddenViews",
        Category::Automation,
        "playwright",
    ),
    key(
        "automation.flows.runLayout",
        Category::Automation,
        "playwright",
    ),
    // Switching the sandbox off weakens every view: never over MCP (RB-010-04).
    protected("engines.chromium.sandbox", Category::Engines, "chromium").at(10),
    // Chrome extensions (RB-080-06): a folder is code in every view - only the person names
    // one, never MCP; nor may MCP switch the extensions or the Web Store on or off (security
    // review RB-090-06, finding 2; RB-090-21). The project's offers are not protected: they
    // only offer, under workspace trust.
    protected(
        "engines.chromium.extensions.enabled",
        Category::Engines,
        "chromium",
    ),
    protected(
        "engines.chromium.extensions.unpacked",
        Category::Engines,
        "chromium",
    ),
    key(
        "engines.chromium.extensions.offerProjectRecommendations",
        Category::Engines,
        "chromium",
    ),
    // Web Store installs (RB-080-23): every install asks the person in the window (MCP) or is
    // their command; the switch itself is the person's (RB-090-21).
    protected(
        "engines.chromium.extensions.allowWebStore",
        Category::Engines,
        "chromium",
    ),
    key(
        "engines.chromium.extensions.checkUpdatesAtStart",
        Category::Engines,
        "chromium",
    ),
    // Firefox over WebDriver BiDi (RB-040-02): the program, the profile folder and the extra
    // prefs decide what runs and where it writes - never over MCP.
    protected("engines.firefox.path", Category::Engines, "firefox"),
    protected("engines.firefox.profileDir", Category::Engines, "firefox"),
    key("engines.firefox.syncSource", Category::Engines, "firefox"),
    protected("engines.firefox.extraPrefs", Category::Engines, "firefox"),
    protected("engines.nodePath", Category::Engines, "node"),
    // The Node sidecar (RB-040-03); the browsers folder decides where downloads land.
    key("engines.nodeRpcTimeoutMs", Category::Engines, "node"),
    protected(
        "engines.playwrightBrowsersPath",
        Category::Engines,
        "webkit",
    ),
    // WebKit through Playwright (RB-040-04).
    key("engines.webkit.enabled", Category::Engines, "webkit"),
    key("engines.webkit.syncSource", Category::Engines, "webkit"),
    // The external windows of Firefox and WebKit (RB-040-01).
    key("engines.external.placement", Category::Engines, "external"),
    key(
        "engines.external.restartOnCrash",
        Category::Engines,
        "external",
    ),
    key("engines.external.showTiles", Category::Engines, "external"),
    // A Firefox or WebKit view in its grid cell (RB-080-39).
    key("engines.external.embed", Category::Engines, "external"),
    key("devtools.mode", Category::Devtools, "devtools"),
    key("devtools.followActiveView", Category::Devtools, "devtools"),
    // The dock's size, set by its splitter (RB-010-06).
    key("devtools.dockHeight", Category::Devtools, "devtools"),
    key("devtools.dockWidth", Category::Devtools, "devtools"),
    // The hosted frontend's screencast, off by default (RB-080-21).
    key("devtools.screencast", Category::Devtools, "devtools"),
    // Web Vitals and resources per view (RB-060-10).
    key("vitals.enabled", Category::Devtools, "web-vitals"),
    key("vitals.thresholds", Category::Devtools, "web-vitals"),
    key("resources.enabled", Category::Devtools, "web-vitals"),
    key("resources.ramWarnMb", Category::Devtools, "web-vitals"),
    key("resources.cpuWarnPercent", Category::Devtools, "web-vitals"),
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
    key("reload.pollingFallback", Category::Reload, "polling"),
    key("reload.pollIntervalMs", Category::Reload, "polling"),
    key("reload.intervalPauseSec", Category::Reload, "interval"),
    key(
        "shortcuts.leaderTimeoutMs",
        Category::Shortcuts,
        "behaviour",
    ),
    key(
        "shortcuts.disableInDevtools",
        Category::Shortcuts,
        "behaviour",
    ),
    key("shortcuts.bindings", Category::Shortcuts, "bindings"),
    // The command palette (RB-060-11).
    key(
        "palette.recentCount",
        Category::Shortcuts,
        "command-palette",
    ),
    key(
        "palette.showDescriptions",
        Category::Shortcuts,
        "command-palette",
    ),
    key("palette.prefixes", Category::Shortcuts, "command-palette"),
    key(
        "breakpointShortcuts.newViewModifier",
        Category::Shortcuts,
        "behaviour",
    ),
    key(
        "breakpointShortcuts.overlayMs",
        Category::Shortcuts,
        "behaviour",
    ),
    key(
        "devices.replaceDefaults",
        Category::Breakpoints,
        "device-presets",
    ),
    key("devices.items", Category::Breakpoints, "device-presets"),
    // What the emulation shortcuts switch to, and the own geolocation presets (RB-080-10).
    key(
        "emulation.defaultGeolocation",
        Category::Emulation,
        "geo-time-cpu",
    ),
    key(
        "emulation.geolocationPresets",
        Category::Emulation,
        "geo-time-cpu",
    ),
    key(
        "emulation.defaultTimezone",
        Category::Emulation,
        "geo-time-cpu",
    ),
    key(
        "emulation.defaultCpuThrottling",
        Category::Emulation,
        "geo-time-cpu",
    ),
    // Logging and diagnostics (RB-010-10, `rb_diag::LogSettings`).
    // A raised level would write MCP payloads to the log: never over MCP.
    protected("logs.level", Category::Advanced, "log-level"),
    key("logs.crashDumpsInBundle", Category::Advanced, "diagnostics"),
    key("logs.maxFileSizeMb", Category::Security, "logs"),
    key("logs.maxFiles", Category::Security, "logs"),
    // Plugins (RB-010-03, `docs/plugins.md`): what installs, enables, trusts or widens a plugin
    // is protected.
    protected("plugins.disabled", Category::Plugins, "installed"),
    protected("plugins.trustedKeys", Category::Plugins, "installed"),
    protected("plugins.developmentMode", Category::Plugins, "installed"),
    key("plugins.crashLimit", Category::Plugins, "installed"),
    key("plugins.compileCache", Category::Plugins, "installed"),
    // The plugin runtime (RB-060-12): a page script runs in every page, so switching one back on
    // is the person's; hiding a panel is not.
    protected(
        "plugins.pageScriptsDisabled",
        Category::Plugins,
        "installed",
    ),
    key("plugins.panelsHidden", Category::Plugins, "installed"),
    // Lighthouse beyond the views' sites reads local and LAN pages (RB-090-21): the person's.
    protected("plugins.lighthouseAnyUrl", Category::Plugins, "installed"),
    // The plugin panels' message limit (RB-090-44); `plugins.*` is refused to MCP as a root.
    key(
        "plugins.panelMessageMaxBytes",
        Category::Plugins,
        "installed",
    ),
    // Sessions (RB-020-07): keeping login cookies on disk is the person's decision, never MCP's.
    protected(
        "sessions.persistSessionCookies",
        Category::Security,
        "sessions",
    ),
    // Copying cookies into Firefox and WebKit (RB-040-05), beside the sessions they follow.
    key("engines.cookies.copy", Category::Project, "sessions"),
    key(
        "engines.cookies.domainFilter",
        Category::Project,
        "sessions",
    ),
    key("engines.cookies.exclude", Category::Project, "sessions"),
    key("engines.cookies.reload", Category::Project, "sessions"),
    // WSL (RB-030-04): the helper path names a program rBrowser runs.
    protected("wsl.helperPath", Category::Reload, "wsl-watcher"),
    // Video (RB-080-02): the path names a program rBrowser runs - never over MCP.
    protected("tools.ffmpegPath", Category::Capture, "video"),
    key("wsl.distro", Category::Reload, "wsl-watcher"),
    key(
        "wsl.networkingModeOverride",
        Category::Automation,
        "cdp-proxy",
    ),
    // The settings window's JSON view in syntax colours (RB-080-42).
    key("settings.jsonHighlight", Category::General, "appearance"),
    // The page's context menu (RB-080-43), beside the grid's other switches.
    key("ui.pageContextMenu", Category::Layout, "grid"),
    // Its Emulation submenu (RB-080-49), directly behind its parent switch.
    key("ui.pageContextMenuEmulation", Category::Layout, "grid"),
    // The menus of the label strips, the canvas board and the "+" button (RB-080-41).
    key("ui.viewContextMenu", Category::Layout, "grid"),
    // The page of a view in its cell scaled to the cell (RB-080-40).
    key(
        "engines.external.scaleToCell",
        Category::Engines,
        "external",
    ),
    // The footer under every view (RB-080-44), beside the grid's other switches.
    key("ui.viewFooter.visible", Category::Layout, "grid"),
    key("ui.viewFooter.fields", Category::Layout, "grid"),
    // "Inspect element" opens closed DevTools (RB-080-45).
    key("devtools.openOnInspect", Category::Devtools, "devtools"),
    // New bookmarks as paths that follow the environment (RB-080-48), beside the bookmarks.
    key(
        "bookmarks.followEnvironment",
        Category::Project,
        "bookmarks",
    ),
    // The footer's own analysis of a loaded page (RB-080-47).
    key("ui.viewFooter.analyze", Category::Layout, "grid"),
    // The bundled plugins in the list of Settings > Plugins (RB-080-51).
    key("plugins.showBundled", Category::Plugins, "installed"),
    // What the "New size..." picker lists (RB-080-46).
    key("ui.newViewSize.devices", Category::Layout, "grid"),
    // The canvas's wheel zoom and middle-button pan (RB-080-50), beside `layoutModes.spacePan`.
    key("layoutModes.wheelZoom", Category::Layout, "modes"),
    key("layoutModes.middleButtonPan", Category::Layout, "modes"),
    // The toolbar in a narrow window (RB-080-52), beside the theme.
    key("ui.toolbar.overflow", Category::General, "appearance"),
    // Canvas views that reach past the grid area's edge (RB-080-55), beside the other canvas keys.
    key("layoutModes.showPartialViews", Category::Layout, "modes"),
    // The browser Lighthouse runs in (RB-080-58): it names a program, never over MCP.
    protected("engines.lighthouseBrowserPath", Category::Engines, "node"),
    // The plugin panels floating or docked in the window (RB-080-57), beside their list.
    key("panels.dock", Category::Plugins, "installed"),
    key("panels.dockSize.left", Category::Plugins, "installed"),
    key("panels.dockSize.right", Category::Plugins, "installed"),
    key("panels.dockSize.bottom", Category::Plugins, "installed"),
    // The footer in a narrow view and where its text stands (RB-080-53).
    key("ui.viewFooter.wrap", Category::Layout, "grid"),
    key("ui.viewFooter.align", Category::Layout, "grid"),
    // Code in syntax colours in the panels and the settings' JSON view (RB-080-65), beside the
    // JSON view's own switch.
    key("ui.syntaxHighlight", Category::General, "appearance"),
    // The docked panels open, at the icon rail or hidden (RB-080-93, `panels.dockCollapsed` of
    // RB-080-61 before) and the panels' side bar (RB-080-61), beside the dock keys.
    key("panels.dockState", Category::Plugins, "installed"),
    key("panels.sidebar", Category::Plugins, "installed"),
    // The settings window itself: keys on request (RB-080-70), sections as tabs (RB-080-71).
    key("settings.showKeys", Category::General, "settings-window"),
    key("settings.sectionTabs", Category::General, "settings-window"),
    // Where a link from rBrowser's own surfaces opens (RB-080-92).
    key("ui.openLinksIn", Category::General, "appearance"),
    // Where a sitemap capture's gallery opens (RB-090-83): the person's, and no MCP caller's -
    // `system` hands a file of the repository to the system browser.
    protected("sitemap.galleryOpensIn", Category::Capture, "sitemap"),
    // How the docked panels start and what their Close does (RB-080-93); the panels' switch in
    // the toolbar (RB-080-79).
    key("panels.dockAtStart", Category::Plugins, "installed"),
    key("panels.closeTo", Category::Plugins, "installed"),
    key("ui.toolbar.panelsToggle", Category::General, "appearance"),
    // The portal's button in the toolbar (RB-090-62), beside the toolbar's other switches.
    key("ui.toolbar.portalButton", Category::General, "appearance"),
    // The canvas zoom in the status bar (RB-080-94), beside the bar's other keys.
    key("ui.statusBar.canvasZoom", Category::Layout, "status-bar"),
    // The page zoom inside a view (RB-090-20), beside the grid's zoom.
    key("views.pageZoom.keys", Category::Layout, "zoom"),
    key("views.pageZoom.wheel", Category::Layout, "zoom"),
    key("views.pageZoom.sync", Category::Layout, "zoom"),
    key("views.pageZoom.keepOnReload", Category::Layout, "zoom"),
    // The shell follows Windows' text size (RB-090-26), beside the theme.
    key("ui.followWindowsTextSize", Category::General, "appearance"),
    // The plugin catalogue (RB-091-01): a repository decides what may be installed and the start
    // check reaches the network, so neither is MCP's to set.
    protected("plugins.catalog.repositories", Category::Plugins, "catalog"),
    protected("plugins.catalog.checkOnStart", Category::Plugins, "catalog"),
    // The bundled first-party plugins (RB-091-04): an update installed by itself and the
    // repository as the source decide what runs, so neither is MCP's to set.
    protected("plugins.catalog.autoUpdate", Category::Plugins, "catalog"),
    protected(
        "plugins.catalog.preferRepository",
        Category::Plugins,
        "catalog",
    ),
    key(
        "plugins.catalog.missingNotice",
        Category::Plugins,
        "catalog",
    ),
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
    // The vitals bar under every view (RB-090-71, the label strip's vitals of RB-090-27 before),
    // beside the breakpoint bar's keys; what the label strips show beside the name (RB-090-27).
    key("ui.viewFooter.vitals", Category::Layout, "grid"),
    // The Lighthouse bar below the vitals bar (RB-090-73).
    key("ui.viewFooter.lighthouse", Category::Layout, "grid"),
    key("ui.cellLabel.engineStatus", Category::Layout, "grid"),
    key("ui.cellLabel.address", Category::Layout, "grid"),
    // The time zone of the names a person reads (RB-090-69), beside the language.
    key("ui.fileNameTime", Category::General, "language"),
];
