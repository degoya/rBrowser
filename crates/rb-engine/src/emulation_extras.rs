//! The emulation extras of a view (RB-080-10): geolocation, time zone and CPU throttling
//! resolved from the breakpoint over the project defaults, offline folded into the network
//! conditions, and which of them - with the print media type - an engine can emulate. The
//! list is the code side of ADR 0001, Decision 8 ("not emulable in Firefox"); the view marks
//! what [`not_emulable`] names (RB-040-01, RB-040-02, RB-040-04).

use rb_config::{
    Breakpoint, EmulationFields, EngineKind, Geolocation, MediaType, NetworkConditions,
    cpu_throttling_problem, is_iana_timezone,
};

use crate::{Emulation, EngineError};

/// The field names the marking uses (as in the config).
pub const GEOLOCATION: &str = "geolocation";
/// The time zone.
pub const TIMEZONE: &str = "timezone";
/// The CPU throttling.
pub const CPU_THROTTLING: &str = "cpuThrottling";
/// Offline.
pub const OFFLINE: &str = "offline";
/// The print media type (`media.mediaType: "print"`, the print view).
pub const PRINT: &str = "print";

/// The extras in marking order.
const FIELDS: [&str; 5] = [GEOLOCATION, TIMEZONE, CPU_THROTTLING, OFFLINE, PRINT];

/// Whether an engine can emulate a field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Support {
    /// Live, at any time.
    Yes,
    /// Only when the installed browser knows this command (a feature probe at start, Firefox
    /// over WebDriver BiDi).
    Probe(&'static str),
    /// Only when the browser context is made (Playwright's `timezoneId`): a change makes the
    /// view's context again.
    NewContext,
    /// Not at all: the view is marked "not emulable".
    No,
}

/// What `engine` can do with `field` (one of the constants above; anything else is `No`).
pub fn support(engine: EngineKind, field: &str) -> Support {
    match (engine, field) {
        (EngineKind::Chromium, _) if FIELDS.contains(&field) => Support::Yes,
        (EngineKind::Firefox, GEOLOCATION) => Support::Probe("emulation.setGeolocationOverride"),
        (EngineKind::Firefox, TIMEZONE) => Support::Probe("emulation.setTimezoneOverride"),
        (EngineKind::Firefox, OFFLINE) => Support::Probe("emulation.setNetworkConditions"),
        (EngineKind::Webkit, TIMEZONE) => Support::NewContext,
        (EngineKind::Webkit, GEOLOCATION | OFFLINE | PRINT) => Support::Yes,
        _ => Support::No,
    }
}

/// The extras `emulation` sets that `engine` cannot emulate, in marking order; `probed` are the
/// commands the engine's feature probe found.
pub fn not_emulable(
    engine: EngineKind,
    emulation: &Emulation,
    probed: &[&str],
) -> Vec<&'static str> {
    FIELDS
        .into_iter()
        .filter(|field| is_set(emulation, field))
        .filter(|field| match support(engine, field) {
            Support::Yes | Support::NewContext => false,
            Support::Probe(command) => !probed.contains(&command),
            Support::No => true,
        })
        .collect()
}

fn is_set(emulation: &Emulation, field: &str) -> bool {
    match field {
        GEOLOCATION => emulation.geolocation.is_some(),
        TIMEZONE => emulation.timezone.is_some(),
        CPU_THROTTLING => emulation.cpu_throttling.is_some(),
        OFFLINE => emulation.network.is_some_and(|network| network.offline),
        PRINT => emulation.media.media_type == Some(MediaType::Print),
        _ => false,
    }
}

/// The resolved geolocation, time zone and CPU throttling of a view.
pub(crate) struct Extras {
    pub geolocation: Option<Geolocation>,
    pub timezone: Option<String>,
    pub cpu_throttling: Option<f64>,
}

/// The extras of `breakpoint` over the project `defaults`: its own value wins, `null`, an empty
/// time zone and a factor of 1 switch a default off. A value a browser would refuse is
/// [`EngineError::Invalid`].
pub(crate) fn resolve_extras(
    breakpoint: &Breakpoint,
    defaults: &EmulationFields,
) -> Result<Extras, EngineError> {
    let own = &breakpoint.emulation;
    let invalid =
        |why: String| EngineError::Invalid(format!("breakpoint {}: {why}", breakpoint.id));
    let geolocation = own.geolocation.or(defaults.geolocation).flatten();
    if let Some(why) = geolocation.and_then(|geolocation| geolocation.problem()) {
        return Err(invalid(format!("geolocation: {why}")));
    }
    let timezone = own
        .timezone
        .clone()
        .or_else(|| defaults.timezone.clone())
        .filter(|timezone| !timezone.is_empty());
    if let Some(timezone) = timezone.as_deref()
        && !is_iana_timezone(timezone)
    {
        return Err(invalid(format!(
            "timezone {timezone:?} is not an IANA time zone name"
        )));
    }
    let cpu_throttling = own.cpu_throttling.or(defaults.cpu_throttling);
    if let Some(why) = cpu_throttling.and_then(cpu_throttling_problem) {
        return Err(invalid(format!("cpuThrottling {why}")));
    }
    Ok(Extras {
        geolocation,
        timezone,
        cpu_throttling: cpu_throttling.filter(|rate| *rate > 1.0),
    })
}

/// `offline` over the network conditions: `true` takes the network away (a throttle keeps its
/// other values), `false` gives it back; unset leaves them.
pub(crate) fn with_offline(
    network: Option<NetworkConditions>,
    offline: Option<bool>,
) -> Option<NetworkConditions> {
    match (network, offline) {
        (network, None) => network,
        (Some(conditions), Some(offline)) => Some(NetworkConditions {
            offline,
            ..conditions
        }),
        (None, Some(true)) => Some(NetworkConditions {
            offline: true,
            ..NetworkConditions::default()
        }),
        (None, Some(false)) => None,
    }
}

#[cfg(test)]
#[path = "emulation_extras_tests.rs"]
mod tests;
