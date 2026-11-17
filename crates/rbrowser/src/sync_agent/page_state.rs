//! The page API's state of a view (RB-020-08), pure: the switches in effect (`pageApi.*`,
//! `bus.*`, the toggles' session overrides), what a page learns about its view
//! ([`page_state`]) and the main-world script with that state ([`main_world_source`]).

use rb_config::{ColorScheme, EngineKind, Orientation, ProjectConfig};
use rb_engine::{Emulation, ViewInfo};
use rb_sync::{BusSettings, PAGE_BOOT_PARAM, PageEmulation, PageInfo, PageState};

/// The switches in effect.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PageSwitches {
    /// `window.__rbrowser` in the views' pages.
    pub api: bool,
    /// ` rBrowser/<version>` at the end of the user agent.
    pub ua_suffix: bool,
    /// The bus's own switch and limits (held to their ranges).
    pub bus: BusSettings,
}

impl PageSwitches {
    /// The bus as it works: off without the page API.
    pub fn bus_in_effect(&self) -> BusSettings {
        BusSettings {
            enabled: self.bus.enabled && self.api,
            ..self.bus
        }
    }
}

/// A switch a toggle command flips.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Switch {
    /// `pageApi.enabled`.
    Api,
    /// `pageApi.uaSuffix`.
    UaSuffix,
    /// `bus.enabled`.
    Bus,
}

/// The toggles' overrides for the session. An override ends when the config's own value of its
/// key changes (the settings window, an edit of `config.json`), so the newer decision wins.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Overrides {
    api: Option<bool>,
    ua_suffix: Option<bool>,
    bus: Option<bool>,
    /// The config's values when the overrides were last applied: api, suffix, bus.
    seen: Option<[bool; 3]>,
}

impl Overrides {
    /// The switches of `config` under the overrides.
    pub fn apply(&mut self, config: &ProjectConfig) -> PageSwitches {
        let own = [
            config.page_api.enabled,
            config.page_api.ua_suffix,
            config.bus.enabled,
        ];
        if let Some(seen) = self.seen {
            for (slot, (before, now)) in [&mut self.api, &mut self.ua_suffix, &mut self.bus]
                .into_iter()
                .zip(seen.into_iter().zip(own))
            {
                if before != now {
                    *slot = None;
                }
            }
        }
        self.seen = Some(own);
        let [api, ua_suffix, bus] = own;
        PageSwitches {
            api: self.api.unwrap_or(api),
            ua_suffix: self.ua_suffix.unwrap_or(ua_suffix),
            bus: BusSettings {
                enabled: self.bus.unwrap_or(bus),
                max_payload_bytes: config.bus.max_payload_bytes,
                max_per_second: config.bus.max_per_second,
            }
            .clamped(),
        }
    }

    /// Flips `switch` for the session; returns its new value.
    pub fn toggle(&mut self, switch: Switch, config: &ProjectConfig) -> bool {
        let current = self.apply(config);
        let (slot, value) = match switch {
            Switch::Api => (&mut self.api, current.api),
            Switch::UaSuffix => (&mut self.ua_suffix, current.ua_suffix),
            Switch::Bus => (&mut self.bus, current.bus.enabled),
        };
        *slot = Some(!value);
        !value
    }
}

/// What the page of `view` (at grid position `index`, with `emulation`) learns. The user agent,
/// headers and network conditions stay out: they are no view info, and headers can be secret.
pub fn page_state(
    view: &ViewInfo,
    index: usize,
    emulation: &Emulation,
    switches: &PageSwitches,
) -> PageState {
    let engine = match view.engine {
        EngineKind::Chromium => "chromium",
        EngineKind::Firefox => "firefox",
        EngineKind::Webkit => "webkit",
    };
    PageState {
        info: PageInfo {
            id: view.id.0,
            label: view.label.clone(),
            width: view.width,
            height: view.height,
            index: u32::try_from(index).unwrap_or(u32::MAX),
            engine: engine.to_owned(),
            emulation: PageEmulation {
                dpr: emulation.dpr,
                mobile: emulation.mobile,
                touch: emulation.touch,
                orientation: emulation.orientation.map(|orientation| {
                    match orientation {
                        Orientation::Portrait => "portrait",
                        Orientation::Landscape => "landscape",
                    }
                    .to_owned()
                }),
                color_scheme: emulation.color_scheme.map(|scheme| {
                    match scheme {
                        ColorScheme::Light => "light",
                        ColorScheme::Dark => "dark",
                    }
                    .to_owned()
                }),
                reduced_motion: emulation.reduced_motion,
                locale: emulation.locale.clone(),
            },
        },
        bus: switches.bus_in_effect(),
    }
}

/// The main-world script: `bundle` wrapped in a function whose parameter is the state, so the
/// state never becomes a global of the page. The state is JSON, a JavaScript expression nothing
/// in it can break out of. `None` for an empty bundle (`agent/` not built).
pub fn main_world_source(bundle: &str, state: &PageState) -> Option<String> {
    if bundle.trim().is_empty() {
        return None;
    }
    let state = serde_json::to_string(state).ok()?;
    Some(format!(
        "(function({PAGE_BOOT_PARAM}){{\n{bundle}\n}})({state});"
    ))
}

#[cfg(test)]
#[path = "page_state_tests.rs"]
mod tests;
