//! The way from CEF's `GetAuthCredentials` to an answer (RB-099-04, review w35): the IO thread
//! only hands the challenge over and returns; the store is read in a job on a thread that may
//! block (on macOS and Linux a locked keyring may ask the person first, RB-099-05), and its
//! credential answers - or the person is asked in the bar (`super::ask`), never for a windowless
//! render view, whose request is cancelled.

use std::sync::Arc;

use rb_engine::ViewId;

use super::ask::{Ask, Asking, TIMEOUT, Waiter};
use super::decide::{Challenge, shown_origin};
use super::service::BasicAuth;
use crate::control::Core;

/// Who a challenge came from, and whether a person may be asked (review w35).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Requester {
    /// A grid view.
    View(ViewId),
    /// A window a grid view opened: asked in the bar, named by its opener.
    Popup(ViewId),
    /// A windowless render view (sitemap capture): nobody watches, nobody is asked - the stored
    /// credential or nothing.
    Render,
}

impl Requester {
    /// The view the bar names, when a person may be asked.
    #[must_use]
    pub fn asks(self) -> Option<ViewId> {
        match self {
            Self::View(view) | Self::Popup(view) => Some(view),
            Self::Render => None,
        }
    }
}

/// A challenge as CEF reports it, owned, so it can wait for the store off CEF's IO thread.
#[derive(Clone, Debug)]
pub struct OwnedChallenge {
    /// The challenged request's URL: never logged, never shown.
    pub origin_url: String,
    /// Whether a proxy asks.
    pub is_proxy: bool,
    /// The server's host.
    pub host: String,
    /// Its port.
    pub port: i32,
    /// The realm.
    pub realm: String,
    /// The scheme.
    pub scheme: String,
}

impl OwnedChallenge {
    /// The challenge borrowed.
    #[must_use]
    pub fn challenge(&self) -> Challenge<'_> {
        Challenge {
            origin_url: &self.origin_url,
            is_proxy: self.is_proxy,
            host: &self.host,
            port: self.port,
            realm: &self.realm,
            scheme: &self.scheme,
        }
    }
}

/// A piece of work for a thread that may block (`spawn_blocking` in the app).
pub type Job = Box<dyn FnOnce() + Send>;

/// Answers `challenge` of `requester` (CEF browser `browser`) through `waiter`, later: the store
/// is read in the job `spawn` runs (never on CEF's IO thread, which returns at once); its
/// credential answers, or the person is asked - not for a render view, whose request is
/// cancelled - the question cancelled after [`TIMEOUT`].
#[allow(
    clippy::too_many_arguments,
    reason = "CEF's challenge, its view and waiter, and the three services it is answered from"
)]
pub fn answer_later(
    core: Arc<Core>,
    auth: &'static BasicAuth,
    asking: &'static Asking,
    requester: Requester,
    browser: i32,
    challenge: OwnedChallenge,
    waiter: Box<dyn Waiter>,
    spawn: impl FnOnce(Job),
) {
    spawn(Box::new(move || {
        let challenge = challenge.challenge();
        let host = core.host();
        let project = host
            .project_root()
            .map(|root| crate::instance::file::key(&root));
        match auth.answer(&challenge, browser, project.as_deref(), &host.config()) {
            Ok(credential) => waiter.sign_in(credential.user(), credential.password()),
            Err(refusal) => {
                let Some(view) = requester.asks() else {
                    tracing::info!(
                        browser,
                        origin = ?shown_origin(&challenge),
                        reason = %refusal,
                        "Basic auth: a render view's challenge is cancelled; nobody is asked there"
                    );
                    waiter.cancel();
                    return;
                };
                let asked = asking.ask(&core, view, browser, &challenge, refusal, waiter);
                if let Ask::Waiting(Some(id)) = asked {
                    let later = Arc::clone(&core);
                    core.runtime().spawn(async move {
                        tokio::time::sleep(TIMEOUT).await;
                        asking.expire(&later, id);
                    });
                }
            }
        }
    }));
}

#[cfg(test)]
#[path = "later_tests.rs"]
mod tests;
