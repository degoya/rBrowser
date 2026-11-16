//! Bringing a view's choice to the screen: Chromium's own emulation over CDP, or the page
//! script's drawing.

use rb_plugin_api::{PluginHost, ViewInfo};
use serde_json::{Value, json};

use crate::catalog::{Simulation, Spec, spec};
use crate::state::{Choice, Settings};

/// The CDP command of Chromium's own emulation.
pub const CDP_METHOD: &str = "Emulation.setEmulatedVisionDeficiency";

/// How a view shows its simulation.
#[derive(Clone, Debug, PartialEq)]
pub enum Mode {
    /// Nothing is simulated.
    Off,
    /// Chromium renders it: the `type` of [`CDP_METHOD`].
    Native(&'static str),
    /// The page script draws it.
    Script(Spec),
}

impl Mode {
    fn name(&self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Native(_) => "native",
            Self::Script(_) => "script",
        }
    }
}

/// The strength `choice` is drawn with.
#[must_use]
pub fn severity(choice: &Choice) -> Option<f64> {
    choice
        .kind
        .default_severity()
        .and(choice.severity.or(choice.kind.default_severity()))
}

fn drawing(choice: &Choice, settings: &Settings) -> Option<Spec> {
    spec(
        choice.kind,
        severity(choice).unwrap_or(1.0),
        settings.blur_px,
    )
}

/// How `choice` would show in a view of `engine`. Chromium renders a type it knows itself when
/// `preferNative` is on and no strength was given (its own strength is fixed).
#[must_use]
pub fn mode(choice: &Choice, engine: &str, settings: &Settings) -> Mode {
    let Some(drawn) = drawing(choice, settings) else {
        return Mode::Off;
    };
    match choice.kind.native() {
        Some(native)
            if engine == "chromium" && settings.prefer_native && choice.severity.is_none() =>
        {
            Mode::Native(native)
        }
        _ => Mode::Script(drawn),
    }
}

fn emulate(host: &mut dyn PluginHost, view: &ViewInfo, native: &str) -> bool {
    let params = json!({ "type": native }).to_string();
    match host.cdp_send(view.id, CDP_METHOD, &params) {
        Ok(_) => true,
        Err(failure) => {
            let code = failure.code.unwrap_or_default();
            host.log(
                "warn",
                &format!("view {}: {CDP_METHOD} ({native}) failed: {code}", view.id),
            );
            false
        }
    }
}

/// Brings `after` to `view`, which showed `before`: sets Chromium's emulation, or clears the one
/// `before` set. Answers the mode in force - the page script's drawing when the emulation was
/// refused.
pub fn apply(
    host: &mut dyn PluginHost,
    view: &ViewInfo,
    before: Option<&Choice>,
    after: &Choice,
    settings: &Settings,
) -> Mode {
    let mut mode = mode(after, &view.engine, settings);
    if let Mode::Native(native) = mode {
        if !emulate(host, view, native) {
            mode = drawing(after, settings).map_or(Mode::Off, Mode::Script);
        }
    } else if before
        .is_some_and(|before| matches!(self::mode(before, &view.engine, settings), Mode::Native(_)))
    {
        emulate(host, view, "none");
    }
    let how = match mode {
        Mode::Off => "off".to_owned(),
        Mode::Native(native) => format!("via CDP {CDP_METHOD} ({native})"),
        Mode::Script(_) => "via the page script".to_owned(),
    };
    host.log(
        "info",
        &format!("view {}: {} {how}", view.id, after.kind.id()),
    );
    mode
}

/// The page script's message for `mode`: the drawing, or none.
#[must_use]
pub fn message(mode: &Mode) -> Value {
    let spec = match mode {
        Mode::Script(spec) => json!(spec),
        Mode::Off | Mode::Native(_) => Value::Null,
    };
    json!({ "type": "apply", "spec": spec })
}

/// One view in an answer.
#[must_use]
pub fn report(view: &ViewInfo, choice: &Choice, mode: &Mode) -> Value {
    json!({
        "view": view.id,
        "label": view.label,
        "engine": view.engine,
        "type": choice.kind,
        "severity": severity(choice),
        "mode": mode.name(),
    })
}

/// The catalogue the picker offers: id, group, default strength, whether Chromium can render it.
#[must_use]
pub fn catalog() -> Value {
    crate::catalog::ALL
        .iter()
        .map(|kind: &Simulation| {
            json!({
                "id": kind.id(),
                "group": kind.group(),
                "severity": kind.default_severity(),
                "native": kind.native().is_some(),
            })
        })
        .collect()
}
