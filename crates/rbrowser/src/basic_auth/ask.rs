//! rBrowser's own sign-in dialog (RB-099-04): Chromium's prompt is off
//! (`launch::LOGIN_PROMPT_OFF`), so a challenge the stored credentials do not answer - another
//! site's, a proxy's, a refused password's - is asked here. CEF's callback waits ([`Waiter`]);
//! the question goes to the shell's bar (`authPrompt`, `ui/src/basic-auth/AuthPromptBar.vue`),
//! never to a page, and the person's `authPromptReply` answers every view waiting on it - or
//! cancels them, and the server's 401 page shows. An answer is used once (Chromium's auth cache
//! keeps it for the session) unless the person saves it for the environment the core named.
//! MCP and plugins neither see nor answer a question: it travels only between the core and the
//! shell's page. Even the store is read off CEF's IO thread (`super::later`): on macOS and Linux
//! a locked keyring may ask the person first, and the IO thread carries every view's network. The user and the password reach no log, error or `Debug` output; the log names
//! the question's origin, realm and why it was asked.

use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use rb_engine::ViewId;
use rb_ipc::{AuthPromptReply, CoreToShell, IpcErrorCode, Notification, NotificationLevel};

use super::command::{NOT_LISTED_KEY, SAVED_KEY, notice};
use super::decide::{Challenge, Refusal, environment_origin, saveable, shown_origin, unencrypted};
use super::prompt::{Asked, Change, PromptKey, Prompts, Waiting};
use super::service::BasicAuth;
use super::store::Credential;
use crate::control::Core;
use crate::ipc::Failure;

/// How long a question waits for the person before its requests are cancelled.
pub const TIMEOUT: Duration = Duration::from_secs(5 * 60);

/// A request that waits for the person's answer: CEF's `AuthCallback` in the app.
pub trait Waiter: Send {
    /// Sends `user` and `password`.
    fn sign_in(&self, user: &str, password: &str);
    /// Gives up: the request goes on without a credential.
    fn cancel(&self);
}

/// The notice after the server refused every answer in a view; placeholder `{origin}`.
pub const GAVE_UP_KEY: &str = "auth.prompt.gaveUp";

/// What [`Asking::ask`] decided.
#[derive(Debug, PartialEq, Eq)]
pub enum Ask {
    /// The request waits; `Some(id)` for a new question, whose timeout the caller starts.
    Waiting(Option<u64>),
    /// Not asked: the view was refused too often, or closed. The request was cancelled.
    NotAsked,
}

/// The open questions of the process.
#[derive(Default)]
pub struct Asking {
    pub(super) prompts: Mutex<Prompts<Box<dyn Waiter>>>,
}

static ASKING: OnceLock<Asking> = OnceLock::new();

/// The process's questions.
pub fn asking() -> &'static Asking {
    ASKING.get_or_init(Asking::default)
}

impl Asking {
    /// Asks the person for `challenge` of the view `view` (CEF browser `browser`), which the
    /// stored credentials did not answer for `refusal`.
    pub fn ask(
        &self,
        core: &Core,
        view: ViewId,
        browser: i32,
        challenge: &Challenge<'_>,
        refusal: Refusal,
        waiter: Box<dyn Waiter>,
    ) -> Ask {
        let key = PromptKey {
            origin: shown_origin(challenge),
            realm: challenge.realm.to_owned(),
            proxy: challenge.is_proxy,
        };
        let waiting = Waiting {
            key,
            view,
            browser,
            environment: saveable(challenge, &core.host().config()),
            stored_refused: refusal == Refusal::Refused,
            insecure: unencrypted(challenge),
            waiter,
        };
        let asked = rb_util::lock(&self.prompts).ask(waiting);
        match asked {
            Asked::Cancel(waiter) => {
                waiter.cancel();
                let origin = shown_origin(challenge);
                tracing::info!(
                    ?view,
                    origin = ?origin,
                    realm = ?challenge.realm,
                    "Basic auth: the server refused every answer in this view; not asked again"
                );
                core.notify(CoreToShell::Notification(Notification {
                    level: NotificationLevel::Warning,
                    message_key: GAVE_UP_KEY.to_owned(),
                    args: BTreeMap::from([("origin".to_owned(), origin)]),
                    settings_key: None,
                }));
                Ask::NotAsked
            }
            Asked::Gone(waiter) => {
                waiter.cancel();
                tracing::info!(
                    ?view,
                    "Basic auth: the view closed before its challenge was asked"
                );
                Ask::NotAsked
            }
            Asked::Show { prompt, new } => {
                tracing::info!(
                    ?view,
                    id = prompt.id,
                    origin = ?prompt.origin,
                    realm = ?prompt.realm,
                    proxy = prompt.proxy,
                    reason = %refusal,
                    "Basic auth: the person is asked to sign in"
                );
                let id = prompt.id;
                core.send(&CoreToShell::AuthPrompt(prompt));
                if new {
                    core.ui().focus_shell();
                }
                Ask::Waiting(new.then_some(id))
            }
        }
    }

    /// The shell's `authPromptReply`: the views waiting get the user and password, stored first
    /// for the question's environment when the person asked so, or are cancelled.
    ///
    /// # Errors
    ///
    /// A [`Failure`] in fixed words: no such question, nothing to save for, a credential the
    /// store refuses. The question stays open then.
    pub fn reply(
        &self,
        core: &Core,
        auth: &BasicAuth,
        reply: &AuthPromptReply,
    ) -> Result<CoreToShell, Failure> {
        let gone = || {
            (
                IpcErrorCode::NotFound,
                "this sign-in no longer waits".to_owned(),
            )
        };
        let Some(sign_in) = &reply.sign_in else {
            let waiters = rb_util::lock(&self.prompts)
                .cancel(reply.id)
                .ok_or_else(gone)?;
            tracing::info!(
                id = reply.id,
                "Basic auth: the person cancelled the sign-in"
            );
            waiters.iter().for_each(|waiter| waiter.cancel());
            return Ok(CoreToShell::Ack);
        };
        rb_diag::register_secret(&sign_in.password);
        let (environment, origin) = {
            let prompts = rb_util::lock(&self.prompts);
            let environment = prompts.environment(reply.id).ok_or_else(gone)?;
            (environment, prompts.origin(reply.id).ok_or_else(gone)?)
        };
        let mut saved = false;
        if sign_in.save {
            let environment = environment.ok_or_else(|| {
                (
                    IpcErrorCode::BadRequest,
                    "this address belongs to no environment; the sign-in cannot be saved"
                        .to_owned(),
                )
            })?;
            saved = save(
                core,
                auth,
                &environment,
                &origin,
                &sign_in.user,
                &sign_in.password,
            )?;
        }
        let answered = rb_util::lock(&self.prompts)
            .sign_in(reply.id)
            .ok_or_else(gone)?;
        tracing::info!(
            id = reply.id,
            origin = ?answered.key.origin,
            realm = ?answered.key.realm,
            views = answered.waiters.len(),
            saved,
            "Basic auth: the person signed in"
        );
        for waiter in &answered.waiters {
            waiter.sign_in(&sign_in.user, &sign_in.password);
        }
        Ok(CoreToShell::Ack)
    }

    /// The view of `browser` closed: its requests are dropped, a question nobody waits on is
    /// withdrawn. At the end of the app every view closes, so every question ends.
    pub fn view_closed(&self, core: &Core, browser: i32) {
        let (waiters, changes) = rb_util::lock(&self.prompts).drop_browser(browser);
        waiters.iter().for_each(|waiter| waiter.cancel());
        for change in changes {
            core.send(&match change {
                Change::Show(prompt) => CoreToShell::AuthPrompt(prompt),
                Change::Closed(id) => CoreToShell::AuthPromptClosed { id },
            });
        }
    }

    /// The question `id` waited [`TIMEOUT`]: its requests are cancelled.
    pub fn expire(&self, core: &Core, id: u64) {
        let Some(waiters) = rb_util::lock(&self.prompts).cancel(id) else {
            return;
        };
        tracing::info!(
            id,
            "Basic auth: a sign-in waited too long and was cancelled"
        );
        waiters.iter().for_each(|waiter| waiter.cancel());
        core.send(&CoreToShell::AuthPromptClosed { id });
    }

    /// A credential was set or removed: every view may be answered again.
    pub fn reset(&self) {
        rb_util::lock(&self.prompts).reset_tries();
    }
}

/// Stores the answer for `environment`, as `environment.auth` would, without reloading the
/// views that wait for it; `false` when the environment's address is no longer the question's
/// `origin` (its URL changed while the question was open): the answer is used once then and
/// stored nowhere, so a credential typed for one origin is never sent to another (review w35).
fn save(
    core: &Core,
    auth: &BasicAuth,
    environment: &str,
    origin: &str,
    user: &str,
    password: &str,
) -> Result<bool, Failure> {
    let refused = |what: &str| (IpcErrorCode::BadRequest, what.to_owned());
    let credential = Credential::new(user, password).map_err(|_| {
        refused("not saved: enter a user without a colon and a password of at least 4 characters")
    })?;
    let host = core.host();
    let root = host
        .project_root()
        .ok_or_else(|| refused("not saved: open a project first"))?;
    let project = crate::instance::file::key(&root);
    let config = host.config();
    let now = environment_origin(&config, environment)
        .and_then(Result::ok)
        .map(|current| current.to_string());
    if now.as_deref() != Some(origin) {
        tracing::info!(
            environment,
            origin = ?origin,
            "Basic auth: not saved - the environment's address changed while it was asked; used once"
        );
        return Ok(false);
    }
    auth.set(&project, &config, environment, &credential)
        .map_err(|error| (IpcErrorCode::Internal, error.to_string()))?;
    let listed = config.basic_auth.uses(environment);
    let (level, key) = if listed {
        (NotificationLevel::Info, SAVED_KEY)
    } else {
        (NotificationLevel::Warning, NOT_LISTED_KEY)
    };
    core.notify(notice(level, key, environment));
    super::status::push(core);
    Ok(true)
}

#[cfg(test)]
#[path = "ask_tests.rs"]
pub(super) mod tests;
