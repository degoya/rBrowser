//! The fixed categories of the settings window and their sections (`docs/settings.md`).
//!
//! The ids here are the values of the `x-category` and `x-section` schema annotations. They are
//! part of contract 0.1 and change only through the coordinator; the lint and the registry that
//! check them are RB-010-02's.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A top-level category of the settings window, in the order of the left navigation.
///
/// Fixed: plugins hang their settings into one of these and never add a category of their own.
/// The serialized form (`x-category`) is the id of [`Category::id`]. Owned by the contract;
/// RB-010-02 checks every schema key against it, RB-010-08 renders it.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum Category {
    /// 1 - start behaviour, recent projects, language, updates.
    General,
    /// 2 - start URL, environments, sessions, bookmarks, workspace trust status.
    Project,
    /// 3 - breakpoints, device presets, breakpoint analyser.
    Breakpoints,
    /// 4 - layout presets, grid, zoom, focus mode, further modes.
    Layout,
    /// 5 - navigation, scroll, interaction, leader submit, message bus, page API.
    Sync,
    /// 6 - defaults, media features, network throttling, geolocation/time zone/CPU.
    Emulation,
    /// 7 - Chromium/CEF, Firefox, WebKit/Playwright, Node runtime.
    Engines,
    /// 8 - DevTools mode, console, overlays, simulators, peek, rulers, Web Vitals & resources,
    /// outline, snippets, JSON/XML viewer, storage, image overlay,
    /// colour picker, form autofill.
    Devtools,
    /// 9 - watch paths, ignore, polling fallback, WSL watcher, interval reload.
    Reload,
    /// 10 - mocks, blocking, custom headers, host resolver, certificates, cache.
    Network,
    /// 11 - screenshots, video, visual regression, cookie-banner hiding, sitemap.
    Capture,
    /// 12 - SEO, accessibility, Lighthouse, coverage, PWA, error report, fonts.
    Checks,
    /// 13 - MCP, CDP proxy, Playwright and flows, recorder.
    Automation,
    /// 14 - workspace trust, allow `evaluate`, portal/pairing, logs.
    Security,
    /// 15 - installed plugins, one sub-page per plugin.
    Plugins,
    /// 16 - every command by leader group, the shortcut editor.
    Shortcuts,
    /// 17 - diagnostics, log level, experimental flags, reset.
    Advanced,
    /// 18 - version, engine versions, licences (RB-010-18).
    About,
}

impl Category {
    /// Every category in navigation order.
    pub const ALL: [Category; 18] = [
        Category::General,
        Category::Project,
        Category::Breakpoints,
        Category::Layout,
        Category::Sync,
        Category::Emulation,
        Category::Engines,
        Category::Devtools,
        Category::Reload,
        Category::Network,
        Category::Capture,
        Category::Checks,
        Category::Automation,
        Category::Security,
        Category::Plugins,
        Category::Shortcuts,
        Category::Advanced,
        Category::About,
    ];

    /// The id used in `x-category`, in i18n keys and in deep links.
    pub const fn id(self) -> &'static str {
        match self {
            Category::General => "general",
            Category::Project => "project",
            Category::Breakpoints => "breakpoints",
            Category::Layout => "layout",
            Category::Sync => "sync",
            Category::Emulation => "emulation",
            Category::Engines => "engines",
            Category::Devtools => "devtools",
            Category::Reload => "reload",
            Category::Network => "network",
            Category::Capture => "capture",
            Category::Checks => "checks",
            Category::Automation => "automation",
            Category::Security => "security",
            Category::Plugins => "plugins",
            Category::Shortcuts => "shortcuts",
            Category::Advanced => "advanced",
            Category::About => "about",
        }
    }

    /// The category with this id, if any.
    pub fn from_id(id: &str) -> Option<Category> {
        Category::ALL
            .into_iter()
            .find(|category| category.id() == id)
    }

    /// The i18n key of the category's title: `settings.category.<id>`.
    pub fn title_key(self) -> String {
        format!("settings.category.{}", self.id())
    }

    /// The sections of this category, in display order.
    pub fn sections(self) -> impl Iterator<Item = &'static Section> {
        SECTIONS
            .iter()
            .filter(move |section| section.category == self)
    }
}

/// A section inside a category; its id is the value of `x-section`.
///
/// The i18n key of its title is `settings.section.<category>.<id>`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Section {
    /// The category the section belongs to.
    pub category: Category,
    /// The id, unique inside its category.
    pub id: &'static str,
}

impl Section {
    /// The i18n key of the section's title.
    pub fn title_key(&self) -> String {
        format!("settings.section.{}.{}", self.category.id(), self.id)
    }
}

const fn s(category: Category, id: &'static str) -> Section {
    Section { category, id }
}

/// Every section of every category, grouped by category in navigation order and in display
/// order inside each category (`docs/settings.md`, "Categories").
pub const SECTIONS: &[Section] = &[
    s(Category::General, "startup"),
    s(Category::General, "recent-projects"),
    s(Category::General, "language"),
    s(Category::General, "appearance"),
    s(Category::General, "settings-window"),
    s(Category::General, "updates"),
    s(Category::Project, "start-url"),
    s(Category::Project, "environments"),
    s(Category::Project, "sessions"),
    s(Category::Project, "bookmarks"),
    s(Category::Project, "trust"),
    s(Category::Breakpoints, "breakpoints"),
    s(Category::Breakpoints, "device-presets"),
    s(Category::Breakpoints, "analyzer"),
    s(Category::Layout, "presets"),
    s(Category::Layout, "grid"),
    s(Category::Layout, "zoom"),
    s(Category::Layout, "focus-mode"),
    s(Category::Layout, "modes"),
    s(Category::Layout, "status-bar"),
    s(Category::Layout, "pop-out"),
    s(Category::Sync, "navigation"),
    s(Category::Sync, "scroll"),
    s(Category::Sync, "interaction"),
    s(Category::Sync, "leader-submit"),
    s(Category::Sync, "message-bus"),
    s(Category::Sync, "page-api"),
    s(Category::Emulation, "defaults"),
    s(Category::Emulation, "media-features"),
    s(Category::Emulation, "throttling"),
    s(Category::Emulation, "geo-time-cpu"),
    s(Category::Engines, "chromium"),
    s(Category::Engines, "firefox"),
    s(Category::Engines, "webkit"),
    s(Category::Engines, "node"),
    s(Category::Engines, "external"),
    s(Category::Devtools, "devtools"),
    s(Category::Devtools, "framework-devtools"),
    s(Category::Devtools, "console"),
    s(Category::Devtools, "overlays"),
    s(Category::Devtools, "simulators"),
    s(Category::Devtools, "peek"),
    s(Category::Devtools, "rulers"),
    s(Category::Devtools, "web-vitals"),
    s(Category::Devtools, "outline"),
    s(Category::Devtools, "snippets"),
    s(Category::Devtools, "json-viewer"),
    s(Category::Devtools, "storage"),
    s(Category::Devtools, "image-overlay"),
    s(Category::Devtools, "color-picker"),
    s(Category::Devtools, "form-autofill"),
    s(Category::Reload, "watch"),
    s(Category::Reload, "ignore"),
    s(Category::Reload, "polling"),
    s(Category::Reload, "wsl-watcher"),
    s(Category::Reload, "interval"),
    s(Category::Network, "mocks"),
    s(Category::Network, "blocking"),
    s(Category::Network, "headers"),
    s(Category::Network, "host-resolver"),
    s(Category::Network, "proxy"),
    s(Category::Network, "certificates"),
    s(Category::Network, "cache"),
    s(Category::Capture, "screenshots"),
    s(Category::Capture, "video"),
    s(Category::Capture, "visual-regression"),
    s(Category::Capture, "cookie-banners"),
    s(Category::Capture, "sitemap"),
    s(Category::Capture, "screenshot-editor"),
    s(Category::Capture, "env-compare"),
    s(Category::Checks, "findings"),
    s(Category::Checks, "seo"),
    s(Category::Checks, "accessibility"),
    s(Category::Checks, "lighthouse"),
    s(Category::Checks, "coverage"),
    s(Category::Checks, "pwa"),
    s(Category::Checks, "error-report"),
    s(Category::Checks, "fonts"),
    s(Category::Automation, "mcp"),
    s(Category::Automation, "cdp-proxy"),
    s(Category::Automation, "playwright"),
    s(Category::Automation, "recorder"),
    s(Category::Security, "trust"),
    s(Category::Security, "evaluate"),
    s(Category::Security, "portal"),
    s(Category::Security, "logs"),
    s(Category::Security, "sessions"),
    s(Category::Plugins, "installed"),
    s(Category::Shortcuts, "behaviour"),
    s(Category::Shortcuts, "bindings"),
    s(Category::Shortcuts, "command-palette"),
    s(Category::Advanced, "diagnostics"),
    s(Category::Advanced, "log-level"),
    s(Category::Advanced, "experimental"),
    s(Category::Advanced, "reset"),
    s(Category::About, "version"),
    s(Category::About, "engines"),
    s(Category::About, "licences"),
];

/// The section `id` of `category`, if it exists.
pub fn section(category: Category, id: &str) -> Option<&'static Section> {
    category.sections().find(|section| section.id == id)
}

#[cfg(test)]
#[path = "category_tests.rs"]
mod tests;
