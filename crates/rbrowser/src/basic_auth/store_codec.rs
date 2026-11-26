//! What the macOS Keychain and the Linux Secret Service keep of a Basic auth credential
//! (RB-099-05), in code that compiles everywhere so its tests run on every platform; the calls
//! themselves are `store_macos` and `store_linux`.
//!
//! - One entry per target: the service [`SERVICE`] and the account `<project key>/<origin>`, the
//!   target name without the service (a Keychain generic password's service and account, the
//!   Secret Service attributes `service` and `account`).
//! - The secret is `user:password`, the form Basic auth sends: a user holds no `:` (RFC 7617,
//!   `Credential::new`), so the first `:` splits it, and both stay inside the encrypted part.
//! - The platforms' errors become a [`StoreError`] that names no value: a status code, or what
//!   failed.

use super::{Credential, SERVICE, StoreError};

/// The account of `target` (`rbrowser/basic-auth/<project key>/<origin>`): `<project key>/<origin>`.
/// A name of another shape is kept whole.
#[cfg(any(test, target_os = "macos", target_os = "linux"))]
#[must_use]
pub fn account(target: &str) -> &str {
    target
        .strip_prefix(SERVICE)
        .and_then(|rest| rest.strip_prefix('/'))
        .unwrap_or(target)
}

/// The Secret Service attributes that find the entry of `target`; stored in clear by the
/// service, so they carry no secret.
#[cfg(any(test, target_os = "linux"))]
#[must_use]
pub fn attributes(target: &str) -> [(&'static str, &str); 2] {
    [("service", SERVICE), ("account", account(target))]
}

/// The label a keyring manager (Seahorse, KWalletManager) shows for the entry of `target`.
#[cfg(any(test, target_os = "linux"))]
#[must_use]
pub fn label(target: &str) -> String {
    format!("rBrowser Basic auth: {}", account(target))
}

/// The secret of `credential`: `user:password`. The caller zeroes it once handed over.
#[cfg(any(test, target_os = "macos", target_os = "linux"))]
#[must_use]
pub fn secret_of(credential: &Credential) -> Vec<u8> {
    let mut secret = Vec::with_capacity(credential.user().len() + 1 + credential.password().len());
    secret.extend_from_slice(credential.user().as_bytes());
    secret.push(b':');
    secret.extend_from_slice(credential.password().as_bytes());
    secret
}

/// The credential in `secret`; `None` for anything [`secret_of`] does not write.
#[cfg(any(test, target_os = "macos", target_os = "linux"))]
#[must_use]
pub fn credential_of(secret: &[u8]) -> Option<Credential> {
    let (user, password) = std::str::from_utf8(secret).ok()?.split_once(':')?;
    Credential::new(user, password).ok()
}

/// `errSecItemNotFound`: the Keychain has no such item.
#[cfg(any(test, target_os = "macos"))]
pub const ERR_SEC_ITEM_NOT_FOUND: i32 = -25300;

/// The error of the Keychain's `status`; `None` for a missing item, which is no error.
#[cfg(any(test, target_os = "macos"))]
#[must_use]
pub fn keychain_error(status: i32) -> Option<StoreError> {
    match status {
        ERR_SEC_ITEM_NOT_FOUND => None,
        // errSecUserCanceled, errSecAuthFailed, errSecInteractionNotAllowed: the person said no,
        // or the Keychain is locked and may not ask.
        -128 | -25293 | -25308 => Some(StoreError::Denied),
        status => Some(StoreError::Keychain(status)),
    }
}

/// The error of a D-Bus error named `name` (libdbus's own while connecting, the bus's or the
/// service's after); `""` for an error without a name.
#[cfg(any(test, target_os = "linux"))]
#[must_use]
pub fn dbus_error(name: &str) -> StoreError {
    const DBUS: &str = "org.freedesktop.DBus.Error.";
    let Some(short) = name.strip_prefix(DBUS) else {
        return match name {
            "org.freedesktop.Secret.Error.IsLocked" => StoreError::Denied,
            _ => StoreError::SecretService("dbus"),
        };
    };
    match short {
        // No session bus: no address (libdbus cannot autolaunch one without X11), no socket, or
        // nobody listening on it - a server, WSL without D-Bus.
        "NotSupported" | "BadAddress" | "FileNotFound" | "NoServer" => StoreError::NoSecretService,
        // A bus, but nobody owns `org.freedesktop.secrets` and the bus cannot start anybody.
        "ServiceUnknown" | "NameHasNoOwner" => StoreError::NoSecretService,
        spawn if spawn.starts_with("Spawn.") => StoreError::NoSecretService,
        "AccessDenied" | "AuthFailed" => StoreError::Denied,
        "Timeout" | "TimedOut" | "NoReply" => StoreError::SecretService("no answer"),
        "Disconnected" => StoreError::SecretService("disconnected"),
        _ => StoreError::SecretService("dbus"),
    }
}

#[cfg(test)]
#[path = "store_codec_tests.rs"]
mod tests;
