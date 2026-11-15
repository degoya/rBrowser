//! The sidecar's `webkit.*` methods (`sidecar/src/webkit/index.ts`) as the engine calls them:
//! the method names, the context options of an [`Emulation`], the notifications and how a
//! sidecar error becomes an [`EngineError`].

use rb_config::{ColorScheme, Contrast, EngineKind, ForcedColors, MediaType};
use rb_engine::external::CellScale;
use rb_engine::{Emulation, EngineError, ViewId};
use rb_sidecar::SidecarError;
use serde::Deserialize;
use serde_json::{Map, Value, json};

/// Makes sure WebKit can start: `{ agent?: string }` -> `{ version }` (null before the first
/// browser). Starts nothing: every view gets a browser process of its own (RB-080-40).
pub const LAUNCH: &str = "webkit.launch";
/// Opens a view in a browser of its own: `{ view, url?, options, agentConfig? }` ->
/// `{ pid, version }` (the view's browser process, the owner of its window; null when unknown).
pub const CREATE: &str = "webkit.view.create";
/// `{ view, url }`.
pub const NAVIGATE: &str = "webkit.view.navigate";
/// `{ view }`.
pub const RELOAD: &str = "webkit.view.reload";
/// `{ view }`.
pub const BACK: &str = "webkit.view.back";
/// `{ view }`.
pub const FORWARD: &str = "webkit.view.forward";
/// `{ view, options }` -> `{ recreated }`.
pub const EMULATE: &str = "webkit.view.emulate";
/// `{ view, events, before?, blockSubmitMs? }`.
pub const INPUT: &str = "webkit.view.input";
/// `{ view, x, y, maxX, maxY }`.
pub const SCROLL: &str = "webkit.view.scroll";
/// `{ view, fullPage, scale? }` -> `{ pngBase64 }`; `scale: "css"` is one pixel per CSS px (a
/// view scaled into its cell keeps the breakpoint's size, RB-080-40).
pub const SCREENSHOT: &str = "webkit.view.screenshot";
/// `{ view, command }` -> the agent's `CommandResult`.
pub const AGENT: &str = "webkit.view.agent";
/// `{ view, cookies }` -> `{ added, refused }`: copied cookies into the view's context
/// (RB-040-05; `sidecar/src/webkit/cookies.ts`).
pub const COOKIES: &str = "webkit.view.cookies";
/// `{ view }`: the window to the front.
pub const RAISE: &str = "webkit.view.raise";
/// `{ view }`.
pub const CLOSE: &str = "webkit.view.close";
/// Closes every view and its browser.
pub const SHUTDOWN: &str = "webkit.shutdown";

/// Notification: `{ view?, kind, url?, crashed? }` (`disconnected` without a view ended every
/// view before RB-080-40; a view's browser ending is its own `closed` now).
pub const EVENT: &str = "webkit.event";
/// Notification: `{ view, top, payload }`, a report of a view's sync agent.
pub const AGENT_REPORT: &str = "webkit.agent";

/// JSON-RPC's invalid parameters.
const INVALID_PARAMS: i64 = -32602;
/// The pinned WebKit build is not installed.
pub const NOT_INSTALLED: i64 = -32003;
/// No WebKit view has this id (its window was closed).
const UNKNOWN_VIEW: i64 = -32005;

/// A `webkit.event` notification.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct EventNote {
    /// The view; absent for `disconnected`.
    #[serde(default)]
    pub view: Option<ViewId>,
    /// `navigated`, `load`, `closed` or `disconnected`.
    pub kind: String,
    /// The URL of `navigated` and `load`.
    #[serde(default)]
    pub url: Option<String>,
    /// Whether a `closed` view crashed.
    #[serde(default)]
    pub crashed: bool,
}

/// A `webkit.agent` notification.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct AgentNote {
    /// The view.
    pub view: ViewId,
    /// Whether the agent runs in the top frame.
    pub top: bool,
    /// What the agent wrote; untrusted, parsed with `rb_sync::parse_message`.
    pub payload: String,
}

/// The browser context options of `emulation` (Playwright's names). Fields WebKit has no option
/// for stay out (CPU throttling among them); `EmulationReport::for_engine` marks them. An
/// emulated position is granted to every origin of the view's own context.
pub fn context_options(emulation: &Emulation) -> Value {
    context_options_in(emulation, None)
}

/// [`context_options`] for a view in its grid cell (RB-080-40): the viewport is the
/// breakpoint's width at the cell's height and `deviceScaleFactor` the cell's factor, so WebKit
/// renders the breakpoint's width into the cell's pixels. The page's `devicePixelRatio` is that
/// factor then, not the breakpoint's (Playwright has one knob for both; a screenshot is taken
/// in CSS px instead). Without a cell the breakpoint's own size and ratio.
pub fn context_options_in(emulation: &Emulation, cell: Option<CellScale>) -> Value {
    let mut options = Map::new();
    match cell {
        Some(cell) => {
            options.insert(
                "viewport".into(),
                json!({ "width": emulation.width.max(1), "height": cell.height.max(1) }),
            );
            options.insert("deviceScaleFactor".into(), json!(cell.factor));
        }
        None => {
            options.insert(
                "viewport".into(),
                json!({ "width": emulation.width.max(1), "height": emulation.height.max(1) }),
            );
            // 0 is the display's own ratio: Playwright keeps it when the option is absent.
            if emulation.dpr > 0.0 {
                options.insert("deviceScaleFactor".into(), json!(emulation.dpr));
            }
        }
    }
    options.insert("isMobile".into(), json!(emulation.mobile));
    options.insert("hasTouch".into(), json!(emulation.touch));
    if let Some(agent) = &emulation.user_agent {
        options.insert("userAgent".into(), json!(agent));
    }
    if let Some(locale) = &emulation.locale {
        options.insert("locale".into(), json!(locale));
    }
    if let Some(timezone) = &emulation.timezone {
        options.insert("timezoneId".into(), json!(timezone));
    }
    if let Some(position) = &emulation.geolocation {
        let position = json!({
            "latitude": position.latitude,
            "longitude": position.longitude,
            "accuracy": position.accuracy,
        });
        options.insert("geolocation".into(), position);
        options.insert("permissions".into(), json!(["geolocation"]));
    }
    if let Some(scheme) = emulation.color_scheme {
        let scheme = match scheme {
            ColorScheme::Light => "light",
            ColorScheme::Dark => "dark",
        };
        options.insert("colorScheme".into(), json!(scheme));
    }
    if let Some(reduce) = emulation.reduced_motion {
        let motion = if reduce { "reduce" } else { "no-preference" };
        options.insert("reducedMotion".into(), json!(motion));
    }
    let media = &emulation.media;
    if let Some(forced) = media.forced_colors {
        let forced = match forced {
            ForcedColors::Active => "active",
            ForcedColors::None => "none",
        };
        options.insert("forcedColors".into(), json!(forced));
    }
    // Playwright knows `more` and `no-preference` only.
    match media.contrast {
        Some(Contrast::More) => {
            options.insert("contrast".into(), json!("more"));
        }
        Some(Contrast::NoPreference) => {
            options.insert("contrast".into(), json!("no-preference"));
        }
        Some(Contrast::Less | Contrast::Custom) | None => {}
    }
    if media.media_type == Some(MediaType::Print) {
        options.insert("media".into(), json!("print"));
    }
    if emulation
        .network
        .as_ref()
        .is_some_and(|network| network.offline)
    {
        options.insert("offline".into(), json!(true));
    }
    if !emulation.headers.is_empty() {
        options.insert("extraHTTPHeaders".into(), json!(emulation.headers));
    }
    Value::Object(options)
}

/// What a sidecar error means for a WebKit view. Never carries a secret: the sidecar's messages
/// are masked like everything else it says.
pub fn engine_error(method: &str, error: SidecarError) -> EngineError {
    match error {
        SidecarError::Remote { code, .. } if code == NOT_INSTALLED => not_installed(),
        SidecarError::Remote { code, .. } if code == UNKNOWN_VIEW => EngineError::Closed,
        SidecarError::Remote { code, message, .. } if code == INVALID_PARAMS => {
            EngineError::Invalid(rb_diag::redact(&message))
        }
        SidecarError::Exited | SidecarError::CrashLoop { .. } => EngineError::Closed,
        other => EngineError::Unavailable {
            engine: EngineKind::Webkit,
            reason: rb_diag::redact(&format!("{method}: {other}")),
        },
    }
}

/// WebKit is not installed: the reason names the command that installs it.
pub fn not_installed() -> EngineError {
    EngineError::Unavailable {
        engine: EngineKind::Webkit,
        reason: rb_i18n::t("engines.webkit.notInstalled"),
    }
}

#[cfg(test)]
#[path = "rpc_tests.rs"]
mod tests;
