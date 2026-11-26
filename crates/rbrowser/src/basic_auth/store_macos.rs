//! The macOS Keychain as the store of Basic auth credentials (RB-099-05): one generic password
//! per target in the person's login keychain, service `rbrowser/basic-auth`, account
//! `<project key>/<origin>`, its data `user:password` (`store_codec`). The Keychain encrypts it
//! and asks the person before another program - or a differently signed rBrowser - reads it;
//! refusing is [`StoreError::Denied`]. The copy of the secret is zeroed as soon as it was
//! handed over or read.

use security_framework::passwords::{
    PasswordOptions, delete_generic_password, generic_password, set_generic_password,
};

use super::codec;
use super::{Credential, CredentialStore, SERVICE, StoreError};

/// The login keychain of the signed-in user.
pub struct KeychainStore;

/// The error of `error`; `None` for a missing item.
fn keychain_error(error: &security_framework::base::Error) -> Option<StoreError> {
    codec::keychain_error(error.code())
}

impl CredentialStore for KeychainStore {
    fn read(&self, target: &str) -> Result<Option<Credential>, StoreError> {
        let options = PasswordOptions::new_generic_password(SERVICE, codec::account(target));
        match generic_password(options) {
            Ok(mut secret) => {
                let credential = codec::credential_of(&secret);
                secret.fill(0);
                Ok(credential)
            }
            Err(error) => keychain_error(&error).map_or(Ok(None), Err),
        }
    }

    fn write(&self, target: &str, credential: &Credential) -> Result<(), StoreError> {
        let mut secret = codec::secret_of(credential);
        // Creates the item, or updates the one of this service and account.
        let written = set_generic_password(SERVICE, codec::account(target), &secret);
        // The copy of the password goes before an error can be returned.
        secret.fill(0);
        written
            .map_err(|error| keychain_error(&error).unwrap_or(StoreError::Keychain(error.code())))
    }

    fn remove(&self, target: &str) -> Result<bool, StoreError> {
        match delete_generic_password(SERVICE, codec::account(target)) {
            Ok(()) => Ok(true),
            Err(error) => keychain_error(&error).map_or(Ok(false), Err),
        }
    }
}
