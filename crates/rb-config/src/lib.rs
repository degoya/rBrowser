//! The project config `.rbrowser/config.json` and the user settings
//! `%APPDATA%\rBrowser\settings.json`: the serde model, the schema and the entry points
//! (contract 0.1, `docs/contracts-0.1.md`).
//!
//! The types and signatures are the contract; RB-010-01 implements them: discovery
//! (`discover`), loading with migration, the local-only rule, the merge `config.json` <-
//! `config.local.json` and validation (`load`, `migrate`, `local_only`, `merge`, `validate`),
//! writing (`jsonio`), the user settings file (`user_file`), the watcher hook (`watch`) and the
//! annotated schemas (`annotate`, `schema`).
//!
//! Wave 0.1-w1b adds (`docs/contracts-0.1-w1b.md`): the app's folders and the portable mode
//! (`dirs`, RB-010-15), workspace trust (`trust`, RB-010-09), the device presets (`devices`,
//! RB-010-12) and the user settings `startup`, `grid`, `focusMode`, `devtools.followActiveView`;
//! a breakpoint's viewport resolved against its device preset (`viewport`, RB-010-12). RB-080-10
//! adds the emulation extras (`emulation_extras`) and their user settings (`emulation_settings`).

mod activity_bar_settings;
pub mod allowed;
mod annotate;
mod automation;
mod basic_auth;
pub mod bookmark_paths;
pub mod bookmarks;
mod breakpoint;
pub mod breakpoint_refs;
mod bug_report;
mod cell_label_settings;
mod clock_settings;
mod devices;
mod dirs;
mod discover;
mod emulation_extras;
mod emulation_settings;
mod engine_compare;
mod env_compare;
mod error;
mod extension_settings;
mod file_name_time;
mod findings_export;
mod framework_devtools;
mod jsonio;
mod layout_mode_settings;
mod layout_modes;
mod load;
mod local_only;
mod merge;
mod migrate;
mod model;
mod new_view_size_settings;
mod notification_settings;
mod open_links;
mod page_api;
mod page_locale;
mod page_zoom_settings;
mod palette;
mod panel_settings;
mod perf_settings;
mod plugin_settings;
mod pop_out_settings;
mod retired;
mod schema;
mod sessions;
mod shared_settings;
mod sitemap;
mod status_bar_settings;
mod sync_config;
mod title_bar_settings;
mod toolbar_settings;
pub mod trust;
mod ui_theme;
mod user_android;
mod user_automation;
mod user_cdp;
mod user_engines;
mod user_file;
mod user_ios;
mod user_logs;
mod user_mcp;
mod user_portal;
mod user_presentation;
mod user_reload;
mod user_report;
mod user_safari;
mod user_settings;
mod user_shortcuts;
mod user_sitemap;
mod user_updates;
mod user_view;
mod user_vitals;
mod validate;
mod video_settings;
mod view_footer_settings;
mod viewport;
mod visual;
mod watch;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::Value;

pub use activity_bar_settings::{
    ActivityBarMode, ActivityBarPosition, ActivityBarSettings, ActivityBarShow,
};
pub use automation::{
    AutomationConfig, LocatorPreference, PlaybackPace, RecordHover, RecordPointer, RecorderConfig,
    RecorderExportConfig,
};
pub use basic_auth::BasicAuthConfig;
pub use bookmarks::{Bookmark, BookmarkTarget, ResolvedBookmark};
pub use breakpoint::{
    Breakpoint, ColorGamut, ColorScheme, Contrast, DeviceKind, DevicePreset, EmulationFields,
    EngineKind, ForcedColors, MediaFeatures, MediaType, NetworkConditions, NetworkPreset,
    NetworkThrottle, Orientation, SafeAreaInsets, TIMEZONES, Viewport,
};
pub use bug_report::{BugReportConfig, ReportPart};
pub use cell_label_settings::{CellAddress, CellLabelSettings};
pub use clock_settings::{
    ANIMATION_RATE_CHOICES, ClockUserSettings, DEFAULT_ANIMATION_RATES, DEFAULT_ANIMATION_STEP_MS,
};
pub use devices::{BUILTIN_DEVICES_FILE, builtin_devices, effective_devices};
pub use dirs::{
    LOCAL_APP_DIR, PORTABLE_DATA_DIR, PORTABLE_LOCAL_DIR, PORTABLE_MARKER, local_dir,
    local_dir_for, portable_root, portable_root_for, roaming_dir, roaming_dir_for,
};
pub use emulation_extras::{
    DEFAULT_ACCURACY_M, Geolocation, MAX_CPU_THROTTLING, MIN_CPU_THROTTLING, Paper,
    cpu_throttling_problem, is_iana_timezone,
};
pub use emulation_settings::{
    DEFAULT_CPU_THROTTLING, DEFAULT_GEOLOCATION_PRESET, DEFAULT_TIMEZONE, EmulationUserSettings,
    GeolocationPreset, geolocation_preset, geolocation_presets,
};
pub use engine_compare::EngineCompareConfig;
pub use env_compare::EnvCompareConfig;
pub use error::{ConfigError, ConfigIssue, MigrationNote};
pub use extension_settings::{ChromiumExtensionSettings, UnpackedExtension};
pub use file_name_time::FileNameTime;
pub use findings_export::{FindingsExportFormat, FindingsExportSettings, FindingsFailOn};
pub use framework_devtools::{FrameworkDevtoolsMode, FrameworkDevtoolsSettings};
pub use jsonio::{ConfigFile, GITIGNORE, read_json, to_json_string, write_config_file, write_json};
pub use layout_mode_settings::{LayoutModeSettings, MinimapPosition, PresetFile, SPLIT_STEP_RANGE};
pub use layout_modes::{
    CANVAS_ZOOM_RANGE, CanvasItem, LayoutModes, MAX_CANVAS_COORD, MAX_SNAP, ModeFields,
    SPLIT_RATIO_RANGE, STRIP_HEIGHT_RANGE, SplitOrientation, StripHeight, check_hidden,
    default_item_zoom, hidden_of, hidden_selection, hidden_selection_in, keeps_a_chromium_view,
    keeps_a_chromium_view_in, known_hidden,
};
pub use merge::merge;
pub use model::{
    AnalyzerConfig, AnalyzerMode, ChecksConfig, DEFAULT_STATIC_GLOBS, Environment,
    ExtensionRecommendation, Layout, LayoutCell, LayoutMode, LayoutPreset, Layouts,
    LighthouseConfig, NetworkConfig, ProjectConfig, ReloadConfig, ReloadMode, ScrollMode,
    SeoConfig, SlugStrategy, SyncConfig, VisualConfig, Zoom,
};
pub use new_view_size_settings::NewViewSizeSettings;
pub use notification_settings::NotificationSettings;
pub use open_links::OpenLinksIn;
pub use page_api::{BusConfig, PageApiConfig};
pub use page_locale::{
    DEFAULT_PSEUDO_FACTOR, PSEUDO_FACTOR_RANGE, PseudoLocaleSettings, TextDirection,
    clamp_pseudo_factor,
};
pub use page_zoom_settings::{PageZoomSettings, ViewsSettings};
pub use palette::PaletteSettings;
pub use panel_settings::{
    PanelCloseTo, PanelDock, PanelDockAtStart, PanelDockSize, PanelDockState, PanelSettings,
    PanelSidebar,
};
pub use perf_settings::{MAX_INACTIVE_FPS, MAX_MEMORY_WARNING_MB, PerformanceSettings};
pub use pop_out_settings::PopOutSettings;
pub use rb_settings::{Origin, Scope};
pub use retired::RETIRED_KEY;
pub use schema::{PROJECT_SCHEMA_FILE, USER_SETTINGS_SCHEMA_FILE, schema_files};
pub use sessions::{
    SESSION_COLORS_KEY, SESSION_ID_MAX_LEN, SessionSettings, check_sessions, is_valid_session_id,
    parse_hex_color, session_color,
};
pub use shared_settings::{CaptureConfig, FindingsSettings, IgnoreEntry, RuleSetting};
pub use sitemap::SitemapConfig;
pub use status_bar_settings::{
    MAX_HISTORY_SIZE, MAX_INFO_TIMEOUT_SEC, MIN_HISTORY_SIZE, StatusBarSettings,
};
pub use title_bar_settings::TitleBarSettings;
pub use toolbar_settings::{ToolbarOverflow, ToolbarSettings};
pub use user_android::AndroidSettings;
pub use user_automation::{
    AutomationSettings, DEFAULT_FLOWS_DIR, FlowSettings, HiddenViews, RUN_LAYOUTS,
};
pub use user_engines::{CookieSettings, ExternalWindowSettings, FirefoxSettings, WebkitSettings};
pub use user_ios::IosSettings;
pub use user_report::UserReportSettings;
pub use user_safari::SafariSettings;
pub use user_sitemap::{GalleryOpensIn, UserSitemapSettings};
// The plugin catalogue's settings (RB-091-01).
pub use plugin_settings::{CatalogRepository, CatalogSettings};
pub use user_file::settings_path_for;
pub use user_portal::{MAX_DEVICES, MAX_PAIRING_TTL_SEC, MIN_PAIRING_TTL_SEC, PortalSettings};
// The portal over the internet (RB-096-05).
pub use user_presentation::{
    MAX_LINK_TTL_SEC, MAX_VIEWERS, MIN_LINK_TTL_SEC, PresentationSettings,
};
pub use user_settings::{
    BreakpointShortcutSettings, CdpSettings, ChromiumSettings, DeviceSettings, DevtoolsMode,
    DevtoolsSettings, EngineSettings, LogSettings, McpClient, McpScriptSettings, McpSettings,
    NewViewModifier, PluginId, PluginSettings, ShortcutSettings, Theme, TrustedPluginKey,
    UiSettings, UpdateAutoInstall, UpdateChannel, UpdateCheck, UpdateSettings, UserReloadSettings,
    UserSettings, WslNetworkingMode, WslSettings,
};
pub use user_view::{
    BookmarkSettings, FocusModeSettings, GridViewSettings, SettingsWindowSettings, StartupRestore,
    StartupSettings,
};
pub use user_vitals::{ResourcesSettings, VitalThreshold, VitalThresholds, VitalsSettings};
pub use validate::{
    ASSET_ORIGIN_KEY, DEFAULT_SESSION, ENVIRONMENT_URL_KEY, MAX_BREAKPOINT_SIZE, START_URL_KEY,
    check, is_http_origin,
};
pub use video_settings::{ToolsSettings, VideoConfig};
pub use view_footer_settings::{FooterAlign, FooterField, FooterWrap, ViewFooterSettings};
pub use viewport::check_viewports;
pub use watch::{is_config_file, reload_on_change, watched_files};

/// The config format version this build writes; older files are migrated to it.
pub const CURRENT_VERSION: u32 = 1;

/// The project's config directory, relative to the project root.
pub const CONFIG_DIR: &str = ".rbrowser";
/// The shared config inside [`CONFIG_DIR`], checked in.
pub const CONFIG_FILE: &str = "config.json";
/// The machine's overrides inside [`CONFIG_DIR`], git-ignored.
pub const LOCAL_CONFIG_FILE: &str = "config.local.json";
/// The user settings file name inside `%APPDATA%\rBrowser\`.
pub const USER_SETTINGS_FILE: &str = "settings.json";

/// Keys valid only in `config.local.json`; in `config.json` they are ignored, with one warning
/// each (`config.warning.local_only_ignored`) unless the value is `null` or `false`.
pub const LOCAL_ONLY_KEYS: &[&str] = &["network.ignoreCertificateErrors", "network.proxy"];

/// Keys no project file applies - neither `config.json` nor `config.local.json`, which a cloned
/// repository can commit as well - while the workspace is not trusted: they would switch off
/// TLS checks or redirect every page of the grid. Each one set gives a warning
/// (`config.warning.trust_required`) naming key and file. [`load_project`] always removes
/// them; [`trust::load_project_trusted`] keeps them for a trusted workspace (RB-010-09). The
/// local-only rule ([`LOCAL_ONLY_KEYS`]) holds even then.
pub const TRUST_GATED_KEYS: &[&str] = &[
    "network.ignoreCertificateErrors",
    "network.proxy",
    "network.hostResolverRules",
];

pub(crate) fn current_version() -> u32 {
    CURRENT_VERSION
}

/// A project config after discovery, merge, migration and validation.
#[derive(Clone, Debug)]
pub struct LoadedConfig {
    /// The project root (the directory that holds [`CONFIG_DIR`]).
    pub root: PathBuf,
    /// The effective config: defaults <- `config.json` <- `config.local.json`.
    pub config: ProjectConfig,
    /// Where each set value came from, by dotted key (`sync.scroll`,
    /// `breakpoints.mobile.width` - array items by their `id`). Keys absent here are defaults.
    pub origins: BTreeMap<String, Origin>,
    /// Warnings that did not stop loading (a local-only key in `config.json`).
    pub warnings: Vec<ConfigIssue>,
    /// What a migration changed; empty when the file was current.
    pub migrations: Vec<MigrationNote>,
}

/// Loads the config of the project at `root`: reads `.rbrowser/config.json` and
/// `config.local.json`, migrates in memory (writes nothing; [`write_config_file`] backs an
/// older file up to `config.json.bak-v<n>` when it is saved), merges (arrays with `id` by `id`,
/// others replaced), applies the local-only rule and the trust gate ([`TRUST_GATED_KEYS`],
/// closed: the app loads through [`trust::load_project_trusted`]) and validates. RB-010-01.
pub fn load_project(root: &Path) -> Result<LoadedConfig, ConfigError> {
    load::load_project(root)
}

/// Finds the project root from `start` upwards: the first directory with
/// `.rbrowser/config.json`. Works on `\\wsl.localhost\...` paths. RB-010-01.
pub fn discover(start: &Path) -> Option<PathBuf> {
    discover::discover(start)
}

/// Loads the user settings file at `path`; a missing file gives the defaults. A value the schema
/// or the types refuse falls back to its default and the rest of the file applies (see
/// [`load_user_settings_with_warnings`]). RB-010-01.
pub fn load_user_settings(path: &Path) -> Result<UserSettings, ConfigError> {
    user_file::load_user_settings(path)
}

/// [`load_user_settings`] with a warning (`config.error.invalid_value`, the key's JSON pointer)
/// for every value that fell back to its default. Unreadable JSON, a newer `version` or a file
/// that is no object stay errors.
pub fn load_user_settings_with_warnings(
    path: &Path,
) -> Result<(UserSettings, Vec<ConfigIssue>), ConfigError> {
    user_file::load_user_settings_with_warnings(path)
}

/// The user settings path, `%APPDATA%\rBrowser\settings.json` (portable: beside the executable,
/// [`roaming_dir`]); `None` when the platform has no such directory. RB-010-01.
pub fn user_settings_path() -> Option<PathBuf> {
    user_file::user_settings_path()
}

/// Moves a config value of any older `version` to [`CURRENT_VERSION`] and says what changed.
/// RB-010-01.
pub fn migrate(value: Value) -> Result<(Value, Vec<MigrationNote>), ConfigError> {
    migrate::migrate(value)
}

/// The JSON schema of the project config, generated from the types and annotated with the
/// settings structure (the source of `schema/config.v1.json`).
pub fn project_schema() -> schemars::Schema {
    schema::cached().0.clone()
}

/// The JSON schema of the user settings (the source of `schema/settings.v1.json`).
pub fn user_settings_schema() -> schemars::Schema {
    schema::cached().1.clone()
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
