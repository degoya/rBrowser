//! The one Basic auth service of the process: the credential store, the answers given, and the
//! three things done with them - answer a challenge (CEF's IO thread), set and remove a
//! credential (`environment.auth`). A password is registered with `rb_diag` the moment it enters
//! the process, so no log line or diagnostics ZIP can show it; the errors name environments and
//! origins only.

use std::sync::{Mutex, OnceLock};

use rb_config::ProjectConfig;

use super::attempts::Attempts;
use super::decide::{Challenge, Refusal, eligible, environment_origin};
use super::origin::{AuthOrigin, OriginError};
use super::store::{Credential, CredentialStore, StoreError, platform_store, target_name};

/// Why a credential was not set or removed. Never carries a value.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum AuthError {
    /// No environment has this id.
    #[error("there is no environment {0:?}")]
    UnknownEnvironment(String),
    /// The environment's URL may not carry a credential.
    #[error("the environment's address may not carry Basic auth: {0}")]
    Origin(OriginError),
    /// The credential store failed.
    #[error("{0}")]
    Store(StoreError),
    /// No project is open.
    #[error("open a project first")]
    NoProject,
}

/// The store and the answers given.
pub struct BasicAuth {
    store: Box<dyn CredentialStore>,
    attempts: Mutex<Attempts>,
}

static SERVICE: OnceLock<BasicAuth> = OnceLock::new();

/// The process's service over the platform's store.
pub fn service() -> &'static BasicAuth {
    SERVICE.get_or_init(|| BasicAuth::new(platform_store()))
}

impl BasicAuth {
    /// A service over `store`.
    #[must_use]
    pub fn new(store: Box<dyn CredentialStore>) -> Self {
        Self {
            store,
            attempts: Mutex::new(Attempts::default()),
        }
    }

    /// The credential that answers `challenge` in the view with browser id `browser` of the
    /// project with key `project` and config `config`.
    ///
    /// # Errors
    ///
    /// The [`Refusal`]: CEF leaves the challenge to the person.
    pub fn answer(
        &self,
        challenge: &Challenge<'_>,
        browser: i32,
        project: Option<&str>,
        config: &ProjectConfig,
    ) -> Result<Credential, Refusal> {
        let project = project.ok_or(Refusal::NoProject)?;
        let found = eligible(challenge, config)?;
        let credential = self
            .store
            .read(&target_name(project, &found.origin))
            .map_err(|error| {
                tracing::warn!(%error, "the Basic auth credential could not be read");
                Refusal::NoCredential
            })?
            .ok_or(Refusal::NoCredential)?;
        rb_diag::register_secret(credential.password());
        if !rb_util::lock(&self.attempts).take(browser, &found.origin, challenge.realm) {
            return Err(Refusal::Refused);
        }
        Ok(credential)
    }

    /// Stores `credential` for the environment `id` of `config` in the project `project`;
    /// answers the origin it is bound to.
    ///
    /// # Errors
    ///
    /// [`AuthError`]: an unknown environment, an address that may not carry it, the store.
    pub fn set(
        &self,
        project: &str,
        config: &ProjectConfig,
        id: &str,
        credential: &Credential,
    ) -> Result<AuthOrigin, AuthError> {
        rb_diag::register_secret(credential.password());
        let origin = origin_of(config, id)?;
        self.store
            .write(&target_name(project, &origin), credential)
            .map_err(AuthError::Store)?;
        rb_util::lock(&self.attempts).reset();
        Ok(origin)
    }

    /// Removes the credential of the environment `id`; `false` when there was none.
    ///
    /// # Errors
    ///
    /// [`AuthError`]: an unknown environment, an address that may not carry one, the store.
    pub fn remove(
        &self,
        project: &str,
        config: &ProjectConfig,
        id: &str,
    ) -> Result<bool, AuthError> {
        let origin = origin_of(config, id)?;
        let removed = self
            .store
            .remove(&target_name(project, &origin))
            .map_err(AuthError::Store)?;
        rb_util::lock(&self.attempts).reset();
        Ok(removed)
    }
}

fn origin_of(config: &ProjectConfig, id: &str) -> Result<AuthOrigin, AuthError> {
    environment_origin(config, id)
        .ok_or_else(|| AuthError::UnknownEnvironment(id.to_owned()))?
        .map_err(AuthError::Origin)
}

#[cfg(test)]
#[path = "service_tests.rs"]
mod tests;
