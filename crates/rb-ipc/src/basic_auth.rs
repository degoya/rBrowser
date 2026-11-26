//! Basic auth per environment (RB-094-29): whether a credential is stored for an environment, and
//! for which user - never the password, which stays in the platform's credential store
//! (`rbrowser::basic_auth`). The answer to `getBasicAuthStatus` and pushed after
//! `environment.auth` stored or removed one, so Settings > Project > Environments and the shell's
//! bar show it at once; with it, which store this platform uses, so the texts name it (RB-099-05).
//! Mirror: `ui/src/ipc/basicAuth.ts`.

use serde::{Deserialize, Serialize};

/// One environment's stored credential: the user, never the password (there is no field for it,
/// so neither the JSON nor the `Debug` output can carry one).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EnvironmentAuthStatus {
    /// The environment id.
    pub environment: String,
    /// Whether a credential is stored for the environment's address in this project.
    pub stored: bool,
    /// The stored credential's user name; absent when none is stored.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
}

impl EnvironmentAuthStatus {
    /// `environment` with a credential for `user`.
    #[must_use]
    pub fn stored(environment: &str, user: &str) -> Self {
        Self {
            environment: environment.to_owned(),
            stored: true,
            user: Some(user.to_owned()),
        }
    }

    /// `environment` without a credential.
    #[must_use]
    pub fn none(environment: &str) -> Self {
        Self {
            environment: environment.to_owned(),
            stored: false,
            user: None,
        }
    }
}

/// Where this platform keeps Basic auth credentials (RB-099-05), for the shell to name it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CredentialStoreKind {
    /// Windows: the Credential Manager.
    WindowsCredentialManager,
    /// macOS: the login keychain.
    MacosKeychain,
    /// Linux: the Secret Service (GNOME Keyring, KWallet).
    SecretService,
    /// A platform without a store rBrowser uses: nothing is kept.
    None,
}

#[cfg(test)]
#[path = "basic_auth_tests.rs"]
mod tests;
