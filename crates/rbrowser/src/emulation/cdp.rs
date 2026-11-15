//! The CDP mapping of a view's [`Emulation`] (RB-010-05): which commands bring a session from
//! what it has to what it should have. Pure, so the mapping is tested without a browser; the
//! host sends the calls (`host/attach.rs`).
//!
//! rBrowser owns `Emulation.setDeviceMetricsOverride` (ADR 0001): [`metrics_params`] adds the
//! screen orientation to the metrics the host fits into the cell. Everything else is one group
//! per CDP command, sent when it changed - or, after a reconnect or `reapply_emulation`, again.
//! The extras of RB-080-10 (geolocation, time zone, CPU) are [`super::cdp_extras`].

use rb_config::{ColorScheme, Contrast, ForcedColors, MediaType, NetworkConditions, Orientation};
use rb_engine::Emulation;
use serde_json::{Map, Value, json};

use crate::fit::DeviceMetrics;

/// The touch points an emulated touch screen reports (`navigator.maxTouchPoints`).
pub const MAX_TOUCH_POINTS: u32 = 5;
/// Chromium's default font sizes in px, scaled by `textScale`.
const STANDARD_FONT_PX: f64 = 16.0;
const FIXED_FONT_PX: f64 = 13.0;

/// One CDP command for a view's session.
#[derive(Clone, Debug, PartialEq)]
pub struct CdpCall {
    /// The method.
    pub method: &'static str,
    /// Its parameters.
    pub params: Value,
}

impl CdpCall {
    pub(crate) fn new(method: &'static str, params: Value) -> Self {
        Self { method, params }
    }
}

/// The parameters of `Emulation.setDeviceMetricsOverride`: `metrics` plus the screen
/// orientation (absent: the display's).
pub fn metrics_params(metrics: &DeviceMetrics, orientation: Option<Orientation>) -> Value {
    let mut params = metrics.to_params();
    if let (Some(orientation), Some(object)) = (orientation, params.as_object_mut()) {
        let (kind, angle) = match orientation {
            Orientation::Portrait => ("portraitPrimary", 0),
            Orientation::Landscape => ("landscapePrimary", 90),
        };
        object.insert(
            "screenOrientation".to_owned(),
            json!({ "type": kind, "angle": angle }),
        );
    }
    params
}

/// The calls that bring a session from `previous` (what rBrowser sent it; `None`: nothing) to
/// `next`, whose headers are already the ones allowed (workspace trust). With `full` the
/// commands DevTools' device toolbar also sends (media, touch, user agent) go again even when
/// unchanged, and every group that `next` sets. The network domain is enabled only for a view
/// that throttles or sends headers.
pub fn emulation_calls(previous: Option<&Emulation>, next: &Emulation, full: bool) -> Vec<CdpCall> {
    let unset = Emulation::default();
    let before = previous.unwrap_or(&unset);
    let mut calls = Vec::new();
    let media = (&next.color_scheme, &next.reduced_motion, &next.media);
    if full || media != (&before.color_scheme, &before.reduced_motion, &before.media) {
        calls.push(CdpCall::new(
            "Emulation.setEmulatedMedia",
            media_params(next),
        ));
    }
    if full || next.touch != before.touch {
        let params = if next.touch {
            json!({ "enabled": true, "maxTouchPoints": MAX_TOUCH_POINTS })
        } else {
            json!({ "enabled": false })
        };
        calls.push(CdpCall::new("Emulation.setTouchEmulationEnabled", params));
    }
    if full || user_agent_group(next) != user_agent_group(before) {
        calls.push(CdpCall::new(
            "Emulation.setUserAgentOverride",
            user_agent_params(next),
        ));
    }
    if (full && next.locale.is_some()) || next.locale != before.locale {
        let params = match &next.locale {
            Some(locale) => json!({ "locale": locale }),
            None => json!({}),
        };
        calls.push(CdpCall::new("Emulation.setLocaleOverride", params));
    }
    if (full && next.text_scale.is_some()) || next.text_scale != before.text_scale {
        let scale = next.text_scale.unwrap_or(1.0);
        calls.push(CdpCall::new(
            "Page.setFontSizes",
            json!({ "fontSizes": {
                "standard": (STANDARD_FONT_PX * scale).round(),
                "fixed": (FIXED_FONT_PX * scale).round(),
            } }),
        ));
    }
    if (full && next.safe_areas.is_some()) || next.safe_areas != before.safe_areas {
        let insets = next.safe_areas.map_or_else(
            || json!({}),
            |insets| {
                json!({
                    "top": insets.top,
                    "right": insets.right,
                    "bottom": insets.bottom,
                    "left": insets.left,
                })
            },
        );
        calls.push(CdpCall::new(
            "Emulation.setSafeAreaInsetsOverride",
            json!({ "insets": insets }),
        ));
    }
    let network = (full && next.network.is_some()) || next.network != before.network;
    let headers = (full && !next.headers.is_empty()) || next.headers != before.headers;
    if (network || headers) && (next.network.is_some() || !next.headers.is_empty()) {
        calls.push(CdpCall::new("Network.enable", json!({})));
    }
    if network {
        calls.push(CdpCall::new(
            "Network.emulateNetworkConditions",
            network_params(next.network.as_ref()),
        ));
    }
    if headers {
        calls.push(CdpCall::new(
            "Network.setExtraHTTPHeaders",
            json!({ "headers": next.headers }),
        ));
    }
    calls.extend(super::cdp_extras::extras_calls(before, next, full));
    calls
}

/// `Emulation.setEmulatedMedia`: the media type and every feature that is set; a `false`
/// preference is emulated as `no-preference`. The capture preparation (RB-080-12) adds
/// `prefers-reduced-motion: reduce` to it and sends it back afterwards.
pub fn media_params(emulation: &Emulation) -> Value {
    let reduce = |on: bool| if on { "reduce" } else { "no-preference" };
    let media = &emulation.media;
    let mut features: Vec<(&str, &str)> = Vec::new();
    if let Some(scheme) = emulation.color_scheme {
        let value = match scheme {
            ColorScheme::Light => "light",
            ColorScheme::Dark => "dark",
        };
        features.push(("prefers-color-scheme", value));
    }
    if let Some(on) = emulation.reduced_motion {
        features.push(("prefers-reduced-motion", reduce(on)));
    }
    if let Some(on) = media.reduced_data {
        features.push(("prefers-reduced-data", reduce(on)));
    }
    if let Some(on) = media.reduced_transparency {
        features.push(("prefers-reduced-transparency", reduce(on)));
    }
    if let Some(forced) = media.forced_colors {
        let value = match forced {
            ForcedColors::None => "none",
            ForcedColors::Active => "active",
        };
        features.push(("forced-colors", value));
    }
    if let Some(contrast) = media.contrast {
        let value = match contrast {
            Contrast::NoPreference => "no-preference",
            Contrast::More => "more",
            Contrast::Less => "less",
            Contrast::Custom => "custom",
        };
        features.push(("prefers-contrast", value));
    }
    if let Some(gamut) = media.color_gamut {
        let value = match gamut {
            rb_config::ColorGamut::Srgb => "srgb",
            rb_config::ColorGamut::P3 => "p3",
            rb_config::ColorGamut::Rec2020 => "rec2020",
        };
        features.push(("color-gamut", value));
    }
    let media_type = match media.media_type {
        Some(MediaType::Print) => "print",
        Some(MediaType::Screen) => "screen",
        None => "",
    };
    let features: Vec<Value> = features
        .into_iter()
        .map(|(name, value)| json!({ "name": name, "value": value }))
        .collect();
    json!({ "media": media_type, "features": features })
}

type UserAgentGroup<'a> = (
    &'a Option<String>,
    &'a Option<String>,
    &'a Option<String>,
    bool,
);

/// What `setUserAgentOverride` carries; `mobile` only matters with a user agent (its client
/// hints).
fn user_agent_group(emulation: &Emulation) -> UserAgentGroup<'_> {
    (
        &emulation.user_agent,
        &emulation.platform,
        &emulation.locale,
        emulation.user_agent.is_some() && emulation.mobile,
    )
}

/// `Emulation.setUserAgentOverride`: an empty user agent, platform or language keeps the
/// engine's own, so one call sets and clears all three. With a user agent come its client hints
/// (`userAgentMetadata`).
fn user_agent_params(emulation: &Emulation) -> Value {
    let mut params = Map::new();
    params.insert(
        "userAgent".to_owned(),
        json!(emulation.user_agent.clone().unwrap_or_default()),
    );
    if let Some(locale) = &emulation.locale {
        params.insert("acceptLanguage".to_owned(), json!(accept_language(locale)));
    }
    if let Some(platform) = &emulation.platform {
        params.insert("platform".to_owned(), json!(platform));
    }
    if let Some(user_agent) = &emulation.user_agent {
        params.insert(
            "userAgentMetadata".to_owned(),
            json!({
                "platform": hints_platform(user_agent, emulation.platform.as_deref()),
                "platformVersion": "",
                "architecture": "",
                "model": "",
                "mobile": emulation.mobile,
            }),
        );
    }
    Value::Object(params)
}

/// `Accept-Language` for `locale`: the tag, then its language (`de-DE,de;q=0.9`).
pub fn accept_language(locale: &str) -> String {
    match locale.split_once('-') {
        Some((language, _)) if !language.is_empty() => format!("{locale},{language};q=0.9"),
        _ => locale.to_owned(),
    }
}

/// The client hints platform (`Sec-CH-UA-Platform`) of a user agent and `navigator.platform`.
fn hints_platform(user_agent: &str, platform: Option<&str>) -> &'static str {
    let platform = platform.unwrap_or_default();
    let has = |needle: &str| user_agent.contains(needle) || platform.contains(needle);
    if has("iPhone") || has("iPad") || has("iPod") {
        "iOS"
    } else if has("Android") || platform.starts_with("Linux arm") {
        "Android"
    } else if has("CrOS") {
        "Chrome OS"
    } else if has("Win") {
        "Windows"
    } else if has("Mac") {
        "macOS"
    } else if has("Linux") {
        "Linux"
    } else {
        ""
    }
}

/// `Network.emulateNetworkConditions`: throughput in bytes/s, `-1` unthrottled; `None` lifts
/// every throttle.
fn network_params(conditions: Option<&NetworkConditions>) -> Value {
    let conditions = conditions.copied().unwrap_or_default();
    let throughput = |kbps: u32| {
        if kbps == 0 {
            -1.0
        } else {
            f64::from(kbps) * 1000.0 / 8.0
        }
    };
    json!({
        "offline": conditions.offline,
        "latency": conditions.latency_ms,
        "downloadThroughput": throughput(conditions.download_kbps),
        "uploadThroughput": throughput(conditions.upload_kbps),
    })
}

#[cfg(test)]
#[path = "cdp_tests.rs"]
mod tests;
