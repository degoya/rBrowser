//! [`HostJars`]: the jars of one window (RB-040-05) - a Chromium session's cookies read over
//! the view's CDP session (`Network.getAllCookies`, which answers the cookies of the view's own
//! CEF request context, so of its session), Firefox's jar of a session through
//! `rb_bidi::FirefoxEngine::set_cookies`, a WebKit view's context through the sidecar's
//! `webkit.view.cookies`, and the reload through the external window's handle. None of them
//! starts an engine that is not running.

use std::sync::Arc;

use async_trait::async_trait;
use rb_bidi::FirefoxEngine;
use rb_bidi::storage::Cookie as BidiCookie;
use rb_engine::{EngineError, ViewHost, ViewId};
use serde_json::json;

use super::copy::{Jars, Taken};
use super::mapping::PlaywrightCookie;
use super::model::{ChromeCookie, cookies_of};
use crate::engines::webkit::WebkitEngine;
use crate::host::Host;

/// The CDP method that reads a session's cookies through one of its views.
pub const READ_METHOD: &str = "Network.getAllCookies";

/// The jars of `host`'s window.
pub struct HostJars {
    /// The window.
    pub host: Arc<Host>,
}

#[async_trait]
impl Jars for HostJars {
    async fn read(&self, view: ViewId) -> Result<Vec<ChromeCookie>, String> {
        self.host
            .cdp_send(view, READ_METHOD, json!({}))
            .await
            .map(|answer| cookies_of(&answer))
            .map_err(|error| error.to_string())
    }

    async fn set_firefox(&self, session: &str, cookies: &[BidiCookie]) -> Result<Taken, String> {
        let engine =
            crate::engines::firefox::for_host(&self.host).and_then(|bridge| bridge.firefox());
        into_firefox(engine, session, cookies).await
    }

    async fn set_webkit(
        &self,
        view: ViewId,
        cookies: &[PlaywrightCookie],
    ) -> Result<Taken, String> {
        into_webkit(crate::engines::webkit::for_host(&self.host), view, cookies).await
    }

    async fn reload(&self, view: ViewId) -> Result<(), String> {
        let handle = crate::engines::for_host(&self.host)
            .and_then(|engines| engines.windows().handle(view))
            .ok_or_else(|| EngineError::UnknownView(view).to_string())?;
        handle
            .reload(false)
            .await
            .map_err(|error| error.to_string())
    }
}

/// `cookies` into the jar of `session` of the running Firefox, if there is one.
pub async fn into_firefox(
    engine: Option<Arc<FirefoxEngine>>,
    session: &str,
    cookies: &[BidiCookie],
) -> Result<Taken, String> {
    let not_running = || rb_i18n::t("cookies.error.firefoxNotRunning");
    let engine = engine.ok_or_else(not_running)?;
    match engine.set_cookies(session, cookies).await {
        Ok(outcome) => Ok(Taken {
            set: outcome.set,
            refused: outcome.refused,
        }),
        Err(rb_bidi::BidiError::Closed) => Err(not_running()),
        Err(error) => Err(error.to_string()),
    }
}

/// `cookies` into the WebKit view `view`, if WebKit runs it.
pub async fn into_webkit(
    engine: Option<Arc<WebkitEngine>>,
    view: ViewId,
    cookies: &[PlaywrightCookie],
) -> Result<Taken, String> {
    let not_running = || rb_i18n::t("cookies.error.webkitNotRunning");
    let engine = engine.ok_or_else(not_running)?;
    let list = serde_json::to_value(cookies).map_err(|error| error.to_string())?;
    match engine.add_cookies(view, list).await {
        Ok((set, refused)) => Ok(Taken { set, refused }),
        Err(EngineError::Closed | EngineError::UnknownView(_)) => Err(not_running()),
        Err(error) => Err(error.to_string()),
    }
}

#[cfg(test)]
#[path = "jars_tests.rs"]
pub(crate) mod tests;
