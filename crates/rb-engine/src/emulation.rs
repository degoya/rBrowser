//! The fully resolved emulation of one view: everything RB-010-05 sets through CDP.
//!
//! A [`rb_config::Breakpoint`] says what to emulate with optional fields; [`Emulation`] is the
//! result after device preset, project defaults and engine defaults are applied
//! ([`resolve_emulation`], RB-010-05). [`crate::ViewHost::apply_emulation`] takes it.

use std::collections::BTreeMap;

use rb_config::{
    Breakpoint, ColorScheme, DeviceKind, DevicePreset, EmulationFields, Geolocation, MediaFeatures,
    NetworkConditions, NetworkPreset, NetworkThrottle, Orientation, SafeAreaInsets, TextDirection,
};
use serde::{Deserialize, Serialize};

use crate::emulation_extras::{resolve_extras, with_offline};
use crate::{EngineError, Target, ViewId};

/// Everything a view emulates. `None` means "not overridden": the engine's own behaviour.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Emulation {
    /// Viewport width in CSS px.
    pub width: u32,
    /// Viewport height in CSS px.
    pub height: u32,
    /// Device pixel ratio.
    pub dpr: f64,
    /// Mobile device type (`setDeviceMetricsOverride.mobile`).
    pub mobile: bool,
    /// Touch emulation (`setTouchEmulationEnabled`, `maxTouchPoints`).
    pub touch: bool,
    /// User agent (`setUserAgentOverride`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_agent: Option<String>,
    /// `navigator.platform` / UA-CH platform.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub platform: Option<String>,
    /// Locale (`setLocaleOverride` + `Accept-Language`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,
    /// `prefers-color-scheme`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color_scheme: Option<ColorScheme>,
    /// `prefers-reduced-motion: reduce`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reduced_motion: Option<bool>,
    /// The other media features and the media type (`setEmulatedMedia`).
    #[serde(default)]
    pub media: MediaFeatures,
    /// Network throttling, presets expanded (`emulateNetworkConditions`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network: Option<NetworkConditions>,
    /// Extra request headers (`setExtraHTTPHeaders`); project headers only when trusted.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub headers: BTreeMap<String, String>,
    /// Screen orientation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub orientation: Option<Orientation>,
    /// Safe-area insets.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub safe_areas: Option<SafeAreaInsets>,
    /// Text scale factor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_scale: Option<f64>,
    /// The `scale` of `setDeviceMetricsOverride`. `None`: the host fits the viewport to the
    /// cell (`min(1, cell width / width)`, or the cell's fixed zoom) and reports the result in
    /// [`crate::ViewInfo::scale`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale: Option<f64>,
    /// The emulated position (`setGeolocationOverride`, granted to the view's origin;
    /// RB-080-10).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geolocation: Option<Geolocation>,
    /// IANA time zone (`setTimezoneOverride`; RB-080-10).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
    /// CPU throttling factor above 1 (`setCPUThrottlingRate`; RB-080-10).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu_throttling: Option<f64>,
    /// The text direction forced on the page's `<html dir>` by the sync agent (RB-092-02).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dir: Option<TextDirection>,
    /// The language forced on the page's `<html lang>` (BCP 47) by the sync agent (RB-092-02).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
}

impl Default for Emulation {
    fn default() -> Self {
        Self {
            width: 1280,
            height: 800,
            dpr: 1.0,
            mobile: false,
            touch: false,
            user_agent: None,
            platform: None,
            locale: None,
            color_scheme: None,
            reduced_motion: None,
            media: MediaFeatures::default(),
            network: None,
            headers: BTreeMap::new(),
            orientation: None,
            safe_areas: None,
            text_scale: None,
            scale: None,
            geolocation: None,
            timezone: None,
            cpu_throttling: None,
            dir: None,
            lang: None,
        }
    }
}

/// The arguments of the command `emulation.set` and of the MCP tool `emulation_set` (RB-010-05,
/// contract 0.1-w1b): `fields` are written over the target views' current emulation, live where
/// CDP allows. Shift-click in the shell sends the same with `target: "all"`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmulationSetArgs {
    /// `"all"` or a view id; the active view when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<Target>,
    /// The fields to set; absent fields stay as they are (`emulation.reset_view` goes back to
    /// the breakpoint).
    pub fields: EmulationFields,
}

/// The answer of `emulation.set`, `emulation.toggle_*` and `emulation.reset_view`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmulationChange {
    /// The views changed.
    pub views: Vec<ViewId>,
    /// A field CDP cannot change live was set: the shell shows "reload needed".
    pub reload_required: bool,
}

/// Resolves the emulation of `breakpoint`: its own fields, then its `device` preset (looked up
/// in `devices`), then the project's `emulation` defaults, then the engine defaults; network
/// presets expanded, `offline` folded into them; geolocation, time zone and CPU throttling as
/// RB-080-10 resolves them. The size is [`Breakpoint::viewport`]: the breakpoint's own sides,
/// its paper's or the preset's viewport for a side it leaves out (RB-010-12, RB-080-10). A DPR
/// left open is `0`, the display's own (RB-010-04); an orientation that
/// disagrees with the viewport's aspect turns it. Values a browser would refuse (a DPR outside
/// `(0, 10]`, a broken locale, control characters in a header) and an unknown device are
/// [`EngineError::Invalid`]; the messages never carry a header value. RB-010-05.
pub fn resolve_emulation(
    breakpoint: &Breakpoint,
    defaults: &EmulationFields,
    devices: &[DevicePreset],
) -> Result<Emulation, EngineError> {
    let own = &breakpoint.emulation;
    let device = match &breakpoint.device {
        Some(id) => Some(
            devices
                .iter()
                .find(|device| device.id == *id)
                .ok_or_else(|| {
                    EngineError::Invalid(format!(
                        "breakpoint {} names the unknown device {id}",
                        breakpoint.id
                    ))
                })?,
        ),
        None => None,
    };
    let dpr = own.dpr.or(device.map(|device| device.dpr)).or(defaults.dpr);
    if let Some(dpr) = dpr
        && !(MIN_DPR..=MAX_DPR).contains(&dpr)
    {
        return Err(invalid(breakpoint, "dpr", "is not in [0.25, 10]"));
    }
    let text_scale = own.text_scale.or(defaults.text_scale);
    if let Some(scale) = text_scale
        && !(scale > 0.0 && scale <= MAX_TEXT_SCALE)
    {
        return Err(invalid(breakpoint, "textScale", "is not in (0, 10]"));
    }
    let touch = own
        .touch
        .or(device.map(|device| device.touch))
        .or(defaults.touch)
        .unwrap_or(false);
    let mobile = own
        .mobile
        .or(device.map(|device| device.kind != DeviceKind::Desktop))
        .or(defaults.mobile)
        .unwrap_or(touch);
    let user_agent = own
        .user_agent
        .clone()
        .or_else(|| device.and_then(|device| device.ua.clone()))
        .or_else(|| defaults.user_agent.clone())
        .filter(|ua| !ua.is_empty());
    let platform = own
        .platform
        .clone()
        .or_else(|| device.and_then(|device| device.platform.clone()))
        .or_else(|| defaults.platform.clone())
        .filter(|platform| !platform.is_empty());
    for (field, value) in [("userAgent", &user_agent), ("platform", &platform)] {
        if value.as_deref().is_some_and(has_control) {
            return Err(invalid(breakpoint, field, "has a control character"));
        }
    }
    let locale = own
        .locale
        .clone()
        .or_else(|| defaults.locale.clone())
        .filter(|locale| !locale.is_empty());
    if locale.as_deref().is_some_and(|locale| !is_locale(locale)) {
        return Err(invalid(breakpoint, "locale", "is not a BCP 47 tag"));
    }
    // The page's `<html lang>` (RB-092-02): checked as the locale, so no markup reaches it.
    let lang = own
        .lang
        .clone()
        .or_else(|| defaults.lang.clone())
        .filter(|lang| !lang.is_empty());
    if lang.as_deref().is_some_and(|lang| !is_locale(lang)) {
        return Err(invalid(breakpoint, "lang", "is not a BCP 47 tag"));
    }
    let mut headers = defaults.headers.clone();
    headers.extend(own.headers.clone());
    for (name, value) in &headers {
        if !is_header_name(name) {
            return Err(invalid(
                breakpoint,
                "headers",
                "has a header name that is not a token",
            ));
        }
        if has_control(value) {
            return Err(EngineError::Invalid(format!(
                "breakpoint {}: the value of header {name} has a control character",
                breakpoint.id
            )));
        }
    }
    let orientation = own
        .orientation
        .or(device.and_then(|device| device.orientation))
        .or(defaults.orientation);
    let viewport = breakpoint.viewport(devices).ok_or_else(|| {
        invalid(
            breakpoint,
            "width and height",
            "are not set and no device preset supplies them",
        )
    })?;
    let (width, height) = oriented(viewport.width, viewport.height, orientation);
    let extras = resolve_extras(breakpoint, defaults)?;
    let network = own.network.or(defaults.network).map(network_conditions);
    Ok(Emulation {
        width,
        height,
        dpr: dpr.unwrap_or(DISPLAY_DPR),
        mobile,
        touch,
        user_agent,
        platform,
        locale,
        color_scheme: own.color_scheme.or(defaults.color_scheme),
        reduced_motion: own.reduced_motion.or(defaults.reduced_motion),
        media: merge_media(own.media.as_ref(), defaults.media.as_ref()),
        network: with_offline(network, own.offline.or(defaults.offline)),
        headers,
        orientation,
        safe_areas: own
            .safe_areas
            .or(device.and_then(|device| device.safe_areas))
            .or(defaults.safe_areas),
        text_scale,
        scale: None,
        geolocation: extras.geolocation,
        timezone: extras.timezone,
        cpu_throttling: extras.cpu_throttling,
        dir: own.dir.or(defaults.dir),
        lang,
    })
}

/// The DPR a breakpoint without one gets: `0`, the display's own.
const DISPLAY_DPR: f64 = 0.0;
/// The smallest DPR a breakpoint may set: below it a full-page capture would grow a view past any
/// memory (security review w23).
const MIN_DPR: f64 = 0.25;
/// The largest DPR (as `ViewHost::set_size`).
const MAX_DPR: f64 = 10.0;
/// The largest text scale.
const MAX_TEXT_SCALE: f64 = 10.0;

fn invalid(breakpoint: &Breakpoint, field: &str, why: &str) -> EngineError {
    EngineError::Invalid(format!("breakpoint {}: {field} {why}", breakpoint.id))
}

fn has_control(value: &str) -> bool {
    value.chars().any(char::is_control)
}

/// A BCP 47 tag as far as a header and ICU need it: ASCII letters, digits and `-`.
fn is_locale(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && !value.starts_with('-')
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

/// An HTTP token (RFC 9110).
fn is_header_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte))
}

/// Width and height turned to `orientation` (landscape: wider than high).
fn oriented(width: u32, height: u32, orientation: Option<Orientation>) -> (u32, u32) {
    match orientation {
        Some(Orientation::Landscape) if width < height => (height, width),
        Some(Orientation::Portrait) if width > height => (height, width),
        _ => (width, height),
    }
}

fn merge_media(own: Option<&MediaFeatures>, defaults: Option<&MediaFeatures>) -> MediaFeatures {
    let own = own.cloned().unwrap_or_default();
    let defaults = defaults.cloned().unwrap_or_default();
    MediaFeatures {
        reduced_data: own.reduced_data.or(defaults.reduced_data),
        reduced_transparency: own.reduced_transparency.or(defaults.reduced_transparency),
        forced_colors: own.forced_colors.or(defaults.forced_colors),
        contrast: own.contrast.or(defaults.contrast),
        color_gamut: own.color_gamut.or(defaults.color_gamut),
        media_type: own.media_type.or(defaults.media_type),
    }
}

/// A throttling preset as conditions: the values of Chrome DevTools' presets (latency in ms,
/// throughput in kbit/s, with DevTools' factors applied).
fn network_conditions(throttle: NetworkThrottle) -> NetworkConditions {
    let preset = |latency_ms, download_kbps, upload_kbps| NetworkConditions {
        offline: false,
        latency_ms,
        download_kbps,
        upload_kbps,
    };
    match throttle {
        NetworkThrottle::Custom(conditions) => conditions,
        NetworkThrottle::Preset(NetworkPreset::Offline) => NetworkConditions {
            offline: true,
            ..NetworkConditions::default()
        },
        NetworkThrottle::Preset(NetworkPreset::Slow3g) => preset(2000, 400, 400),
        NetworkThrottle::Preset(NetworkPreset::Fast3g) => preset(563, 1440, 675),
        NetworkThrottle::Preset(NetworkPreset::FourG) => preset(165, 8100, 1350),
    }
}

#[cfg(test)]
#[path = "emulation_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "emulation_page_locale_tests.rs"]
mod page_locale_tests;
