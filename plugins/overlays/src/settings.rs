//! The plugin's settings (`settings.schema.json`) as the page script needs them, each checked
//! against its schema; a missing or broken value is its default.

use rb_plugin_api::PluginHost;
use serde::Serialize;
use serde_json::Value;

/// The default colours: border and fill of each mark (`colors.*`).
pub const DEFAULT_OVERFLOW: &str = "#f5b800";
pub const DEFAULT_CAUSE: &str = "#e5262b";
pub const DEFAULT_VW100: &str = "#1f6feb";
pub const DEFAULT_HIGHLIGHT: &str = "#c026d3";
/// The default and the bounds of `overflow.mutationThrottleMs`.
pub const DEFAULT_THROTTLE_MS: u64 = 250;
const THROTTLE_MS: std::ops::RangeInclusive<u64> = 50..=5000;
/// The defaults and bounds of the further overlays' settings (RB-070-07).
pub const DEFAULT_TARGET_SIZE_PX: u64 = 24;
const TARGET_SIZE_PX: std::ops::RangeInclusive<u64> = 8..=96;
pub const DEFAULT_SMALL_TEXT_PX: u64 = 12;
const SMALL_TEXT_PX: std::ops::RangeInclusive<u64> = 6..=32;
pub const DEFAULT_SCROLLBAR_TOLERANCE_PX: u64 = 4;
const SCROLLBAR_TOLERANCE_PX: std::ops::RangeInclusive<u64> = 0..=100;
pub const DEFAULT_CHAOS_FACTOR: f64 = 2.0;
const CHAOS_FACTOR: std::ops::RangeInclusive<f64> = 0.1..=10.0;

/// The colours of the marks, `#rrggbb` each.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Colors {
    pub overflow: String,
    pub cause: String,
    pub vw100: String,
    pub highlight: String,
}

/// What the page script needs of the settings.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageSettings {
    pub mutation_throttle_ms: u64,
    pub show100vw: bool,
    pub containers_only: bool,
    pub colors: Colors,
    pub target_size_px: u64,
    pub small_text_px: u64,
    pub scrollbar_tolerance_px: u64,
    pub chaos_factor: f64,
    pub css_off_include_inline: bool,
    /// The tab order overlay (RB-070-12).
    pub tab_order_show_lines: bool,
    pub tab_order_live: bool,
    pub tab_order_include_iframes: bool,
}

/// The value of the plugin's key `key` (`overflow.show100vw`); the host may name it with the
/// plugin's id in front (`overlays.overflow.show100vw`), so both are asked.
fn get(host: &mut dyn PluginHost, key: &str) -> Option<Value> {
    host.setting(key)
        .or_else(|| host.setting(&format!("{}.{key}", crate::PLUGIN_ID)))
        .and_then(|text| serde_json::from_str(&text).ok())
}

fn whole(
    host: &mut dyn PluginHost,
    key: &str,
    range: std::ops::RangeInclusive<u64>,
    default: u64,
) -> u64 {
    get(host, key)
        .and_then(|value| value.as_u64())
        .filter(|value| range.contains(value))
        .unwrap_or(default)
}

fn flag(host: &mut dyn PluginHost, key: &str, default: bool) -> bool {
    get(host, key)
        .and_then(|value| value.as_bool())
        .unwrap_or(default)
}

/// Whether `text` is a colour `#rrggbb`.
#[must_use]
pub fn is_color(text: &str) -> bool {
    text.len() == 7
        && text.starts_with('#')
        && text[1..].chars().all(|digit| digit.is_ascii_hexdigit())
}

fn color(host: &mut dyn PluginHost, key: &str, default: &str) -> String {
    get(host, key)
        .and_then(|value| value.as_str().map(str::to_owned))
        .filter(|text| is_color(text))
        .unwrap_or_else(|| default.to_owned())
}

/// `restoreOnStart`: whether a start keeps the overlays of the last session.
pub fn restore_on_start(host: &mut dyn PluginHost) -> bool {
    flag(host, "restoreOnStart", true)
}

/// `reportFindings`: whether switching a checking overlay on reports its hits as findings.
pub fn report_findings(host: &mut dyn PluginHost) -> bool {
    flag(host, "reportFindings", true)
}

/// The settings the page script draws with.
pub fn page_settings(host: &mut dyn PluginHost) -> PageSettings {
    let mutation_throttle_ms = get(host, "overflow.mutationThrottleMs")
        .and_then(|value| value.as_u64())
        .filter(|ms| THROTTLE_MS.contains(ms))
        .unwrap_or(DEFAULT_THROTTLE_MS);
    PageSettings {
        mutation_throttle_ms,
        show100vw: flag(host, "overflow.show100vw", true),
        containers_only: flag(host, "outlines.containersOnly", false),
        colors: Colors {
            overflow: color(host, "colors.overflow", DEFAULT_OVERFLOW),
            cause: color(host, "colors.cause", DEFAULT_CAUSE),
            vw100: color(host, "colors.vw100", DEFAULT_VW100),
            highlight: color(host, "colors.highlight", DEFAULT_HIGHLIGHT),
        },
        target_size_px: whole(host, "targetSizePx", TARGET_SIZE_PX, DEFAULT_TARGET_SIZE_PX),
        small_text_px: whole(host, "smallTextPx", SMALL_TEXT_PX, DEFAULT_SMALL_TEXT_PX),
        scrollbar_tolerance_px: whole(
            host,
            "scrollbarTolerancePx",
            SCROLLBAR_TOLERANCE_PX,
            DEFAULT_SCROLLBAR_TOLERANCE_PX,
        ),
        chaos_factor: get(host, "chaos.factor")
            .and_then(|value| value.as_f64())
            .filter(|factor| CHAOS_FACTOR.contains(factor))
            .unwrap_or(DEFAULT_CHAOS_FACTOR),
        css_off_include_inline: flag(host, "cssOff.includeInline", false),
        tab_order_show_lines: flag(host, "tabOrder.showLines", true),
        tab_order_live: flag(host, "tabOrder.live", true),
        tab_order_include_iframes: flag(host, "tabOrder.includeIframes", true),
    }
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
