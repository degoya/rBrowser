//! The emulation of one Firefox view over BiDi: the viewport and DPR
//! (`browsingContext.setViewport`), then one override per field Firefox has a command for, each
//! for the view's context alone - user agent, locale, color scheme, screen size and orientation,
//! offline, extra headers. A field not asked for is reset, so an emulation applied again is the
//! whole state. What Firefox cannot emulate (ADR 0001, Decision 8) is never sent: the report says
//! so, from the table every place shares (`rb_engine::EmulationReport`).

use rb_config::{ColorScheme, Orientation};
use rb_engine::external::CellScale;
use rb_engine::{Emulation, EmulationReport, EngineKind, FieldSupport};
use serde_json::{Value, json};

use crate::browsing_context::set_viewport;
use crate::protocol::Command;

/// One override and the report field it serves (`None`: part of the viewport, not reported).
#[derive(Clone, Debug, PartialEq)]
pub struct Override {
    /// The field of [`EmulationReport`] (`userAgent`).
    pub field: Option<&'static str>,
    /// The command.
    pub command: Command,
}

/// The viewport and device pixel ratio of `emulation` for `context`.
pub fn viewport(context: &str, emulation: &Emulation) -> Command {
    viewport_in(context, emulation, None)
}

/// The viewport of `emulation` for `context` in its grid cell (RB-080-40): the breakpoint's
/// width, the cell's height at the profile's render scale, and the breakpoint's DPR - which is
/// what the page reports and what a screenshot is taken at, so one stays breakpoint-sized; a
/// breakpoint without a DPR (0, the display's) gets 1 in a cell, else the screenshot would
/// shrink with the cell. Without a cell the breakpoint's own size.
pub fn viewport_in(context: &str, emulation: &Emulation, cell: Option<CellScale>) -> Command {
    let (width, height, dpr) = wanted_viewport(emulation, cell);
    set_viewport(context, width, height, dpr)
}

/// The viewport [`viewport_in`] asks for: width, height and DPR (0 leaves the ratio to the
/// display).
pub fn wanted_viewport(emulation: &Emulation, cell: Option<CellScale>) -> (u32, u32, f64) {
    match cell {
        Some(cell) => (
            emulation.width,
            cell.height,
            if emulation.dpr > 0.0 {
                emulation.dpr
            } else {
                1.0
            },
        ),
        None => (emulation.width, emulation.height, emulation.dpr),
    }
}

/// Every override for `context`, set or reset.
pub fn overrides(context: &str, emulation: &Emulation) -> Vec<Override> {
    let item = |field, method, key: &str, value: Value| {
        let mut params = serde_json::Map::new();
        params.insert(key.to_owned(), value);
        params.insert("contexts".to_owned(), json!([context]));
        Override {
            field,
            command: Command::call(method, Value::Object(params)),
        }
    };
    let color_scheme = emulation.color_scheme.map(|scheme| {
        let name = match scheme {
            ColorScheme::Light => "light",
            ColorScheme::Dark => "dark",
        };
        json!({ "prefers-color-scheme": name })
    });
    let orientation = emulation.orientation.map(|orientation| {
        let kind = match orientation {
            Orientation::Portrait => "portrait-primary",
            Orientation::Landscape => "landscape-primary",
        };
        json!({ "natural": "portrait", "type": kind })
    });
    let offline = emulation
        .network
        .as_ref()
        .filter(|network| network.offline)
        .map(|_| json!({ "type": "offline" }));
    let headers: Vec<Value> = emulation
        .headers
        .iter()
        .map(|(name, value)| json!({ "name": name, "value": { "type": "string", "value": value } }))
        .collect();
    vec![
        item(
            None,
            "emulation.setScreenSettingsOverride",
            "screenArea",
            json!({ "width": emulation.width, "height": emulation.height }),
        ),
        item(
            Some("userAgent"),
            "emulation.setUserAgentOverride",
            "userAgent",
            json!(emulation.user_agent),
        ),
        item(
            Some("locale"),
            "emulation.setLocaleOverride",
            "locale",
            json!(emulation.locale),
        ),
        item(
            Some("colorScheme"),
            "emulation.setMediaFeaturesOverride",
            "features",
            json!(color_scheme),
        ),
        item(
            Some("orientation"),
            "emulation.setScreenOrientationOverride",
            "screenOrientation",
            json!(orientation),
        ),
        item(
            Some("network.offline"),
            "emulation.setNetworkConditions",
            "networkConditions",
            json!(offline),
        ),
        item(
            Some("headers"),
            "network.setExtraHeaders",
            "headers",
            json!(headers),
        ),
    ]
}

/// The report of `emulation`: the shared Firefox table, with each field in `missing` (this
/// Firefox has no command for it) unsupported.
pub fn report(emulation: &Emulation, missing: &[&str]) -> EmulationReport {
    let mut report = EmulationReport::for_engine(EngineKind::Firefox, emulation);
    for field in &mut report.fields {
        if missing.contains(&field.field.as_str()) {
            field.support =
                FieldSupport::Unsupported("this Firefox version has no such override".to_owned());
        }
    }
    report
}

#[cfg(test)]
#[path = "emulation_tests.rs"]
mod tests;
