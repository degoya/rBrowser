//! Where a Basic auth credential is kept: the platform's credential store, one entry per project
//! and origin (`rbrowser/basic-auth/<project key>/<scheme://host:port>`), never a file of the
//! project. On Windows that is the Credential Manager (generic credential, persisted for the
//! user on this machine, encrypted with DPAPI; `store_windows`). Linux and macOS (phase 2) get
//! the Secret Service and the Keychain behind the same trait; until then they keep nothing and
//! setting a credential fails, so a password never lands in a plain file.

use std::fmt;

use super::origin::AuthOrigin;

/// The longest user name taken.
pub const MAX_USER: usize = 256;
/// The longest password taken.
pub const MAX_PASSWORD: usize = 1024;

/// A user name and password. Neither reaches a `Debug` output.
#[derive(Clone, PartialEq, Eq)]
pub struct Credential {
    user: String,
    password: String,
}

/// Why a user name or password is not taken.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum CredentialError {
    /// Empty, too long, a `:` (RFC 7617) or a control character.
    #[error("the user name is empty, too long or holds a ':' or a control character")]
    User,
    /// Shorter than `rb_diag` masks, too long or a control character.
    #[error("the password is shorter than 4 characters, too long or holds a control character")]
    Password,
}

impl Credential {
    /// The credential `user`/`password`.
    ///
    /// # Errors
    ///
    /// [`CredentialError`] for a value Basic auth cannot carry.
    pub fn new(user: &str, password: &str) -> Result<Self, CredentialError> {
        let plain = |text: &str, max: usize| {
            !text.is_empty() && text.chars().count() <= max && !text.chars().any(char::is_control)
        };
        if !plain(user, MAX_USER) || user.contains(':') {
            return Err(CredentialError::User);
        }
        // Shorter than `rb_diag` masks (it skips such values), it could reach a log unmasked.
        if !plain(password, MAX_PASSWORD)
            || password.chars().count() < rb_diag::redact::MIN_SECRET_LEN
        {
            return Err(CredentialError::Password);
        }
        Ok(Self {
            user: user.to_owned(),
            password: password.to_owned(),
        })
    }

    /// The user name.
    #[must_use]
    pub fn user(&self) -> &str {
        &self.user
    }

    /// The password: a secret.
    #[must_use]
    pub fn password(&self) -> &str {
        &self.password
    }
}

impl fmt::Debug for Credential {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Credential")
            .field("user", &"<masked>")
            .field("password", &"<masked>")
            .finish()
    }
}

/// Why the credential store failed. Never carries a value.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum StoreError {
    /// This platform has no credential store rBrowser uses yet (phase 2).
    #[error("this platform keeps no Basic auth credentials yet")]
    Unsupported,
    /// The platform's store refused (the OS error code).
    #[error("the credential store failed (code {0})")]
    Failed(u32),
}

/// A store of credentials by target name.
pub trait CredentialStore: Send + Sync {
    /// The credential under `target`, if there is one.
    ///
    /// # Errors
    ///
    /// [`StoreError`] when the store cannot be read.
    fn read(&self, target: &str) -> Result<Option<Credential>, StoreError>;

    /// Keeps `credential` under `target`, replacing what was there.
    ///
    /// # Errors
    ///
    /// [`StoreError`] when the store refuses.
    fn write(&self, target: &str, credential: &Credential) -> Result<(), StoreError>;

    /// Removes the credential under `target`; `false` when there was none.
    ///
    /// # Errors
    ///
    /// [`StoreError`] when the store refuses.
    fn remove(&self, target: &str) -> Result<bool, StoreError>;
}

/// The name of the credential of `origin` in the project with key `project`
/// (`crate::instance::file::key`).
#[must_use]
pub fn target_name(project: &str, origin: &AuthOrigin) -> String {
    format!("rbrowser/basic-auth/{project}/{origin}")
}

/// The store of this platform.
#[must_use]
pub fn platform_store() -> Box<dyn CredentialStore> {
    #[cfg(windows)]
    {
        Box::new(super::store_windows::WindowsStore)
    }
    #[cfg(not(windows))]
    {
        Box::new(NoStore)
    }
}

/// Linux and macOS until phase 2: keeps nothing.
#[cfg(not(windows))]
struct NoStore;

#[cfg(not(windows))]
impl CredentialStore for NoStore {
    fn read(&self, _target: &str) -> Result<Option<Credential>, StoreError> {
        Ok(None)
    }

    fn write(&self, _target: &str, _credential: &Credential) -> Result<(), StoreError> {
        Err(StoreError::Unsupported)
    }

    fn remove(&self, _target: &str) -> Result<bool, StoreError> {
        Ok(false)
    }
}

/// A store in memory, for tests.
#[cfg(test)]
#[derive(Default)]
pub struct MemoryStore {
    entries: std::sync::Mutex<std::collections::HashMap<String, Credential>>,
}

#[cfg(test)]
impl CredentialStore for MemoryStore {
    fn read(&self, target: &str) -> Result<Option<Credential>, StoreError> {
        Ok(rb_util::lock(&self.entries).get(target).cloned())
    }

    fn write(&self, target: &str, credential: &Credential) -> Result<(), StoreError> {
        rb_util::lock(&self.entries).insert(target.to_owned(), credential.clone());
        Ok(())
    }

    fn remove(&self, target: &str) -> Result<bool, StoreError> {
        Ok(rb_util::lock(&self.entries).remove(target).is_some())
    }
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;
