//! CEF's `GetAuthCredentials` (IO thread) for a grid view (RB-094-08): the stored credential of
//! an environment behind Basic auth answers its own origin's challenge, once per view and realm;
//! every other challenge - the shell's, a proxy's, another site's, a refused credential's - is
//! left to CEF's default (`0`), as before. Neither the user nor the password is logged; the
//! refusal is, at debug level, by its reason only.

use std::os::raw::c_int;

use cef::*;
use rb_engine::ViewId;

use crate::basic_auth::{Challenge, Refusal, credential_for};

/// The challenge as CEF reports it.
pub struct Asked<'a> {
    pub origin_url: Option<&'a CefString>,
    pub is_proxy: c_int,
    pub host: Option<&'a CefString>,
    pub port: c_int,
    pub realm: Option<&'a CefString>,
    pub scheme: Option<&'a CefString>,
}

/// Answers `asked` in the view `view` (`None`: the shell) through `callback`; `1` when a
/// credential was handed over.
pub fn answer(
    view: Option<ViewId>,
    browser: Option<&mut Browser>,
    asked: &Asked<'_>,
    callback: Option<&mut AuthCallback>,
) -> c_int {
    let (Some(_), Some(browser), Some(callback)) = (view, browser, callback) else {
        return 0;
    };
    let Some(core) = super::views::core() else {
        return 0;
    };
    let text = |value: Option<&CefString>| value.map(ToString::to_string).unwrap_or_default();
    let (origin_url, host, scheme) = (text(asked.origin_url), text(asked.host), text(asked.scheme));
    let realm = text(asked.realm);
    let challenge = Challenge {
        origin_url: &origin_url,
        is_proxy: asked.is_proxy != 0,
        host: &host,
        port: asked.port,
        realm: &realm,
        scheme: &scheme,
    };
    match credential_for(core, &challenge, browser.identifier()) {
        Ok(credential) => {
            callback.cont(
                Some(&CefString::from(credential.user())),
                Some(&CefString::from(credential.password())),
            );
            tracing::debug!(?view, "a Basic auth challenge was answered");
            1
        }
        Err(Refusal::NotAnEnvironment | Refusal::Scheme | Refusal::Proxy) => 0,
        Err(refusal) => {
            tracing::debug!(?view, %refusal, "a Basic auth challenge is left to the person");
            0
        }
    }
}
