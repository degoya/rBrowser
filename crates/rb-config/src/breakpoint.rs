//! A breakpoint, the emulation fields it carries, and the device presets it may reference.
//!
//! The model is the contract; RB-010-01 loads, merges and validates it, RB-010-05 maps the
//! emulation fields onto CDP, RB-010-12 resolves `device`, RB-010-16 registers `shortcut`.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{Geolocation, Paper};

/// The `navigator.platform` values the settings window suggests (`x-options`, RB-080-20); any
/// other string stays allowed.
pub const PLATFORMS: [&str; 6] = [
    "Win32",
    "MacIntel",
    "Linux x86_64",
    "Linux armv81",
    "iPhone",
    "iPad",
];

/// The BCP 47 locales the settings window suggests; any other stays allowed.
pub const LOCALES: [&str; 10] = [
    "en-US", "en-GB", "de-DE", "de-AT", "de-CH", "fr-FR", "es-ES", "it-IT", "nl-NL", "ja-JP",
];

/// The IANA time zones the settings window suggests; any other stays allowed.
pub const TIMEZONES: [&str; 8] = [
    "UTC",
    "Europe/Berlin",
    "Europe/London",
    "America/New_York",
    "America/Los_Angeles",
    "America/Sao_Paulo",
    "Asia/Tokyo",
    "Australia/Sydney",
];

/// The engine a view runs on. Serialized lowercase. Only `chromium` is embedded in 0.1;
/// Firefox and WebKit arrive with RB-040-02 / RB-040-04 as external windows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum EngineKind {
    /// CEF, embedded in the grid.
    #[default]
    Chromium,
    /// The installed Firefox over WebDriver BiDi.
    Firefox,
    /// Playwright's WebKit build ("WebKit, not Safari").
    Webkit,
}

impl EngineKind {
    /// The name in the config, the command arguments and the MCP answers (`firefox`): what
    /// serde writes, without a round trip.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            EngineKind::Chromium => "chromium",
            EngineKind::Firefox => "firefox",
            EngineKind::Webkit => "webkit",
        }
    }
}

/// One breakpoint of `breakpoints[]`: a size plus the emulation that makes it a device.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Breakpoint {
    /// Unique among the breakpoints; referenced by `layout.cells[].breakpoint`.
    pub id: String,
    /// The label shown in the cell; the id when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Viewport width in CSS px. May be left out when `device` names a device preset: the
    /// preset's viewport width applies.
    // Consumers read the size through `Breakpoint::viewport`, never this field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    /// Viewport height in CSS px. May be left out when `device` names a device preset: the
    /// preset's viewport height applies.
    // Consumers read the size through `Breakpoint::viewport`, never this field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    /// A device preset id (`devices.json` or the user's `devices`) whose values fill the fields
    /// this breakpoint leaves out - width and height included; whatever the breakpoint sets
    /// wins (RB-010-12).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device: Option<String>,
    /// The engine; `chromium` by default.
    #[serde(default)]
    pub engine: EngineKind,
    /// The session (cookie/storage partition) of `sessions[]`; `default` when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    /// Whether the view follows synced navigation; `true` by default.
    #[serde(default = "yes")]
    pub sync_navigation: bool,
    /// A shortcut of its own that sets the active view to this breakpoint, in the text form of
    /// `rb-commands` (`"Ctrl+Shift+F9"`); checked against the reserved list (RB-010-16).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shortcut: Option<String>,
    /// The colour of the breakpoint bar under the breakpoint's views, `#rrggbb` (RB-080-44); one
    /// of a fixed palette by the breakpoint's place in the list when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// The emulation fields, flattened into the breakpoint object.
    #[serde(flatten)]
    pub emulation: EmulationFields,
}

fn yes() -> bool {
    true
}

/// The emulation a breakpoint (or `emulation` defaults of the project) may set. Every field is
/// optional: an absent field falls back to the device preset, then the project defaults, then
/// the engine's own default (RB-010-05 resolves the chain).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EmulationFields {
    /// Device pixel ratio.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dpr: Option<f64>,
    /// Touch input (`maxTouchPoints`, touch events, `pointer: coarse`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub touch: Option<bool>,
    /// Mobile device type (meta viewport, overlay scrollbars); defaults to `touch`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mobile: Option<bool>,
    /// User agent override; `null` keeps the engine's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_agent: Option<String>,
    /// `navigator.platform` and UA-CH platform override.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-options" = PLATFORMS))]
    pub platform: Option<String>,
    /// BCP 47 locale: Intl, `navigator.language`, `Accept-Language`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-options" = LOCALES))]
    pub locale: Option<String>,
    /// `prefers-color-scheme`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color_scheme: Option<ColorScheme>,
    /// `prefers-reduced-motion: reduce` when `true`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reduced_motion: Option<bool>,
    /// The other media features.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media: Option<MediaFeatures>,
    /// Network throttling: a preset name or custom conditions; `null` for none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network: Option<NetworkThrottle>,
    /// Extra request headers. Headers from `config.json` and `config.local.json` apply only in a
    /// trusted workspace; an untrusted one sends only headers set live (RB-010-09).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub headers: BTreeMap<String, String>,
    /// Screen orientation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub orientation: Option<Orientation>,
    /// Safe-area insets (notch, home indicator).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub safe_areas: Option<SafeAreaInsets>,
    /// Text scale factor (accessibility font scaling), `1.0` = none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_scale: Option<f64>,
    /// The emulated position, granted to the view's origin without a prompt; `null` switches
    /// a position of the device or the defaults off (RB-080-10).
    #[serde(
        default,
        deserialize_with = "crate::emulation_extras::explicit_null",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "Option<Geolocation>")]
    pub geolocation: Option<Option<Geolocation>>,
    /// IANA time zone (`America/New_York`); empty keeps the system's (RB-080-10).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-options" = TIMEZONES))]
    pub timezone: Option<String>,
    /// CPU throttling factor, 1 to 100; `1` is none (RB-080-10).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 1.0, max = 100.0))]
    pub cpu_throttling: Option<f64>,
    /// No network: over the throttling's conditions, `false` lifts an offline preset
    /// (RB-080-10).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offline: Option<bool>,
    /// The paper of a print view: width and height in CSS px at 96 dpi for a side the
    /// breakpoint leaves out (A4 794 px, Letter 816 px wide). With `media.mediaType: "print"`
    /// (RB-080-10).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paper: Option<Paper>,
}

/// `prefers-color-scheme`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ColorScheme {
    /// `light`.
    Light,
    /// `dark`.
    Dark,
}

/// The media features beyond color scheme and reduced motion (RB-010-05). Absent = not
/// emulated.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MediaFeatures {
    /// `prefers-reduced-data: reduce` when `true`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reduced_data: Option<bool>,
    /// `prefers-reduced-transparency: reduce` when `true`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reduced_transparency: Option<bool>,
    /// `forced-colors`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forced_colors: Option<ForcedColors>,
    /// `prefers-contrast`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contrast: Option<Contrast>,
    /// `color-gamut`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color_gamut: Option<ColorGamut>,
    /// The media type: `print` emulates print media.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media_type: Option<MediaType>,
}

/// `forced-colors`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ForcedColors {
    /// `none`.
    None,
    /// `active`.
    Active,
}

/// `prefers-contrast`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Contrast {
    /// `no-preference`.
    NoPreference,
    /// `more`.
    More,
    /// `less`.
    Less,
    /// `custom`.
    Custom,
}

/// `color-gamut`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ColorGamut {
    /// `srgb`.
    Srgb,
    /// `p3`.
    P3,
    /// `rec2020`.
    Rec2020,
}

/// The emulated CSS media type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum MediaType {
    /// `screen`.
    Screen,
    /// `print`.
    Print,
}

/// Network throttling: `"slow-3g"` or `{ "latencyMs": .., "downloadKbps": .., ... }`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum NetworkThrottle {
    /// A named preset.
    Preset(NetworkPreset),
    /// Custom conditions.
    Custom(NetworkConditions),
}

/// The throttling presets (values: RB-010-05, after Chrome DevTools).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub enum NetworkPreset {
    /// No network.
    #[serde(rename = "offline")]
    Offline,
    /// Slow 3G.
    #[serde(rename = "slow-3g")]
    Slow3g,
    /// Fast 3G.
    #[serde(rename = "fast-3g")]
    Fast3g,
    /// 4G.
    #[serde(rename = "4g")]
    FourG,
}

/// Custom network conditions (CDP `Network.emulateNetworkConditions`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NetworkConditions {
    /// No network at all.
    #[serde(default)]
    pub offline: bool,
    /// Added round-trip latency in ms.
    #[serde(default)]
    pub latency_ms: u32,
    /// Download throughput in kbit/s; 0 = unthrottled.
    #[serde(default)]
    pub download_kbps: u32,
    /// Upload throughput in kbit/s; 0 = unthrottled.
    #[serde(default)]
    pub upload_kbps: u32,
}

/// Screen orientation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Orientation {
    /// Portrait.
    Portrait,
    /// Landscape.
    Landscape,
}

/// Safe-area insets in CSS px.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SafeAreaInsets {
    /// Top inset.
    #[serde(default)]
    pub top: u32,
    /// Right inset.
    #[serde(default)]
    pub right: u32,
    /// Bottom inset.
    #[serde(default)]
    pub bottom: u32,
    /// Left inset.
    #[serde(default)]
    pub left: u32,
}

/// A device preset: the built-in list `crates/rb-config/data/devices.json` (RB-010-12) and the
/// user's own (`devices` in the user settings).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DevicePreset {
    /// Unique id, referenced by `breakpoints[].device` (e.g. `iphone-15`).
    pub id: String,
    /// The display name; built-in presets use an i18n key (`devices.<id>`) instead.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Mobile, tablet or desktop: groups the quick pick.
    pub kind: DeviceKind,
    /// Viewport in CSS px.
    pub viewport: Viewport,
    /// Device pixel ratio.
    pub dpr: f64,
    /// User agent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ua: Option<String>,
    /// `navigator.platform`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-options" = PLATFORMS))]
    pub platform: Option<String>,
    /// Touch input.
    #[serde(default)]
    pub touch: bool,
    /// Default orientation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub orientation: Option<Orientation>,
    /// Safe-area insets.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub safe_areas: Option<SafeAreaInsets>,
}

/// The kind of a device preset.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum DeviceKind {
    /// A phone.
    Mobile,
    /// A tablet.
    Tablet,
    /// A desktop or laptop.
    Desktop,
}

/// A viewport size in CSS px.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Viewport {
    /// Width.
    pub width: u32,
    /// Height.
    pub height: u32,
}
