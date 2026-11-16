//! The rules over a view's emulation fields (RB-010-05): live overrides over the breakpoint,
//! which changes the page only sees after a reload, and which headers an untrusted workspace
//! may send. Pure; the host and the commands call them.

use std::collections::BTreeMap;

use rb_config::{Breakpoint, DevicePreset, EmulationFields, MediaFeatures};
use rb_engine::{Emulation, EngineError, resolve_emulation};

/// What the shell sees instead of a header value.
pub const MASKED_VALUE: &str = "********";

/// `top` over `base`: every field `top` sets wins (a `null` geolocation too: it switches the
/// position off); media features field by field, headers by name.
pub fn overlay(base: &EmulationFields, top: &EmulationFields) -> EmulationFields {
    let media = match (&base.media, &top.media) {
        (Some(base), Some(top)) => Some(MediaFeatures {
            reduced_data: top.reduced_data.or(base.reduced_data),
            reduced_transparency: top.reduced_transparency.or(base.reduced_transparency),
            forced_colors: top.forced_colors.or(base.forced_colors),
            contrast: top.contrast.or(base.contrast),
            color_gamut: top.color_gamut.or(base.color_gamut),
            media_type: top.media_type.or(base.media_type),
        }),
        (base, top) => top.clone().or_else(|| base.clone()),
    };
    let mut headers = base.headers.clone();
    headers.extend(top.headers.clone());
    EmulationFields {
        dpr: top.dpr.or(base.dpr),
        touch: top.touch.or(base.touch),
        mobile: top.mobile.or(base.mobile),
        user_agent: top.user_agent.clone().or_else(|| base.user_agent.clone()),
        platform: top.platform.clone().or_else(|| base.platform.clone()),
        locale: top.locale.clone().or_else(|| base.locale.clone()),
        color_scheme: top.color_scheme.or(base.color_scheme),
        reduced_motion: top.reduced_motion.or(base.reduced_motion),
        media,
        network: top.network.or(base.network),
        headers,
        orientation: top.orientation.or(base.orientation),
        safe_areas: top.safe_areas.or(base.safe_areas),
        text_scale: top.text_scale.or(base.text_scale),
        geolocation: top.geolocation.or(base.geolocation),
        timezone: top.timezone.clone().or_else(|| base.timezone.clone()),
        cpu_throttling: top.cpu_throttling.or(base.cpu_throttling),
        offline: top.offline.or(base.offline),
        paper: top.paper.or(base.paper),
    }
}

/// The emulation of a view: its breakpoint (or, after a free size, the size it has now) with the
/// live `overrides` on top, resolved against the project `defaults` and `devices`.
pub fn view_emulation(
    breakpoint: Option<&Breakpoint>,
    current: &Emulation,
    defaults: &EmulationFields,
    overrides: &EmulationFields,
    devices: &[DevicePreset],
) -> Result<Emulation, EngineError> {
    let mut effective = match breakpoint {
        Some(breakpoint) => breakpoint.clone(),
        None => free_size(current),
    };
    effective.emulation = overlay(&effective.emulation, overrides);
    resolve_emulation(&effective, defaults, devices)
}

/// A breakpoint for a view without one: its size and DPR now.
fn free_size(current: &Emulation) -> Breakpoint {
    Breakpoint {
        id: format!("{}x{}", current.width, current.height),
        label: None,
        width: Some(current.width),
        height: Some(current.height),
        device: None,
        engine: rb_config::EngineKind::Chromium,
        session: None,
        sync_navigation: true,
        shortcut: None,
        color: None,
        emulation: EmulationFields {
            dpr: (current.dpr > 0.0).then_some(current.dpr),
            ..EmulationFields::default()
        },
    }
}

/// Whether going from `before` to `after` changes what the server already answered: the user
/// agent, the platform, the language or the headers apply live, but only to the next request.
pub fn reload_required(before: &Emulation, after: &Emulation) -> bool {
    before.user_agent != after.user_agent
        || before.platform != after.platform
        || before.locale != after.locale
        || before.headers != after.headers
}

/// `emulation` for the shell: header values masked (they may be tokens).
pub fn masked(emulation: &Emulation) -> Emulation {
    let mut masked = emulation.clone();
    for value in masked.headers.values_mut() {
        MASKED_VALUE.clone_into(value);
    }
    masked
}

/// The headers an untrusted workspace sends of `resolved`: only those set live
/// (`emulation.set`), never one from `config.json` or `config.local.json` - a clone can commit
/// the local file too, so its headers wait for workspace trust like the network keys.
pub fn allowed_headers(
    resolved: &BTreeMap<String, String>,
    live: &BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    resolved
        .iter()
        .filter(|(name, value)| live.get(*name) == Some(*value))
        .map(|(name, value)| (name.clone(), value.clone()))
        .collect()
}

#[cfg(test)]
#[path = "fields_tests.rs"]
mod tests;
