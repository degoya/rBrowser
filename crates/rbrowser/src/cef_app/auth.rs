//! CEF's `GetAuthCredentials` (IO thread) for a grid view (RB-094-08): the stored credential of
//! an environment behind Basic auth answers its own origin's challenge, once per view and realm.
//! Chromium's own sign-in prompt is off (`disable-chrome-login-prompt`, `launch.rs`, RB-099-04):
//! without it, CEF hands a browser's challenge to that prompt and never calls this handler. The
//! grid views, their popups and the windowless render views all come here
//! (`basic_auth::later::Requester`); the store is read off this IO thread. Every other challenge
//! of a grid view or popup - a proxy's, another site's, a refused credential's - keeps its
//! callback and goes to rBrowser's own sign-in dialog in the shell (`basic_auth::ask`); Cancel,
//! a closed view or the timeout cancel it and the server's 401 page shows. A render view is never
//! asked; the shell's own challenges are cancelled (`0`). Every decision is logged at INFO
//! (`basic_auth::service`, `basic_auth::ask`, `basic_auth::later`) - environment, origin, realm,
//! reason - never the user, the password or the request URL.

use std::os::raw::c_int;
use std::sync::Arc;

use cef::*;

use crate::basic_auth::ask::{Waiter, asking};
use crate::basic_auth::later::{OwnedChallenge, Requester, answer_later};
use crate::basic_auth::service;

/// CEF's callback of a request that waits for the person.
struct CefWaiter(AuthCallback);

impl Waiter for CefWaiter {
    fn sign_in(&self, user: &str, password: &str) {
        self.0.cont(
            Some(&CefString::from(user)),
            Some(&CefString::from(password)),
        );
    }

    fn cancel(&self) {
        self.0.cancel();
    }
}

/// The challenge as CEF reports it.
pub struct Asked<'a> {
    /// The URL of the request that was challenged (path and query included): never logged.
    pub origin_url: Option<&'a CefString>,
    pub is_proxy: c_int,
    pub host: Option<&'a CefString>,
    pub port: c_int,
    pub realm: Option<&'a CefString>,
    pub scheme: Option<&'a CefString>,
}

/// Answers `asked` of `requester` (`None`: the shell) through `callback`, later: `1` when the
/// callback is kept for the store or the person, `0` (cancel) for the shell.
pub fn answer(
    requester: Option<Requester>,
    browser: Option<&mut Browser>,
    asked: &Asked<'_>,
    callback: Option<&mut AuthCallback>,
) -> c_int {
    let text = |value: Option<&CefString>| value.map(ToString::to_string).unwrap_or_default();
    let (origin_url, host, scheme) = (text(asked.origin_url), text(asked.host), text(asked.scheme));
    let realm = text(asked.realm);
    let (Some(requester), Some(browser), Some(callback)) = (requester, browser, callback) else {
        tracing::info!(
            ?requester,
            host = ?host,
            port = asked.port,
            realm = ?realm,
            "Basic auth: a challenge of the shell's own pages is not answered"
        );
        return 0;
    };
    let Some(core) = super::views::core() else {
        tracing::info!(
            ?requester,
            "Basic auth: a challenge before the core started is not answered"
        );
        return 0;
    };
    let challenge = OwnedChallenge {
        origin_url,
        is_proxy: asked.is_proxy != 0,
        host,
        port: asked.port,
        realm,
        scheme,
    };
    // The store is read off this IO thread (a locked keyring may ask first, RB-099-05); the
    // callback is answered when it returns: the stored credential, or rBrowser's dialog.
    let waiter = Box::new(CefWaiter(callback.clone()));
    let runtime = core.runtime().clone();
    answer_later(
        Arc::clone(core),
        service(),
        asking(),
        requester,
        browser.identifier(),
        challenge,
        waiter,
        move |job| {
            runtime.spawn_blocking(job);
        },
    );
    1
}

/// A view's or popup's browser closed (`OnBeforeClose`): its requests waiting for the person are
/// dropped, and a request of it still on the store's thread is never asked.
pub fn view_closed(browser: Option<&Browser>) {
    if let (Some(core), Some(browser)) = (super::views::core(), browser) {
        asking().view_closed(core, browser.identifier());
    }
}
