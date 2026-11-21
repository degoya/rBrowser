//! Which CDP commands a plugin may send, and with which parameters (review findings H1-H3 of
//! Charge 6).
//!
//! The `cdp` capability shares the view's session with the host, the sync agent and the page
//! scripts, and CDP reaches far beyond the page: every site's cookies, certificate errors, local
//! files, other targets, input. So the host lets through only the methods of
//! [`PLUGIN_CDP_METHODS`] - what the first-party plugins send - and holds the parameters of the
//! dangerous ones to the plugin's own reach:
//!
//! - **Its own world** (H1): `Page.createIsolatedWorld` only with the world `rbplugin-<id>` and
//!   no universal access; `Runtime.evaluate` in the page's main world (no `contextId`) or in a
//!   context the host checks is the plugin's own world ([`Pending::OwnContext`]), never by
//!   `uniqueContextId`. Nothing that hands out or takes a remote object of another world
//!   (`Runtime.callFunctionOn`, `DOM.resolveNode`) is listed.
//! - **Web pages only** (H2): `Page.navigate` (not listed today) to `http(s)` or `about:blank`;
//!   nothing that reads local files or downloads (`DOM.setFileInputFiles`, download behaviour)
//!   is listed.
//! - **The view's own site** (H3): cookies only for the URL the view shows
//!   ([`Pending::ViewOrigin`], [`Pending::ViewHost`]); no `getAllCookies`, no `*.disable`, no
//!   `Security`, `Fetch`, `Target`, `Browser`, `Input` or `Storage`.
//!
//! A refusal is `plugins.error.cdp_domain_denied` with the `domain`, the `method` and a `reason`
//! (`method`, `params`, `world`, `context`, `url`, `urls`, `cookie`, `request`, `origin`,
//! `clip`, `width`).

use rb_plugin_types::{Failure, FailureKind, codes, rules};
use serde_json::{Map, Value};

/// The CDP commands a plugin may send, by domain; a manifest's `cdp.domains` may name only
/// these domains.
///
/// Each entry is a method a first-party plugin sends today. **To add one**, name the plugin that
/// needs it, add a parameter rule in [`guard`] when the method can reach beyond the view's page,
/// and extend `cdp_policy_tests.rs`. Never add a `*.disable` (the host's own instrumentation),
/// `Network.getAllCookies`, `Security.*`, `Fetch.*` (interception is the core's `network-rules`),
/// `Target.*`, `Browser.*`, `Input.*`, `Storage.*`, `DOM.setFileInputFiles`, a download
/// behaviour or a method that takes another world's context or object.
pub const PLUGIN_CDP_METHODS: &[(&str, &[&str])] = &[
    // outline (whole tree), peek (one node; the tree's root names the document).
    ("Accessibility", &["getFullAXTree", "getPartialAXTree"]),
    // overlays (force-state), peek (rendered fonts), coverage (rule usage of the view's page
    // and the text of the sheets it names).
    (
        "CSS",
        &[
            "enable",
            "forcePseudoState",
            "getPlatformFontsForNode",
            "getStyleSheetText",
            "startRuleUsageTracking",
            "stopRuleUsageTracking",
            "takeCoverageDelta",
        ],
    ),
    // outline, overlays, peek: find one element and mark it.
    (
        "DOM",
        &[
            "enable",
            "getDocument",
            "getNodeForLocation",
            "pushNodesByBackendIdsToFrontend",
            "querySelector",
            "scrollIntoViewIfNeeded",
        ],
    ),
    // overlays (js-off), simulators (the colour-vision and blur types Chromium renders itself),
    // width-sweep (RB-093-01: a view's width for the sweep, [`width_override`]). The host never
    // sends the two metrics methods as given: the app turns them into a width lease (the view's
    // own metrics at another width, out of sync) and its end.
    (
        "Emulation",
        &[
            "clearDeviceMetricsOverride",
            "setDeviceMetricsOverride",
            "setEmulatedVisionDeficiency",
            "setScriptExecutionDisabled",
        ],
    ),
    // storage-panel (the view's own cookies), network-panel (a response body of the view).
    (
        "Network",
        &[
            "deleteCookies",
            "getCookies",
            "getResponseBody",
            "setCookie",
        ],
    ),
    // outline (the mark).
    ("Overlay", &["enable", "hideHighlight", "highlightNode"]),
    // snippets (its own world in the main frame); color-picker (the pixels around the cursor:
    // the view's own rendering, nothing beyond the page); coverage (a linked style sheet's text,
    // by its web URL); pwa-check (Chromium's installability errors of the view's page: read
    // only, no parameters).
    (
        "Page",
        &[
            "captureScreenshot",
            "createIsolatedWorld",
            "getFrameTree",
            "getInstallabilityErrors",
            "getResourceContent",
        ],
    ),
    // coverage: V8's precise coverage of the view's page. `enable` stays on (no `*.disable`);
    // with precise coverage stopped V8 is back to its best-effort coverage. No sampling profiler,
    // no `Debugger` (it could pause the page).
    (
        "Profiler",
        &[
            "enable",
            "startPreciseCoverage",
            "stopPreciseCoverage",
            "takePreciseCoverage",
        ],
    ),
    // snippets (main world, or its own world).
    ("Runtime", &["evaluate"]),
];

/// The longest `requestId` `Network.getResponseBody` takes, and `frameId`
/// `Page.getResourceContent` takes.
const MAX_REQUEST_ID: usize = 128;
/// The most URLs one `Network.getCookies` names.
const MAX_COOKIE_URLS: usize = 16;
/// The largest `Page.captureScreenshot` clip, `width * height * scale^2` in CSS px: 1 Mpx (the
/// view's device pixel ratio multiplies it once more, at most 16 Mpx at DPR 4) instead of the
/// 16384 x 16384 x 4^2 a whole-page shot could ask for; color-picker's loupe is a few hundred.
const MAX_SCREENSHOT_AREA: f64 = 1_048_576.0;
/// The largest `scale` of a `Page.captureScreenshot` clip.
const MAX_SCREENSHOT_SCALE: f64 = 4.0;
/// The narrowest width `Emulation.setDeviceMetricsOverride` may ask of a view, CSS px.
pub(crate) const MIN_PLUGIN_WIDTH: u64 = 120;
/// The widest, CSS px: a 4K screen's width.
pub(crate) const MAX_PLUGIN_WIDTH: u64 = 3840;

/// What the host still checks before a command passes: what needs the view or the session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Pending {
    /// Nothing; the command may go.
    None,
    /// `Runtime.evaluate` names this execution context: it must be the plugin's own world.
    OwnContext(i64),
    /// Each URL must have the origin of the URL the view shows.
    ViewOrigin(Vec<String>),
    /// Each URL must have the view's host, each cookie domain must domain-match it.
    ViewHost {
        urls: Vec<String>,
        domains: Vec<String>,
    },
    /// `Page.captureScreenshot` of the window's image without a clip: only with the manifest's
    /// `cdp.window_image` (review S2 of RB-080-64).
    WindowImage,
}

/// Whether `method` is on [`PLUGIN_CDP_METHODS`].
#[must_use]
pub(crate) fn allowed(method: &str) -> bool {
    let Some((domain, command)) = method.split_once('.') else {
        return false;
    };
    PLUGIN_CDP_METHODS
        .iter()
        .any(|(listed, methods)| *listed == domain && methods.contains(&command))
}

/// Whether a manifest may list `domain` in `cdp.domains`.
#[must_use]
pub(crate) fn domain_allowed(domain: &str) -> bool {
    PLUGIN_CDP_METHODS
        .iter()
        .any(|(listed, _)| *listed == domain)
}

/// The refusal of `method` for `reason`.
pub(crate) fn denied(method: &str, reason: &str) -> Failure {
    let shown: String = method.chars().take(64).collect();
    let domain = rules::cdp_domain(method)
        .unwrap_or(shown.as_str())
        .to_owned();
    Failure::coded(FailureKind::AccessDenied, codes::CDP_DOMAIN_DENIED)
        .with_param("domain", domain)
        .with_param("method", shown.as_str())
        .with_param("reason", reason)
}

/// Whether plugin `plugin_id` may send `method` with `params`, and what is still to check.
///
/// # Errors
///
/// The method is not on the allowlist, `params` is no object, or a parameter reaches beyond the
/// plugin's own world or the web.
pub(crate) fn check(plugin_id: &str, method: &str, params: &Value) -> Result<Pending, Failure> {
    if !allowed(method) {
        return Err(denied(method, "method"));
    }
    guard(plugin_id, method, params)
}

/// The parameter rules of the methods that can reach beyond the page, whether or not
/// [`PLUGIN_CDP_METHODS`] lists them today.
///
/// # Errors
///
/// As [`check`], without the allowlist.
pub(crate) fn guard(plugin_id: &str, method: &str, params: &Value) -> Result<Pending, Failure> {
    let Some(params) = params.as_object() else {
        return Err(denied(method, "params"));
    };
    match method {
        "Page.createIsolatedWorld" | "Page.addScriptToEvaluateOnNewDocument" => {
            own_world(plugin_id, method, params)
        }
        "Runtime.evaluate" => evaluate_context(method, params),
        "Page.navigate" => navigation(method, params),
        "Network.getCookies" => cookie_urls(method, params),
        "Network.setCookie" | "Network.deleteCookies" => cookie_target(method, params),
        "Page.captureScreenshot" => screenshot(method, params),
        "Page.getResourceContent" => resource(method, params),
        "Emulation.setDeviceMetricsOverride" => width_override(method, params),
        "Emulation.clearDeviceMetricsOverride" if !params.is_empty() => {
            Err(denied(method, "params"))
        }
        "Network.getResponseBody" => {
            let id = params.get("requestId").and_then(Value::as_str);
            match id {
                Some(id) if !id.is_empty() && id.len() <= MAX_REQUEST_ID => Ok(Pending::None),
                _ => Err(denied(method, "request")),
            }
        }
        _ => Ok(Pending::None),
    }
}

fn own_world(
    plugin_id: &str,
    method: &str,
    params: &Map<String, Value>,
) -> Result<Pending, Failure> {
    let own = crate::bridge::world_name(plugin_id);
    let named = params.get("worldName").and_then(Value::as_str);
    // Universal access would let the world read every origin (the CDP name is misspelt).
    let universal = params
        .get("grantUniveralAccess")
        .is_some_and(|grant| grant != &Value::Bool(false));
    if named == Some(own.as_str()) && !universal {
        Ok(Pending::None)
    } else {
        Err(denied(method, "world"))
    }
}

/// `Page.captureScreenshot` only of a small clip of what the view renders (Charge 8 review M1):
/// a `clip` with positive `width` and `height`, `scale` (default 1) above 0 and at most
/// [`MAX_SCREENSHOT_SCALE`], `width * height * scale^2` at most [`MAX_SCREENSHOT_AREA`]; no
/// `captureBeyondViewport` (a whole-page shot). Or, without a clip, the window's image
/// (`fromSurface: false`, RB-080-64): what the view shows on the screen, never rendered anew, so
/// never larger than the view and no frame the view draws for it - [`Pending::WindowImage`],
/// which only a manifest granting `cdp.window_image` passes.
fn screenshot(method: &str, params: &Map<String, Value>) -> Result<Pending, Failure> {
    let refused = || denied(method, "clip");
    let flag = |key: &str, wanted: bool| match params.get(key) {
        None => true,
        Some(value) => value == &Value::Bool(wanted),
    };
    if !flag("captureBeyondViewport", false) {
        return Err(refused());
    }
    if params.get("fromSurface") == Some(&Value::Bool(false)) {
        return if params.contains_key("clip") {
            Err(refused())
        } else {
            Ok(Pending::WindowImage)
        };
    }
    if !flag("fromSurface", true) {
        return Err(refused());
    }
    let clip = params
        .get("clip")
        .and_then(Value::as_object)
        .ok_or_else(refused)?;
    let number = |key: &str| {
        clip.get(key)
            .and_then(Value::as_f64)
            .filter(|n| n.is_finite())
    };
    let (Some(_), Some(_), Some(width), Some(height)) =
        (number("x"), number("y"), number("width"), number("height"))
    else {
        return Err(refused());
    };
    let scale = match clip.get("scale") {
        None => Some(1.0),
        Some(_) => number("scale"),
    }
    .ok_or_else(refused)?;
    let in_range = width > 0.0
        && height > 0.0
        && scale > 0.0
        && scale <= MAX_SCREENSHOT_SCALE
        && width * height * scale * scale <= MAX_SCREENSHOT_AREA;
    if in_range {
        Ok(Pending::None)
    } else {
        Err(refused())
    }
}

/// `Emulation.setDeviceMetricsOverride` only as `{ "width": n }`, a whole number of CSS px from
/// [`MIN_PLUGIN_WIDTH`] to [`MAX_PLUGIN_WIDTH`] (RB-093-01): the height, the device pixel ratio,
/// the scale, the screen and the position stay the host's, which owns the view's metrics (ADR
/// 0001) and sends its own with only the width changed.
fn width_override(method: &str, params: &Map<String, Value>) -> Result<Pending, Failure> {
    let width = params.get("width").and_then(Value::as_u64);
    let in_range =
        width.is_some_and(|width| (MIN_PLUGIN_WIDTH..=MAX_PLUGIN_WIDTH).contains(&width));
    if params.len() == 1 && in_range {
        Ok(Pending::None)
    } else {
        Err(denied(method, "width"))
    }
}

fn evaluate_context(method: &str, params: &Map<String, Value>) -> Result<Pending, Failure> {
    if params.contains_key("uniqueContextId") {
        return Err(denied(method, "context"));
    }
    match params.get("contextId") {
        None => Ok(Pending::None),
        Some(Value::Number(number)) => number
            .as_i64()
            .map(Pending::OwnContext)
            .ok_or_else(|| denied(method, "context")),
        Some(_) => Err(denied(method, "context")),
    }
}

/// `Page.getResourceContent`: a resource of a frame of the view's own session, named by a web
/// URL - never a local file or a browser page the frame may have loaded.
fn resource(method: &str, params: &Map<String, Value>) -> Result<Pending, Failure> {
    let frame = params.get("frameId").and_then(Value::as_str);
    if !frame.is_some_and(|frame| !frame.is_empty() && frame.len() <= MAX_REQUEST_ID) {
        return Err(denied(method, "params"));
    }
    let url = params
        .get("url")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if rules::check_http_url(url).is_ok() {
        Ok(Pending::None)
    } else {
        Err(denied(method, "url"))
    }
}

fn navigation(method: &str, params: &Map<String, Value>) -> Result<Pending, Failure> {
    let url = params
        .get("url")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if url.eq_ignore_ascii_case("about:blank") || rules::check_http_url(url).is_ok() {
        Ok(Pending::None)
    } else {
        Err(denied(method, "url"))
    }
}

fn cookie_urls(method: &str, params: &Map<String, Value>) -> Result<Pending, Failure> {
    // Without `urls` the command answers the cookies of every frame, third parties included.
    let urls: Option<Vec<String>> = params
        .get("urls")
        .and_then(Value::as_array)
        .filter(|urls| !urls.is_empty() && urls.len() <= MAX_COOKIE_URLS)
        .and_then(|urls| {
            urls.iter()
                .map(|url| url.as_str().map(str::to_owned))
                .collect()
        });
    urls.map(Pending::ViewOrigin)
        .ok_or_else(|| denied(method, "urls"))
}

fn cookie_target(method: &str, params: &Map<String, Value>) -> Result<Pending, Failure> {
    let text = |key: &str| -> Result<Option<String>, Failure> {
        match params.get(key) {
            None => Ok(None),
            Some(Value::String(text)) => Ok(Some(text.clone())),
            Some(_) => Err(denied(method, "cookie")),
        }
    };
    let url = text("url")?;
    let domain = text("domain")?;
    // Without either, `deleteCookies` deletes the name on every site.
    if url.is_none() && domain.is_none() {
        return Err(denied(method, "cookie"));
    }
    let mut domains: Vec<String> = domain.into_iter().collect();
    if let Some(partition) = params.get("partitionKey") {
        let site = partition
            .get("topLevelSite")
            .and_then(Value::as_str)
            .and_then(origin)
            .ok_or_else(|| denied(method, "cookie"))?;
        domains.push(site.1);
    }
    Ok(Pending::ViewHost {
        urls: url.into_iter().collect(),
        domains,
    })
}

/// Scheme, lower-case host (an IPv6 address in brackets) and port of an `http(s)` URL.
pub(crate) fn origin(url: &str) -> Option<(String, String, u16)> {
    rules::check_http_url(url).ok()?;
    let (scheme, rest) = url.split_once("://")?;
    let scheme = scheme.to_ascii_lowercase();
    let authority = rest.split(['/', '?', '#']).next()?.to_ascii_lowercase();
    let (host, port) = match authority.rfind(':') {
        Some(colon) if !authority[colon..].contains(']') => {
            let port = authority[colon + 1..].parse::<u16>().ok()?;
            (authority[..colon].to_owned(), port)
        }
        _ => {
            let port = if scheme == "https" { 443 } else { 80 };
            (authority, port)
        }
    };
    (!host.is_empty()).then_some((scheme, host, port))
}

/// Whether a cookie for `domain` may be set or deleted by a page on `host` (RFC 6265 domain
/// matching: the host itself or a parent domain of it, never a bare top-level name).
pub(crate) fn domain_matches(host: &str, domain: &str) -> bool {
    let domain = domain
        .strip_prefix('.')
        .unwrap_or(domain)
        .to_ascii_lowercase();
    if domain.is_empty() {
        return false;
    }
    if host == domain {
        return true;
    }
    let is_address = host.starts_with('[') || host.parse::<std::net::Ipv4Addr>().is_ok();
    !is_address && domain.contains('.') && host.ends_with(&format!(".{domain}"))
}

/// Whether the view showing `view_url` lets a command with `pending` through (cookies only).
pub(crate) fn view_allows(pending: &Pending, view_url: &str) -> bool {
    let view = origin(view_url);
    match pending {
        Pending::None => true,
        Pending::OwnContext(_) | Pending::WindowImage => false,
        Pending::ViewOrigin(urls) => view.is_some_and(|view| {
            urls.iter()
                .all(|url| origin(url).is_some_and(|other| other == view))
        }),
        Pending::ViewHost { urls, domains } => view.is_some_and(|(_, host, _)| {
            urls.iter()
                .all(|url| origin(url).is_some_and(|(_, other, _)| other == host))
                && domains.iter().all(|domain| domain_matches(&host, domain))
        }),
    }
}

#[cfg(test)]
#[path = "cdp_policy_tests.rs"]
mod tests;
