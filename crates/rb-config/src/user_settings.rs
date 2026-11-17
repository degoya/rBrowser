//! The serde model of the user settings, `%APPDATA%\rBrowser\settings.json`: app values that
//! belong to the person, not the project. RB-010-01 loads and writes it; RB-010-02 annotates
//! the keys; the jobs named per field read them.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use crate::plugin_settings::{PluginId, PluginSettings, TrustedPluginKey};
pub use crate::ui_theme::Theme;
use crate::user_automation::AutomationSettings;
use crate::user_engines::{
    CookieSettings, ExternalWindowSettings, FirefoxSettings, WebkitSettings,
};
pub use crate::user_logs::LogSettings;
pub use crate::user_shortcuts::{BreakpointShortcutSettings, NewViewModifier, ShortcutSettings};
pub use crate::user_updates::{UpdateAutoInstall, UpdateChannel, UpdateCheck, UpdateSettings};
use crate::user_view::{
    FocusModeSettings, GridViewSettings, SettingsWindowSettings, StartupSettings,
};
use crate::{DevicePreset, EmulationUserSettings};

/// The whole user settings file. Every section defaults.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct UserSettings {
    /// The schema reference for editors.
    #[serde(rename = "$schema", default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    /// The format version; [`crate::CURRENT_VERSION`] after migration.
    #[serde(default = "crate::current_version")]
    pub version: u32,
    /// General: language and theme.
    #[serde(default)]
    pub ui: UiSettings,
    /// General > recent projects, newest first (absolute paths).
    #[serde(default)]
    pub recent_projects: Vec<String>,
    /// General > start behaviour (RB-010-11).
    #[serde(default)]
    pub startup: StartupSettings,
    /// Layout > zoom of the grid area (RB-010-12).
    #[serde(default)]
    pub grid: GridViewSettings,
    /// Layout > focus mode (RB-010-12).
    #[serde(default)]
    pub focus_mode: FocusModeSettings,
    /// General > appearance: the settings window's JSON view (RB-080-42).
    #[serde(default)]
    pub settings: SettingsWindowSettings,
    /// Layout > further modes: horizontal, split and canvas (RB-080-04).
    #[serde(default)]
    pub layout_modes: crate::LayoutModeSettings,
    /// Layout > pop-out: a view in a window of its own at its real size (RB-080-14).
    #[serde(default)]
    pub pop_out: crate::PopOutSettings,
    /// Layout > grid and Advanced > experimental flags: throttling and the memory warning
    /// (RB-020-09).
    #[serde(default)]
    pub performance: crate::PerformanceSettings,
    /// Automation > CDP proxy (RB-030-06).
    #[serde(default)]
    pub cdp: CdpSettings,
    /// Automation > MCP (RB-030-01).
    #[serde(default)]
    pub mcp: McpSettings,
    /// Security & Privacy > Portal: devices in the LAN (RB-080-05).
    #[serde(default)]
    pub portal: crate::PortalSettings,
    /// Automation > Playwright & flows (RB-040-07).
    #[serde(default)]
    pub automation: AutomationSettings,
    /// Engines: paths of external programs.
    #[serde(default)]
    pub engines: EngineSettings,
    /// DevTools & Debugging (RB-010-06).
    #[serde(default)]
    pub devtools: DevtoolsSettings,
    /// DevTools & Debugging > Web Vitals & resources: the badges and thresholds (RB-060-10).
    #[serde(default)]
    pub vitals: crate::VitalsSettings,
    /// DevTools & Debugging > Web Vitals & resources: CPU and memory per view (RB-060-10).
    #[serde(default)]
    pub resources: crate::ResourcesSettings,
    /// DevTools & Debugging > Framework DevTools: the Vue and React DevTools backends in the
    /// views' pages (RB-080-24).
    #[serde(default)]
    pub framework_devtools: crate::FrameworkDevtoolsSettings,
    /// Reload & Files: the polling fallback (RB-030-05).
    #[serde(default)]
    pub reload: UserReloadSettings,
    /// General > updates (inactive until the move to GitHub, RB-090-02).
    #[serde(default)]
    pub updates: UpdateSettings,
    /// Keyboard Shortcuts (RB-010-17).
    #[serde(default)]
    pub shortcuts: ShortcutSettings,
    /// Breakpoint shortcuts (RB-010-16).
    #[serde(default)]
    pub breakpoint_shortcuts: BreakpointShortcutSettings,
    /// The command palette (RB-060-11).
    #[serde(default)]
    pub palette: crate::PaletteSettings,
    /// The user's own device presets (RB-010-12).
    #[serde(default)]
    pub devices: DeviceSettings,
    /// Emulation > geolocation, time zone and CPU: the shortcuts' values and own presets
    /// (RB-080-10).
    #[serde(default)]
    pub emulation: EmulationUserSettings,
    /// Logging and diagnostics (RB-010-10, read as `rb_diag::LogSettings`).
    #[serde(default)]
    pub logs: LogSettings,
    /// Settings > Plugins (RB-010-03); every key that widens a plugin is protected from MCP.
    #[serde(default)]
    pub plugins: PluginSettings,
    /// Security > Sessions: whether session cookies outlive the app (RB-020-07).
    #[serde(default)]
    pub sessions: crate::SessionSettings,
    /// Reload & Files > WSL watcher and the WSL networking mode (RB-030-04, read as
    /// `rb_wsl::settings::WslSettings`).
    #[serde(default)]
    pub wsl: WslSettings,
    /// Checks > Findings (RB-060-01): the user's values of the keys the project schema
    /// describes (`checks.*`, scopes project, local and user); the project wins per key.
    #[serde(default, skip_serializing_if = "crate::shared_settings::is_default")]
    #[schemars(skip)]
    pub checks: crate::FindingsSettings,
    /// Capture > Screenshots (RB-050-03): the user's values of the project schema's
    /// `capture.*`; the project wins per key, and only here may `capture.dir` be absolute.
    #[serde(default, skip_serializing_if = "crate::shared_settings::is_default")]
    #[schemars(skip)]
    pub capture: crate::CaptureConfig,
    /// Capture > Video: an ffmpeg of the user's own, never a project's (RB-080-02).
    #[serde(default, skip_serializing_if = "crate::shared_settings::is_default")]
    pub tools: crate::ToolsSettings,
    /// Project > bookmarks: whether a new bookmark follows the environment (RB-080-48).
    #[serde(default)]
    pub bookmarks: crate::user_view::BookmarkSettings,
    /// Plugins > Installed: the plugin panels floating or docked in the window (RB-080-57).
    #[serde(default)]
    pub panels: crate::PanelSettings,
    /// Layout > zoom: the page zoom inside a view (RB-090-20).
    #[serde(default)]
    pub views: crate::ViewsSettings,
    /// Plugin sections and anything unknown, kept verbatim.
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// `ui`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct UiSettings {
    /// The UI language: one of `rb_i18n::available_locales()` (only `en` in phase 1,
    /// RB-010-14).
    #[schemars(extend("enum" = rb_i18n::available_locales()))]
    pub language: String,
    /// Light, dark or follow the system (RB-010-07).
    pub theme: Theme,
    /// Layout > status bar (RB-080-15).
    pub status_bar: crate::StatusBarSettings,
    /// The label strip of the active view (the leader) is drawn in the accent colour, in every
    /// layout with label strips - grid, focus, horizontal, split; the canvas board marks it
    /// itself (RB-080-04).
    pub highlight_active_view: bool,
    /// Every view's label strip ends in a badge naming its engine (Chromium, Firefox, WebKit);
    /// a click on it chooses another engine for the view's breakpoint (RB-080-33). The canvas
    /// board draws the same badge. Off hides the badge; the chord still opens the choice.
    pub engine_badge: bool,
    /// The page's context menu offers rBrowser's block below Chromium's entries: Inspect
    /// element, DevTools, Screenshot, Video, Checks and Panels for the clicked view
    /// (RB-080-43). Off leaves Chromium's menu and "Do not sync the next click"; the commands
    /// and their chords stay.
    pub page_context_menu: bool,
    /// The menus of the views (RB-080-41): on a view's label strip (the active view's click, a
    /// right-click on the canvas board's label) "Remove view", "Set as active", "Size", the
    /// engine, "Pop out"; on the free canvas, an empty cell or the toolbar's "+" the hidden
    /// breakpoints to add. Off leaves the strips and the canvas as before; the "+" button, the
    /// commands, the palette and MCP stay.
    pub view_context_menu: bool,
    /// Layout > the breakpoint bar under every view (RB-080-44).
    pub view_footer: crate::ViewFooterSettings,
    /// The page's context menu offers the toolbar's emulation switches for the clicked view -
    /// colour scheme, reduced motion, offline, location, time zone, reset - in an Emulation
    /// submenu, and "Analyze breakpoints" beside it (RB-080-49). Off, the submenu is gone and
    /// "Analyze breakpoints" stays under Checks. Needs `ui.pageContextMenu`.
    pub page_context_menu_emulation: bool,
    /// Layout > what the "New size..." picker lists (RB-080-46).
    pub new_view_size: crate::NewViewSizeSettings,
    /// The toolbar in a narrow window (RB-080-52) and its optional buttons.
    pub toolbar: crate::ToolbarSettings,
    /// Code shows in syntax colours wherever rBrowser shows it - the plugin panels' values,
    /// stack traces, request and response bodies and headers, snippets and storage values, and
    /// the settings window's JSON view (RB-080-65). Off, code is one colour; more than 200 KiB is
    /// always one colour.
    pub syntax_highlight: bool,
    /// Where a link from rBrowser's own surfaces opens - a finding's help page, a plugin panel's
    /// report or help link: `system` the default browser, `activeView` the grid's active view,
    /// `ask` asks on every link (RB-080-92).
    pub open_links_in: crate::OpenLinksIn,
    /// rBrowser's own windows - the toolbar, status bar, label strips, breakpoint bars, settings,
    /// palette and panels - grow with Windows' text size (Settings > Accessibility > Text size,
    /// 100 to 225 %) and follow a change at once (RB-090-26). Off keeps them at 100 %. The
    /// pages in the views are never affected; their zoom is the page zoom.
    pub follow_windows_text_size: bool,
    /// The activity bar while something runs, its place and its entries (RB-090-67).
    pub activity_bar: crate::ActivityBarSettings,
    /// Layout > what each view's label strip shows beside its name (RB-090-27).
    pub cell_label: crate::CellLabelSettings,
    /// Local time or UTC in the folder and file names a person reads (RB-090-69).
    pub file_name_time: crate::FileNameTime,
}

impl Default for UiSettings {
    fn default() -> Self {
        Self {
            language: "en".to_owned(),
            theme: Theme::System,
            status_bar: crate::StatusBarSettings::default(),
            highlight_active_view: true,
            engine_badge: true,
            page_context_menu: true,
            view_context_menu: true,
            view_footer: crate::ViewFooterSettings::default(),
            page_context_menu_emulation: true,
            new_view_size: crate::NewViewSizeSettings::default(),
            toolbar: crate::ToolbarSettings::default(),
            syntax_highlight: true,
            open_links_in: crate::OpenLinksIn::System,
            follow_windows_text_size: true,
            activity_bar: crate::ActivityBarSettings::default(),
            cell_label: crate::CellLabelSettings::default(),
            file_name_time: crate::FileNameTime::Local,
        }
    }
}

/// `cdp`: the external CDP proxy.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct CdpSettings {
    /// Off by default.
    pub enabled: bool,
    /// A fixed port; random when absent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
}

/// `mcp`. Protected from MCP itself.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct McpSettings {
    /// The MCP server runs.
    pub enabled: bool,
    /// The `evaluate` tool may run page JavaScript.
    pub allow_evaluate: bool,
    /// A fixed port on 127.0.0.1; a free one at every start when absent (RB-030-01).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    /// How long MCP waits for each view, in milliseconds: the final URL of `navigate`,
    /// `reload`, `back` and `forward`, the input of `click`, `type`, `hover` and `press_key`,
    /// and `sync_set`'s work (RB-030-01).
    #[schemars(range(min = 1000, max = 600_000))]
    pub navigation_timeout_ms: u32,
    /// Console messages kept per view for `console_logs` (RB-030-01).
    #[schemars(range(min = 10, max = 10_000))]
    pub console_log_limit: u32,
    /// Network requests kept per view for `network_requests` (RB-030-01).
    #[schemars(range(min = 10, max = 10_000))]
    pub network_request_limit: u32,
    /// Uncaught exceptions kept per view for `errors` (RB-030-01).
    #[schemars(range(min = 10, max = 10_000))]
    pub error_limit: u32,
    /// The most bytes `evaluate` and `snapshot` answer; more is cut (RB-030-01).
    #[schemars(range(min = 4096, max = 16_777_216))]
    pub max_answer_bytes: u32,
    /// `rbrowser mcp` starts the project's rBrowser in the background when none runs
    /// (RB-030-02).
    pub stdio_auto_start: bool,
    /// A setting changed over MCP is shown in the shell (RB-030-03).
    pub notify_on_setting_change: bool,
}

impl Default for McpSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            allow_evaluate: false,
            port: None,
            navigation_timeout_ms: 15_000,
            console_log_limit: 500,
            network_request_limit: 500,
            error_limit: 200,
            max_answer_bytes: 262_144,
            stdio_auto_start: false,
            notify_on_setting_change: true,
        }
    }
}

/// `engines`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct EngineSettings {
    /// Chromium, the engine of the grid (RB-010-04).
    pub chromium: ChromiumSettings,
    /// A Node runtime of one's own instead of the bundled one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_path: Option<String>,
    /// How long a request to the Node sidecar waits for its answer, in milliseconds; a request
    /// that takes longer is cancelled (RB-040-03).
    #[schemars(range(min = 1000, max = 600_000))]
    pub node_rpc_timeout_ms: u32,
    /// The folder of the Playwright browsers (WebKit); `%LOCALAPPDATA%\rBrowser\ms-playwright`
    /// when absent (RB-040-03).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub playwright_browsers_path: Option<String>,
    /// The Chromium-based browser Lighthouse runs in; absent, an installed one is found
    /// (RB-080-58).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lighthouse_browser_path: Option<String>,
    /// The Firefox executable before RB-040-02 (`engines.firefoxPath`): still read, never
    /// written, outside the schema; `engines.firefox.path` wins
    /// ([`EngineSettings::firefox_executable`]).
    #[serde(skip_serializing)]
    #[schemars(skip)]
    pub firefox_path: Option<String>,
    /// The external windows of Firefox and WebKit views (RB-040-01).
    pub external: ExternalWindowSettings,
    /// Firefox over WebDriver BiDi (RB-040-02).
    pub firefox: FirefoxSettings,
    /// WebKit through Playwright (RB-040-04).
    pub webkit: WebkitSettings,
    /// Copying Chromium's cookies into the Firefox and WebKit views (RB-040-05).
    pub cookies: CookieSettings,
}

impl Default for EngineSettings {
    fn default() -> Self {
        Self {
            chromium: ChromiumSettings::default(),
            node_path: None,
            node_rpc_timeout_ms: 30_000,
            playwright_browsers_path: None,
            lighthouse_browser_path: None,
            firefox_path: None,
            external: ExternalWindowSettings::default(),
            firefox: FirefoxSettings::default(),
            webkit: WebkitSettings::default(),
            cookies: CookieSettings::default(),
        }
    }
}

/// `engines.chromium`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct ChromiumSettings {
    /// Run the Chromium processes in the sandbox (ADR 0001, Decision 6). Off only where the
    /// sandbox fails on a machine; read at start-up, and while it is off the log says so at
    /// every start. `--no-sandbox` on the command line switches it off for one start.
    pub sandbox: bool,
    /// Chrome extensions from unpacked folders (RB-080-06).
    #[serde(skip_serializing_if = "crate::shared_settings::is_default")]
    pub extensions: crate::ChromiumExtensionSettings,
}

impl Default for ChromiumSettings {
    fn default() -> Self {
        Self {
            sandbox: true,
            extensions: crate::ChromiumExtensionSettings::default(),
        }
    }
}

/// `devtools` (RB-010-06). Whether rBrowser's shortcuts pause while DevTools have focus is
/// `shortcuts.disableInDevtools`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct DevtoolsSettings {
    /// Where DevTools open.
    pub mode: DevtoolsMode,
    /// Docked DevTools follow the active view; off, they stay with the view they were opened
    /// for.
    pub follow_active_view: bool,
    /// The height of DevTools docked below the grid, in DIP; the splitter sets it. The grid
    /// keeps at least a quarter of the window.
    #[schemars(range(min = 120, max = 4000))]
    pub dock_height: u32,
    /// The width of DevTools docked beside the grid, in DIP; the splitter sets it.
    #[schemars(range(min = 120, max = 4000))]
    pub dock_width: u32,
    /// The hosted frontend's screencast beside the panels (the inspected page once more, in
    /// DevTools' own picture). Off, the panels take the whole dock and inspect mode picks in
    /// the view itself (RB-080-21).
    pub screencast: bool,
    /// "Inspect element" (the page's context menu, the shortcut) opens DevTools for the view
    /// when they are closed or with another view, then selects the element. Off, it selects
    /// only in DevTools already open for that view. MCP's `devtools_inspect_at` always opens.
    pub open_on_inspect: bool,
}

impl Default for DevtoolsSettings {
    fn default() -> Self {
        Self {
            mode: DevtoolsMode::default(),
            follow_active_view: true,
            dock_height: 320,
            dock_width: 480,
            screencast: false,
            open_on_inspect: true,
        }
    }
}

/// Where DevTools open (ADR 0001, Decision 5).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum DevtoolsMode {
    /// Docked below the grid (hosted frontend).
    #[default]
    DockedBottom,
    /// Docked beside the grid (hosted frontend).
    DockedRight,
    /// A window of its own (`show_dev_tools`, the native frontend).
    Window,
}

/// `reload` of the user settings.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct UserReloadSettings {
    /// Poll instead of relying on file events.
    pub polling_fallback: bool,
    /// How often polling looks for changes, in milliseconds (RB-030-05).
    #[schemars(range(min = 250))]
    pub poll_interval_ms: u64,
    /// How long input in a view pauses the interval reload, in seconds (RB-030-05).
    pub interval_pause_sec: u64,
}

impl Default for UserReloadSettings {
    fn default() -> Self {
        Self {
            polling_fallback: false,
            poll_interval_ms: 1000,
            interval_pause_sec: 10,
        }
    }
}

/// `wsl` (RB-030-04).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct WslSettings {
    /// Another rbrowser-wsl helper than the one in the package, as a Linux path or a Windows
    /// path; empty for the packaged one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub helper_path: Option<String>,
    /// The WSL distribution the helper runs in; empty for the project's distribution, else WSL's
    /// default.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub distro: Option<String>,
    /// The WSL networking mode: detected (auto), or mirrored or NAT set by hand.
    pub networking_mode_override: WslNetworkingMode,
}

/// `wsl.networkingModeOverride`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum WslNetworkingMode {
    /// Detect it (`wslinfo`, `.wslconfig`, NAT).
    #[default]
    Auto,
    /// Treat every distribution as mirrored.
    Mirrored,
    /// Treat every distribution as NAT.
    Nat,
}

/// `devices`: the user's device presets (RB-010-12).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct DeviceSettings {
    /// `true`: `items` replace the built-in list; `false`: they are added (same id wins).
    pub replace_defaults: bool,
    /// The presets. A breakpoint whose `device` names a preset takes its viewport, pixel ratio,
    /// user agent, platform and touch for every field it leaves out; what the breakpoint sets
    /// itself wins (RB-010-12). Picking a preset for the active view sizes it to the viewport
    /// (RB-010-16).
    pub items: Vec<DevicePreset>,
}

#[cfg(test)]
#[path = "user_settings_tests.rs"]
mod tests;
