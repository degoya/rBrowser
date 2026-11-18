//! The settings annotations of the generated schemas (`docs/settings.md`, "Schema annotations"
//! and "Placement of the 0.1 keys").
//!
//! A **key** is a property the settings window edits as one field; it gets `x-category`,
//! `x-section`, `x-order` and `x-scope`, a local-only key also `x-local-only`, a key MCP may not
//! touch `x-protected` (names: `rb-settings`). A root property that is no key is a **group**
//! (`sync`, `network`): it stays unannotated and each of its properties is a key
//! (`sync.scroll`). The root's `$schema` and `version` are file structure, neither. The tables
//! below are the keys in display order; `x-order` numbers them per section in that order,
//! counted across the project and the user schema together (a key may fix its number), and
//! `annotate_tests` holds it unique per section.

use std::collections::BTreeMap;

use rb_settings::{
    Category, Scope, X_CATEGORY, X_LOCAL_ONLY, X_ORDER, X_PROTECTED, X_SCOPE, X_SECTION,
};
use serde_json::{Map, Value};

use crate::LOCAL_ONLY_KEYS;

mod inputs;
mod user_keys;

pub use inputs::{InputDef, PROJECT_INPUTS, USER_INPUTS, annotate_inputs};
pub use user_keys::USER_KEYS;

/// One settings key and where the window shows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyDef {
    /// The dotted key: a root property or `<group>.<property>`.
    pub key: &'static str,
    /// `x-category`.
    pub category: Category,
    /// `x-section`.
    pub section: &'static str,
    /// `x-protected`: refused for every MCP caller.
    pub protected: bool,
    /// A fixed `x-order` (agreed with another crate's definition); the next free number of the
    /// section when `None`.
    pub order: Option<u32>,
    /// A project key the user settings may set too (`x-scope` project, local and user; the
    /// project wins, `crate::shared_settings`).
    pub shared: bool,
}

impl KeyDef {
    /// The same key, `x-protected`.
    const fn protect(self) -> KeyDef {
        KeyDef {
            protected: true,
            ..self
        }
    }

    /// The same key at the fixed `x-order` `order`.
    const fn at(self, order: u32) -> KeyDef {
        KeyDef {
            order: Some(order),
            ..self
        }
    }
}

const fn key(key: &'static str, category: Category, section: &'static str) -> KeyDef {
    KeyDef {
        key,
        category,
        section,
        protected: false,
        order: None,
        shared: false,
    }
}

/// A project key the user settings may set as well.
const fn shared(key: &'static str, category: Category, section: &'static str) -> KeyDef {
    KeyDef {
        key,
        category,
        section,
        protected: false,
        order: None,
        shared: true,
    }
}

/// A key that chooses a program to run or widens what an agent can do.
const fn protected(key: &'static str, category: Category, section: &'static str) -> KeyDef {
    KeyDef {
        key,
        category,
        section,
        protected: true,
        order: None,
        shared: false,
    }
}

/// The keys of the project config (`config.json`, `config.local.json`).
pub const PROJECT_KEYS: &[KeyDef] = &[
    key("startUrl", Category::Project, "start-url"),
    key("environments", Category::Project, "environments"),
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
    key("checks.lighthouse", Category::Checks, "lighthouse"),
    // Checks > Findings (RB-060-01), the user settings may set them too.
    shared("checks.parallel", Category::Checks, "findings"),
    shared("checks.keepRuns", Category::Checks, "findings"),
    shared("checks.rules", Category::Checks, "findings"),
    shared("checks.ignore", Category::Checks, "findings"),
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

/// The root properties that are file structure, neither key nor group.
#[cfg(test)]
pub const STRUCTURAL_KEYS: &[&str] = &["$schema", "version"];

/// Which file a schema describes; decides `x-scope` and the key table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SchemaKind {
    /// `config.json` / `config.local.json`.
    Project,
    /// The user settings.
    User,
}

impl SchemaKind {
    /// The keys of this schema.
    pub fn keys(self) -> &'static [KeyDef] {
        match self {
            SchemaKind::Project => PROJECT_KEYS,
            SchemaKind::User => USER_KEYS,
        }
    }

    /// The paths of this schema a native dialog fills (`x-input`, RB-080-20).
    pub fn inputs(self) -> &'static [InputDef] {
        match self {
            SchemaKind::Project => PROJECT_INPUTS,
            SchemaKind::User => USER_INPUTS,
        }
    }
}

/// Counts `x-order` per (category, section) across the schemas annotated with it.
pub type OrderCounters = BTreeMap<(Category, &'static str), u32>;

/// Annotates every key of `kind`'s table in the root `schema`, then every path of its input
/// table; the keys and paths it cannot find (a table entry without its field) come back as the
/// error.
pub fn annotate(
    schema: &mut Value,
    kind: SchemaKind,
    counters: &mut OrderCounters,
) -> Result<(), Vec<&'static str>> {
    let mut missing = Vec::new();
    for def in kind.keys() {
        let Some(map) = property_mut(schema, def.key) else {
            missing.push(def.key);
            continue;
        };
        let order = match def.order {
            Some(fixed) => fixed,
            None => {
                let next = counters.entry((def.category, def.section)).or_insert(0);
                *next += 1;
                *next
            }
        };
        let local_only = kind == SchemaKind::Project && LOCAL_ONLY_KEYS.contains(&def.key);
        let scopes: &[Scope] = match kind {
            SchemaKind::User => &[Scope::User],
            SchemaKind::Project if local_only => &[Scope::Local],
            SchemaKind::Project if def.shared => &[Scope::Project, Scope::Local, Scope::User],
            SchemaKind::Project => &[Scope::Project, Scope::Local],
        };
        map.insert(X_CATEGORY.to_owned(), Value::from(def.category.id()));
        map.insert(X_SECTION.to_owned(), Value::from(def.section));
        map.insert(X_ORDER.to_owned(), Value::from(order));
        map.insert(
            X_SCOPE.to_owned(),
            serde_json::to_value(scopes).expect("scopes serialize as strings"),
        );
        if local_only {
            map.insert(X_LOCAL_ONLY.to_owned(), Value::Bool(true));
        }
        if def.protected {
            map.insert(X_PROTECTED.to_owned(), Value::Bool(true));
        }
    }
    if let Err(paths) = annotate_inputs(schema, kind.inputs()) {
        missing.extend(paths);
    }
    if missing.is_empty() {
        Ok(())
    } else {
        Err(missing)
    }
}

/// The schema object of the property at dotted `key` (`properties` at each level).
fn property_mut<'a>(schema: &'a mut Value, key: &str) -> Option<&'a mut Map<String, Value>> {
    let mut node = schema;
    for segment in key.split('.') {
        node = node.get_mut("properties")?.get_mut(segment)?;
    }
    if node.as_bool() == Some(true) {
        *node = Value::Object(Map::new());
    }
    node.as_object_mut()
}

#[cfg(test)]
#[path = "annotate_tests.rs"]
mod tests;
