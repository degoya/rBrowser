//! The Secret Service as the store of Basic auth credentials on Linux (RB-099-05): the D-Bus
//! service `org.freedesktop.secrets` that GNOME Keyring and KWallet provide, over the session
//! bus through the system's libdbus (`dbus-secret-service`). One item per target in the default
//! collection (the login keyring), found by the attributes `service` and `account`
//! (`store_codec`); its secret `user:password` travels in a session encrypted with
//! Diffie-Hellman (`EncryptionType::Dh`), so not even the bus sees it in clear. Each call opens
//! its own connection, given up when the bus does not answer within [`CONNECT_TIMEOUT`]: no
//! state, no secret held between calls.
//!
//! Without a session bus or without anybody serving the Secret Service (a server, WSL without
//! D-Bus), every call fails with [`StoreError::NoSecretService`] - rBrowser never falls back to a
//! file. A locked keyring is unlocked through the service's own prompt, which the call waits
//! for; dismissing it is [`StoreError::Denied`].

use std::collections::HashMap;
use std::time::Duration;

use dbus_secret_service::{EncryptionType, Error, SecretService};

use super::codec;
use super::{Credential, CredentialStore, StoreError};

/// How long the session bus may take to accept the connection.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);

/// The Secret Service on the session bus (`DBUS_SESSION_BUS_ADDRESS`).
pub struct SecretServiceStore;

fn connect() -> Result<SecretService, StoreError> {
    SecretService::connect_with_connection_timeout(EncryptionType::Dh, CONNECT_TIMEOUT)
        .map_err(|error| store_error(&error))
}

/// The attributes of `target` as the service takes them.
fn attributes(target: &str) -> HashMap<&str, &str> {
    codec::attributes(target).into_iter().collect()
}

impl CredentialStore for SecretServiceStore {
    fn read(&self, target: &str) -> Result<Option<Credential>, StoreError> {
        let service = connect()?;
        let found = service
            .search_items(attributes(target))
            .map_err(|error| store_error(&error))?;
        let item = match (found.unlocked.first(), found.locked.first()) {
            (Some(item), _) => item,
            (None, Some(item)) => {
                item.unlock().map_err(|error| store_error(&error))?;
                item
            }
            (None, None) => return Ok(None),
        };
        let mut secret = item.get_secret().map_err(|error| store_error(&error))?;
        let credential = codec::credential_of(&secret);
        secret.fill(0);
        Ok(credential)
    }

    fn write(&self, target: &str, credential: &Credential) -> Result<(), StoreError> {
        let service = connect()?;
        // The default collection only: the `session` one would forget the credential at logout.
        let collection = service
            .get_default_collection()
            .map_err(|error| match error {
                Error::NoResult => StoreError::NoCollection,
                error => store_error(&error),
            })?;
        collection
            .ensure_unlocked()
            .map_err(|error| store_error(&error))?;
        let mut secret = codec::secret_of(credential);
        // `replace`: the item with the same attributes is overwritten, so a target has one.
        let created = collection.create_item(
            &codec::label(target),
            attributes(target),
            &secret,
            true,
            "text/plain",
        );
        // The copy of the password goes before an error can be returned.
        secret.fill(0);
        created.map(|_item| ()).map_err(|error| store_error(&error))
    }

    fn remove(&self, target: &str) -> Result<bool, StoreError> {
        let service = connect()?;
        let found = service
            .search_items(attributes(target))
            .map_err(|error| store_error(&error))?;
        for item in &found.locked {
            item.unlock().map_err(|error| store_error(&error))?;
        }
        let mut removed = false;
        for item in found.unlocked.iter().chain(&found.locked) {
            item.delete().map_err(|error| store_error(&error))?;
            removed = true;
        }
        Ok(removed)
    }
}

/// The [`StoreError`] of a Secret Service error: what failed, never a value - the service's own
/// error text is dropped, only a D-Bus error's name is looked at.
fn store_error(error: &Error) -> StoreError {
    match error {
        Error::Unavailable => StoreError::NoSecretService,
        Error::Locked | Error::Prompt => StoreError::Denied,
        Error::Dbus(error) => codec::dbus_error(error.name().unwrap_or_default()),
        Error::NoResult => StoreError::SecretService("no result"),
        Error::Crypto(_) => StoreError::SecretService("crypto"),
        Error::Path(_) | Error::Parse => StoreError::SecretService("message"),
        _ => StoreError::SecretService("other"),
    }
}

#[cfg(test)]
#[path = "store_linux_tests.rs"]
mod tests;
