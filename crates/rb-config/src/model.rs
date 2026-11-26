//! The serde model of `.rbrowser/config.json` (plan, "Projekt-Config"), camelCase on disk.
//!
//! Every section defaults, so a config with only `version` loads. Unknown top-level sections
//! (plugins) are kept as JSON in [`ProjectConfig::extra`]. RB-010-01 owns loading, merging,
//! validating and writing; the schema root generated from these types is
//! `schema/config.v1.json`.

use std::collections::BTreeMap;

use indexmap::IndexMap;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{Breakpoint, EmulationFields};

pub use crate::sync_config::{ScrollMode, SyncConfig};
pub use crate::visual::{SlugStrategy, VisualConfig};

/// The whole project config.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProjectConfig {
    /// The schema reference for editors (`./schema/config.v1.json`).
    #[serde(rename = "$schema", default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    /// The format version; [`crate::CURRENT_VERSION`] after migration.
    pub version: u32,
    /// The URL every view loads first: an absolute `http`/`https` URL (`crate::check`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_url: Option<String>,
    /// The environments (local, staging, ...) the toolbar switches between (RB-010-12).
    #[serde(default)]
    pub environments: Vec<Environment>,
    /// The environments behind HTTP Basic auth; their credentials never live here (RB-094-08).
    #[serde(default, skip_serializing_if = "crate::shared_settings::is_default")]
    pub basic_auth: crate::BasicAuthConfig,
    /// Named sessions; `default` always exists, shared by every Chromium view without one.
    #[serde(default)]
    pub sessions: Vec<String>,
    /// The badge colour of each named session as `#rrggbb`; a session without one gets a colour
    /// of its own, and `default` shows no badge (RB-020-07).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    #[schemars(extend("additionalProperties" = { "type": "string", "pattern": "^#[0-9a-fA-F]{6}$" }))]
    pub session_colors: BTreeMap<String, String>,
    /// Named layout presets (RB-010-12).
    #[serde(default)]
    pub layouts: Layouts,
    /// The breakpoints, in config order (`Ctrl+Shift+1...9` follows it).
    #[serde(default)]
    pub breakpoints: Vec<Breakpoint>,
    /// Project bookmarks: a path after the environment's URL or an absolute URL (RB-070-06).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bookmarks: Vec<crate::Bookmark>,
    /// The grid when no preset overrides it (RB-010-04).
    #[serde(default)]
    pub layout: Layout,
    /// Project-wide emulation defaults (Settings > Emulation > Defaults, RB-010-05).
    #[serde(default)]
    pub emulation: EmulationFields,
    /// What is synced between the views.
    #[serde(default)]
    pub sync: SyncConfig,
    /// The page API `window.__rbrowser` and the user-agent suffix (RB-020-08).
    #[serde(default)]
    pub page_api: crate::PageApiConfig,
    /// The page API's message bus (RB-020-08).
    #[serde(default)]
    pub bus: crate::BusConfig,
    /// Hot reload.
    #[serde(default)]
    pub reload: ReloadConfig,
    /// Process-wide network settings; two keys are local-only ([`crate::LOCAL_ONLY_KEYS`]).
    #[serde(default)]
    pub network: NetworkConfig,
    /// Capture > Visual regression (RB-050-04).
    #[serde(default)]
    pub visual: VisualConfig,
    /// Capture > Sitemap (RB-050-05); written only when it differs from the defaults.
    #[serde(default, skip_serializing_if = "crate::shared_settings::is_default")]
    pub sitemap: crate::SitemapConfig,
    /// Capture > Environment compare (RB-080-03); written only when it differs from the
    /// defaults.
    #[serde(default, skip_serializing_if = "crate::shared_settings::is_default")]
    pub env_compare: crate::EnvCompareConfig,
    /// Capture > Bug report (RB-093-04); written only when it differs from the defaults.
    #[serde(default, skip_serializing_if = "crate::shared_settings::is_default")]
    pub report: crate::BugReportConfig,
    /// Capture > Engine compare (RB-094-03); written only when it differs from the defaults.
    #[serde(default, skip_serializing_if = "crate::shared_settings::is_default")]
    pub engine_compare: crate::EngineCompareConfig,
    /// Capture > Screenshots (RB-050-03); the user settings may set the same keys.
    #[serde(default, skip_serializing_if = "crate::shared_settings::is_default")]
    pub capture: crate::CaptureConfig,
    /// The check plugins.
    #[serde(default)]
    pub checks: ChecksConfig,
    /// The SEO plugin.
    #[serde(default)]
    pub seo: SeoConfig,
    /// The breakpoint analyser (RB-010-13).
    #[serde(default)]
    pub analyzer: AnalyzerConfig,
    /// Recommended extensions; applied only in a trusted workspace (RB-080-06).
    #[serde(default)]
    pub extensions: Vec<ExtensionRecommendation>,
    /// Automation: the recorder (RB-050-01).
    #[serde(default)]
    pub automation: crate::AutomationConfig,
    /// Sections of plugins and anything unknown, kept verbatim.
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// One environment of `environments[]`; `x-new` is the entry "Add" starts in the settings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-new" = { "id": "environment", "url": "http://localhost:3000" }))]
pub struct Environment {
    /// Unique id (`local`, `staging`).
    pub id: String,
    /// The origin (path is kept when switching): an absolute `http`/`https` URL.
    pub url: String,
    /// The border colour of the views, `#rrggbb`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// The interval reload of this environment in seconds, over `reload.intervalSec`; 0 is off
    /// (RB-030-05).
    #[serde(
        default,
        alias = "reload_interval_sec",
        skip_serializing_if = "Option::is_none"
    )]
    pub reload_interval_sec: Option<u64>,
    /// Further `http` origins the page loads its assets from - a Vite dev server beside a PHP
    /// app (`http://localhost:5173`): origins only, no path. The portal forwards them for a
    /// device while this environment is shown (RB-090-65).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub asset_origins: Vec<String>,
}

/// `layouts`: the named presets and the one used at start.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Layouts {
    /// The preset at start; the top-level `layout` in the canvas when absent (RB-099-26).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
    /// The presets in config order (`leader L 1...9` follows it).
    #[serde(default)]
    pub presets: IndexMap<String, LayoutPreset>,
}

pub use crate::layout_preset::LayoutPreset;

/// The layout modes: grid and focus (RB-010-12), horizontal, split and canvas (RB-080-04).
/// Canvas is the built-in one (owner, 2026-11-26, RB-099-26): a project without a start preset
/// shows it, and so does a preset without `mode` unless it has a grid of its own
/// ([`LayoutPreset`]); a config that names a mode keeps it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum LayoutMode {
    /// Every cell of the grid.
    Grid,
    /// One large view with a quick switcher.
    Focus,
    /// Every view in one row at the same height, scrolled sideways (RB-080-04).
    Horizontal,
    /// Two sides with a movable divider (RB-080-04).
    Split,
    /// Views placed freely, with snapping and a minimap (RB-080-04).
    #[default]
    Canvas,
}

/// `layout`: the grid.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Layout {
    /// Columns of the grid.
    #[serde(default = "one")]
    pub columns: u32,
    /// Rows of the grid.
    #[serde(default = "one")]
    pub rows: u32,
    /// Gap between cells in DIP (cell labels live in it).
    #[serde(default = "default_gap")]
    pub gap: u32,
    /// The cells; one view each.
    #[serde(default)]
    pub cells: Vec<LayoutCell>,
    /// The top-level layout: the fields of the horizontal, split and canvas modes shown over it
    /// (RB-080-04). Ignored in a preset's `layout`.
    #[serde(default, skip_serializing_if = "crate::LayoutModes::is_empty")]
    pub modes: crate::LayoutModes,
    /// The breakpoints whose views are not open, in every layout mode (RB-080-41): the
    /// selection `view.remove` and `view.add` keep in `config.local.json`. A breakpoint not
    /// listed is open; the last open view cannot be hidden. Ignored in a preset's `layout`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hidden: Vec<String>,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            columns: 1,
            rows: 1,
            gap: default_gap(),
            cells: Vec::new(),
            modes: crate::LayoutModes::default(),
            hidden: Vec::new(),
        }
    }
}

fn one() -> u32 {
    1
}

fn default_gap() -> u32 {
    8
}

/// One cell of `layout.cells[]`; `col` and `row` are 1-based.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LayoutCell {
    /// The breakpoint id shown in the cell.
    pub breakpoint: String,
    /// Column, 1-based.
    pub col: u32,
    /// Row, 1-based.
    pub row: u32,
    /// Columns spanned.
    #[serde(default = "one")]
    pub col_span: u32,
    /// Rows spanned.
    #[serde(default = "one")]
    pub row_span: u32,
    /// `"fit"` (default) or a factor `0.25`..=`1`.
    #[serde(default)]
    pub zoom: Zoom,
}

/// The zoom of a cell: `"fit"` or a fixed factor.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(from = "ZoomRepr", into = "ZoomRepr")]
pub enum Zoom {
    /// Scale the breakpoint to the cell width (`scale = min(1, cell / width)`).
    #[default]
    Fit,
    /// A fixed factor, `0.25`..=`1`.
    Factor(f64),
}

#[derive(Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
enum ZoomRepr {
    Keyword(ZoomKeyword),
    Factor(f64),
}

#[derive(Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
enum ZoomKeyword {
    Fit,
}

impl From<ZoomRepr> for Zoom {
    fn from(repr: ZoomRepr) -> Self {
        match repr {
            ZoomRepr::Keyword(ZoomKeyword::Fit) => Zoom::Fit,
            ZoomRepr::Factor(factor) => Zoom::Factor(factor),
        }
    }
}

impl From<Zoom> for ZoomRepr {
    fn from(zoom: Zoom) -> Self {
        match zoom {
            Zoom::Fit => ZoomRepr::Keyword(ZoomKeyword::Fit),
            Zoom::Factor(factor) => ZoomRepr::Factor(factor),
        }
    }
}

/// `reload`: hot reload (RB-030-05).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct ReloadConfig {
    /// Globs to watch, relative to the project.
    pub watch: Vec<String>,
    /// Globs to ignore.
    pub ignore: Vec<String>,
    /// Debounce in ms.
    pub debounce_ms: u64,
    /// When to reload.
    pub mode: ReloadMode,
    /// Leave pages with their own HMR client (Vite, webpack) to their dev server.
    pub respect_hmr: bool,
    /// Reload every view this often, in seconds; 0 is off.
    pub interval_sec: u64,
}

impl Default for ReloadConfig {
    fn default() -> Self {
        Self {
            watch: Vec::new(),
            ignore: Vec::new(),
            debounce_ms: 200,
            mode: ReloadMode::Auto,
            respect_hmr: true,
            interval_sec: 0,
        }
    }
}

/// The reload mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ReloadMode {
    /// Reload on change (CSS only where possible).
    #[default]
    Auto,
    /// Every change reloads the whole page.
    Full,
    /// Only on command.
    Manual,
}

/// `network`: per instance. `ignoreCertificateErrors` and `proxy` are local-only.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct NetworkConfig {
    /// Chromium `--host-resolver-rules`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host_resolver_rules: Option<String>,
    /// Accept invalid certificates. Local only.
    pub ignore_certificate_errors: bool,
    /// Proxy server (`host:port` or a PAC URL). Local only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxy: Option<String>,
}

/// `checks`: the check plugins and the findings settings; unknown plugin entries are kept.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct ChecksConfig {
    /// Lighthouse.
    pub lighthouse: LighthouseConfig,
    /// Checks > Findings (RB-060-01): `parallel`, `keepRuns`, `rules`, `ignore`; before
    /// [`Self::extra`], which keeps only what is left.
    #[serde(flatten)]
    pub findings: crate::FindingsSettings,
    /// Other check plugins, and `axe` as an older config wrote it: no core key since the axe
    /// plugin's `a11y-axe.tags` took it over, which reads an old `checks.axe.tags` while its
    /// own key is unset (`crates/rbrowser/src/plugins/services/settings.rs`, Charge 10).
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// `checks.lighthouse`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct LighthouseConfig {
    /// Lighthouse categories; empty runs all four. The schema names them (the settings window
    /// offers them as a choice, RB-080-20); the plugin skips what it does not know.
    #[schemars(extend("items" = { "type": "string", "enum": LIGHTHOUSE_CATEGORIES }))]
    pub categories: Vec<String>,
}

/// The categories Lighthouse 12 reports, in the order of its report.
pub const LIGHTHOUSE_CATEGORIES: [&str; 4] =
    ["performance", "accessibility", "best-practices", "seo"];

/// `seo`: the SEO plugin (RB-060-08); further keys are kept.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct SeoConfig {
    /// The locales the site is expected to have (hreflang parity).
    #[schemars(extend("items" = { "type": "string", "x-options": crate::breakpoint::LOCALES }))]
    pub locales: Vec<String>,
    /// Pages of the mini crawl.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_pages: Option<u32>,
    /// Further plugin keys.
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// `analyzer`: the breakpoint analyser (RB-010-13).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct AnalyzerConfig {
    /// How views are proposed.
    pub mode: AnalyzerMode,
    /// Further URLs to analyse besides `startUrl`.
    pub urls: Vec<String>,
    /// Add a view at boundary - 1 px.
    pub edge_views: bool,
    /// Boundaries within this many px are merged.
    pub merge_tolerance_px: u32,
    /// Also scan project files (Tailwind, SCSS, Bootstrap).
    #[serde(rename = "static")]
    pub static_scan: bool,
    /// Look for new breakpoints when the project opens.
    pub check_on_open: bool,
    /// The project files the static source reads, as globs relative to the project root; the
    /// analyser plugin's `project-read` globs bound them (RB-030-07).
    pub static_globs: Vec<String>,
}

/// The default of `analyzer.staticGlobs`: Tailwind v3 and UnoCSS configs (RB-090-50) and the
/// CSS, SCSS and Less sources.
pub const DEFAULT_STATIC_GLOBS: [&str; 3] = [
    "**/tailwind.config.{js,cjs,mjs,ts}",
    "**/{uno,unocss}.config.{js,cjs,mjs,ts}",
    "**/*.{css,scss,less}",
];

impl Default for AnalyzerConfig {
    fn default() -> Self {
        Self {
            mode: AnalyzerMode::All,
            urls: Vec::new(),
            edge_views: false,
            merge_tolerance_px: 8,
            static_scan: true,
            check_on_open: false,
            static_globs: DEFAULT_STATIC_GLOBS.map(str::to_owned).to_vec(),
        }
    }
}

/// The analyser's proposal mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum AnalyzerMode {
    /// One view per breakpoint.
    #[default]
    All,
    /// Five groups.
    Simplified,
    /// A view at every edge plus or minus 1 px.
    Edges,
}

/// One entry of `extensions[]`: a Chrome Web Store id/URL or an unpacked folder.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionRecommendation {
    /// The extension id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// A Chrome Web Store URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// An unpacked extension folder, relative to the project.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;
