//! The key table of the project config (`config.json`, `config.local.json`), moved out of
//! `annotate` and re-exported there, as the user settings' table is `user_keys.rs`.

use rb_settings::Category;

use super::{KeyDef, key, protected, shared};

/// The keys of the project config (`config.json`, `config.local.json`).
pub const PROJECT_KEYS: &[KeyDef] = &[
    key("startUrl", Category::Project, "start-url"),
    key("environments", Category::Project, "environments"),
    // RB-094-08: which environments sit behind Basic auth; only a person turns it on.
    protected("basicAuth.environments", Category::Project, "environments"),
    key("sessions", Category::Project, "sessions"),
    key("sessionColors", Category::Project, "sessions"),
    key("bookmarks", Category::Project, "bookmarks"),
    // Breakpoints carry request headers: MCP edits them with the breakpoint tools, which refuse
    // headers, never whole.
    key("breakpoints", Category::Breakpoints, "breakpoints").protect(),
    key("layouts", Category::Layout, "presets"),
    key("layout", Category::Layout, "grid"),
    // Emulation defaults (RB-010-05): one key per field, by the section the job names.
    key("emulation.dpr", Category::Emulation, "defaults"),
    key("emulation.mobile", Category::Emulation, "defaults"),
    key("emulation.touch", Category::Emulation, "defaults"),
    key("emulation.userAgent", Category::Emulation, "defaults"),
    key("emulation.platform", Category::Emulation, "defaults"),
    key("emulation.locale", Category::Emulation, "defaults"),
    key("emulation.orientation", Category::Emulation, "defaults"),
    key("emulation.safeAreas", Category::Emulation, "defaults"),
    key("emulation.textScale", Category::Emulation, "defaults"),
    key(
        "emulation.colorScheme",
        Category::Emulation,
        "media-features",
    ),
    key(
        "emulation.reducedMotion",
        Category::Emulation,
        "media-features",
    ),
    key("emulation.media", Category::Emulation, "media-features"),
    key("emulation.network", Category::Emulation, "throttling"),
    // The emulation extras (RB-080-10): offline beside the throttling, the paper beside the
    // print media type, position, time zone and CPU in their own section.
    key("emulation.offline", Category::Emulation, "throttling"),
    key("emulation.paper", Category::Emulation, "media-features"),
    key("emulation.geolocation", Category::Emulation, "geo-time-cpu"),
    key("emulation.timezone", Category::Emulation, "geo-time-cpu"),
    key(
        "emulation.cpuThrottling",
        Category::Emulation,
        "geo-time-cpu",
    ),
    // The page's text direction and language (RB-092-02).
    key("emulation.dir", Category::Emulation, "text-direction"),
    key("emulation.lang", Category::Emulation, "text-direction"),
    // Project headers apply only in a trusted workspace (RB-010-09).
    key("emulation.headers", Category::Network, "headers").protect(),
    key("sync.navigation", Category::Sync, "navigation"),
    key("sync.navigationSettleMs", Category::Sync, "navigation"),
    key("sync.navigationIncludeHash", Category::Sync, "navigation"),
    key("sync.detectBrowsersync", Category::Sync, "navigation"),
    key("sync.scroll", Category::Sync, "scroll"),
    key("sync.scrollMode", Category::Sync, "scroll"),
    key("sync.scrollDebounceMs", Category::Sync, "scroll"),
    key("sync.click", Category::Sync, "interaction"),
    key("sync.input", Category::Sync, "interaction"),
    key("sync.keyboard", Category::Sync, "interaction"),
    key("sync.hover", Category::Sync, "interaction"),
    key("sync.hoverThrottleMs", Category::Sync, "interaction"),
    key("sync.desyncTimeoutMs", Category::Sync, "interaction"),
    key("sync.contextMenu", Category::Sync, "interaction"),
    key("sync.forms", Category::Sync, "interaction"),
    key("sync.focus", Category::Sync, "interaction"),
    key("sync.leaderOnlySubmit", Category::Sync, "leader-submit"),
    key("sync.leaderPerSession", Category::Sync, "leader-submit"),
    key("sync.activateOnFocus", Category::Sync, "leader-submit"),
    // The page API and its message bus (RB-020-08).
    key("bus.enabled", Category::Sync, "message-bus"),
    key("bus.maxPayloadBytes", Category::Sync, "message-bus"),
    key("bus.maxPerSecond", Category::Sync, "message-bus"),
    key("pageApi.enabled", Category::Sync, "page-api"),
    key("pageApi.uaSuffix", Category::Sync, "page-api"),
    key("reload.watch", Category::Reload, "watch"),
    key("reload.debounceMs", Category::Reload, "watch"),
    key("reload.mode", Category::Reload, "watch"),
    // Hot reload (RB-030-05).
    key("reload.respectHmr", Category::Reload, "watch"),
    key("reload.intervalSec", Category::Reload, "interval"),
    key("reload.ignore", Category::Reload, "ignore"),
    // Guarded by workspace trust (RB-010-09): never over MCP.
    key(
        "network.hostResolverRules",
        Category::Network,
        "host-resolver",
    )
    .protect(),
    key("network.proxy", Category::Network, "proxy").protect(),
    key(
        "network.ignoreCertificateErrors",
        Category::Network,
        "certificates",
    )
    .protect(),
    // Capture > Visual regression (RB-050-04).
    key("visual.threshold", Category::Capture, "visual-regression"),
    key("visual.fullPage", Category::Capture, "visual-regression"),
    key(
        "visual.slugStrategy",
        Category::Capture,
        "visual-regression",
    ),
    key("visual.lfsCheck", Category::Capture, "visual-regression"),
    key(
        "visual.failOnMetaMismatch",
        Category::Capture,
        "visual-regression",
    ),
    // Capture > Screenshots (RB-050-03), the user settings may set them too.
    // Where files are written is the person's choice, never MCP's.
    shared("capture.dir", Category::Capture, "screenshots").protect(),
    shared("capture.pathPattern", Category::Capture, "screenshots"),
    shared("capture.elementPadding", Category::Capture, "screenshots"),
    shared("capture.defaultDelay", Category::Capture, "screenshots"),
    shared("capture.overviewLabels", Category::Capture, "screenshots"),
    // Capture > Cookie banners & animations (RB-080-12), the user settings may set them too.
    shared("capture.hideBanners", Category::Capture, "cookie-banners"),
    shared("capture.hideSelectors", Category::Capture, "cookie-banners"),
    shared(
        "capture.useBuiltinList",
        Category::Capture,
        "cookie-banners",
    ),
    shared(
        "capture.freezeAnimations",
        Category::Capture,
        "cookie-banners",
    ),
    shared("capture.settleMs", Category::Capture, "cookie-banners"),
    // Capture > Video (RB-080-02), the user settings may set them too.
    shared("capture.video.fps", Category::Capture, "video"),
    shared("capture.video.maxDurationSec", Category::Capture, "video"),
    shared("capture.video.quality", Category::Capture, "video"),
    // The Videos panel (RB-110-12).
    shared("capture.video.keepRecordings", Category::Capture, "video"),
    shared("capture.video.keepDays", Category::Capture, "video"),
    shared("capture.video.autoplay", Category::Capture, "video"),
    // Capture > Sitemap (RB-050-05).
    key("sitemap.parallel", Category::Capture, "sitemap"),
    key("sitemap.waitNetworkIdle", Category::Capture, "sitemap"),
    key("sitemap.waitSelector", Category::Capture, "sitemap"),
    key("sitemap.delayMs", Category::Capture, "sitemap"),
    key("sitemap.timeoutSec", Category::Capture, "sitemap"),
    // RB-090-83: a shot's own deadline.
    key("sitemap.shotTimeoutSec", Category::Capture, "sitemap"),
    key("sitemap.fullPage", Category::Capture, "sitemap"),
    key("sitemap.autoScroll", Category::Capture, "sitemap"),
    key("sitemap.include", Category::Capture, "sitemap"),
    key("sitemap.exclude", Category::Capture, "sitemap"),
    key("sitemap.maxPages", Category::Capture, "sitemap"),
    key("sitemap.hostIntervalMs", Category::Capture, "sitemap"),
    key("sitemap.respectRobots", Category::Capture, "sitemap"),
    key("sitemap.hideSelectors", Category::Capture, "sitemap"),
    key("sitemap.retries", Category::Capture, "sitemap"),
    key("sitemap.maxDepth", Category::Capture, "sitemap"),
    key("sitemap.maxSitemapMib", Category::Capture, "sitemap"),
    key("sitemap.maxUrls", Category::Capture, "sitemap"),
    // Capture > Bug report (RB-093-04).
    key("report.include", Category::Capture, "report"),
    key("report.annotate", Category::Capture, "report"),
    // Capture > Environment compare (RB-080-03).
    key("envCompare.environments", Category::Capture, "env-compare"),
    key("envCompare.breakpoints", Category::Capture, "env-compare"),
    key("envCompare.threshold", Category::Capture, "env-compare"),
    key(
        "envCompare.maxDiffPercent",
        Category::Capture,
        "env-compare",
    ),
    key("envCompare.fullPage", Category::Capture, "env-compare"),
    key("envCompare.waitMs", Category::Capture, "env-compare"),
    key("envCompare.parallel", Category::Capture, "env-compare"),
    // Capture > Engine compare (RB-094-03).
    key(
        "engineCompare.threshold",
        Category::Capture,
        "engine-compare",
    ),
    key(
        "engineCompare.maxDiffPercent",
        Category::Capture,
        "engine-compare",
    ),
    key("engineCompare.waitMs", Category::Capture, "engine-compare"),
    key("checks.lighthouse", Category::Checks, "lighthouse"),
    // Checks > Findings (RB-060-01), the user settings may set them too.
    shared("checks.parallel", Category::Checks, "findings"),
    shared("checks.keepRuns", Category::Checks, "findings"),
    shared("checks.rules", Category::Checks, "findings"),
    shared("checks.ignore", Category::Checks, "findings"),
    // The findings export (RB-092-05). Where MCP may write is the person's choice, never MCP's.
    shared("checks.export.defaultFormat", Category::Checks, "findings"),
    shared("checks.export.failOn", Category::Checks, "findings"),
    shared("checks.export.dir", Category::Checks, "findings").protect(),
    key("seo", Category::Checks, "seo"),
    // The breakpoint analyser (RB-010-13).
    key("analyzer.mode", Category::Breakpoints, "analyzer"),
    key("analyzer.urls", Category::Breakpoints, "analyzer"),
    key("analyzer.edgeViews", Category::Breakpoints, "analyzer"),
    key(
        "analyzer.mergeTolerancePx",
        Category::Breakpoints,
        "analyzer",
    ),
    key("analyzer.static", Category::Breakpoints, "analyzer"),
    key("analyzer.checkOnOpen", Category::Breakpoints, "analyzer"),
    // The static source's files (RB-030-07).
    key("analyzer.staticGlobs", Category::Breakpoints, "analyzer"),
    // The recorder (RB-050-01).
    key(
        "automation.recorder.recordScroll",
        Category::Automation,
        "recorder",
    ),
    key(
        "automation.recorder.recordHover",
        Category::Automation,
        "recorder",
    ),
    // How long the pointer rests on a menu's entry and which of its way a recording keeps
    // (RB-090-87).
    key(
        "automation.recorder.hoverDwellMs",
        Category::Automation,
        "recorder",
    ),
    key(
        "automation.recorder.recordPointer",
        Category::Automation,
        "recorder",
    ),
    // The form autofill's fills in a recording (RB-090-78).
    key(
        "automation.recorder.recordAutofill",
        Category::Automation,
        "recorder",
    ),
    // How a recording plays back and its export waits (RB-090-66).
    key(
        "automation.recorder.playbackPace",
        Category::Automation,
        "recorder",
    ),
    // The Recorder panel follows a played recording's steps (RB-090-84).
    key(
        "automation.recorder.followPlayback",
        Category::Automation,
        "recorder",
    ),
    // The Recorder panel lists a recording's steps while it runs (RB-090-89).
    key(
        "automation.recorder.showLiveSteps",
        Category::Automation,
        "recorder",
    ),
    // The least pause before each step at the pace `recorded` (RB-090-84).
    key(
        "automation.recorder.minPauseMs",
        Category::Automation,
        "recorder",
    ),
    // The pause before a step added in the Recorder panel's editor (RB-092-01).
    key(
        "automation.recorder.insertedStepPauseMs",
        Category::Automation,
        "recorder",
    ),
    // The Recorder panel asks before it deletes a recording (RB-090-91).
    key(
        "automation.recorder.confirmDelete",
        Category::Automation,
        "recorder",
    ),
    // The fields whose values a recording masks: MCP may not empty the list and record
    // passwords.
    key(
        "automation.recorder.secretFields",
        Category::Automation,
        "recorder",
    )
    .protect(),
    // Where files are written is the person's choice, never MCP's (as `capture.dir`).
    key("automation.recorder.dir", Category::Automation, "recorder").protect(),
    // The recorder's export (RB-050-02).
    key(
        "automation.recorderExport.locatorPreference",
        Category::Automation,
        "recorder",
    ),
    key(
        "automation.recorderExport.dir",
        Category::Automation,
        "recorder",
    )
    .protect(),
    key(
        "automation.recorderExport.includeConfig",
        Category::Automation,
        "recorder",
    ),
    key("extensions", Category::Engines, "chromium").protect(),
];
