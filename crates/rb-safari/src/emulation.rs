//! The emulation of a Safari view (RB-095-03): classic WebDriver sizes the window and nothing
//! else, so the breakpoint's viewport becomes the window's size - the viewport plus Safari's
//! chrome, which a probe in the page measures ([`VIEWPORT_PROBE`]) - read back and corrected
//! once. Everything else is marked by `rb_engine::EmulationReport`'s Safari column; a viewport
//! Safari would not take (a window narrower than Safari allows) is "approximated" with the size
//! it shows. On iOS (RB-095-04) nothing is sized: the device's screen is the viewport, and the
//! report names the device and the size it shows ([`ios_report`]).

use rb_engine::external::WindowRect;
use rb_engine::{Emulation, EmulationReport, EngineKind, FieldSupport};
use serde_json::{Value, json};

/// Measures the window's chrome and the viewport in CSS px.
pub const VIEWPORT_PROBE: &str = "return [window.outerWidth - window.innerWidth, window.outerHeight - window.innerHeight, window.innerWidth, window.innerHeight];";

/// What [`VIEWPORT_PROBE`] measured.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Probe {
    /// The window's size beyond the viewport (toolbar, borders), width and height.
    pub chrome: (i64, i64),
    /// The viewport, width and height.
    pub inner: (i64, i64),
}

/// The probe's answer; `None` for anything but four numbers with a viewport of some size.
pub fn parse_probe(value: &Value) -> Option<Probe> {
    let numbers: Vec<i64> = value
        .as_array()?
        .iter()
        .map(|number| number.as_f64().map(|n| n.round() as i64))
        .collect::<Option<_>>()?;
    let [chrome_w, chrome_h, inner_w, inner_h] = numbers.as_slice() else {
        return None;
    };
    (*inner_w >= 0 && *inner_h >= 0).then_some(Probe {
        chrome: (*chrome_w, *chrome_h),
        inner: (*inner_w, *inner_h),
    })
}

/// The window size that shows `viewport` with the chrome `probe` measured; at least 1x1.
pub fn window_size(viewport: (u32, u32), probe: &Probe) -> (u32, u32) {
    let side = |wanted: u32, chrome: i64| {
        let size = i64::from(wanted).saturating_add(chrome).max(1);
        u32::try_from(size).unwrap_or(u32::MAX)
    };
    (
        side(viewport.0, probe.chrome.0),
        side(viewport.1, probe.chrome.1),
    )
}

/// The body of `POST /window/rect` that sizes the window.
pub fn size_body(size: (u32, u32)) -> Value {
    json!({ "width": size.0, "height": size.1 })
}

/// The body of `POST /window/rect` that moves the window to `rect`'s corner; its size stays
/// the emulation's.
pub fn position_body(rect: WindowRect) -> Value {
    json!({ "x": rect.x, "y": rect.y })
}

/// The report of `emulation` in Safari; `shown` is the viewport read back, when it was.
pub fn report(emulation: &Emulation, shown: Option<(i64, i64)>) -> EmulationReport {
    let mut report = EmulationReport::for_engine(EngineKind::Safari, emulation);
    let wanted = (i64::from(emulation.width), i64::from(emulation.height));
    if let Some(shown) = shown.filter(|shown| *shown != wanted)
        && let Some(viewport) = report
            .fields
            .iter_mut()
            .find(|field| field.field == "viewport")
    {
        viewport.support =
            FieldSupport::Approximated(format!("Safari shows {}x{}", shown.0, shown.1));
    }
    report
}

/// The report of `emulation` on iOS (RB-095-04): the published table; the viewport read back
/// (`shown`) names `device` and its size when it is not the breakpoint's.
pub fn ios_report(
    emulation: &Emulation,
    device: &str,
    shown: Option<(i64, i64)>,
) -> EmulationReport {
    let mut report = EmulationReport::for_engine(EngineKind::Ios, emulation);
    let wanted = (i64::from(emulation.width), i64::from(emulation.height));
    if let Some(shown) = shown.filter(|shown| *shown != wanted)
        && let Some(viewport) = report
            .fields
            .iter_mut()
            .find(|field| field.field == "viewport")
    {
        viewport.support =
            FieldSupport::Unsupported(format!("{device} shows {}x{}", shown.0, shown.1));
    }
    report
}

#[cfg(test)]
#[path = "emulation_tests.rs"]
mod tests;
