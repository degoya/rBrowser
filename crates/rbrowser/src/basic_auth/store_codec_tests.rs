use super::*;
use crate::basic_auth::origin::AuthOrigin;
use crate::basic_auth::store::target_name;

fn target() -> String {
    let origin = AuthOrigin::parse("https://staging.example.test/").expect("origin");
    target_name("0123abcd", &origin)
}

#[test]
fn the_account_is_the_target_without_the_service() {
    assert_eq!(
        account(&target()),
        "0123abcd/https://staging.example.test:443"
    );
    assert_eq!(SERVICE, "rbrowser/basic-auth");
    // A name of another shape stays whole rather than losing a part.
    assert_eq!(account("elsewhere/x"), "elsewhere/x");
}

#[test]
fn the_secret_service_attributes_are_the_service_and_the_account() {
    assert_eq!(
        attributes(&target()),
        [
            ("service", "rbrowser/basic-auth"),
            ("account", "0123abcd/https://staging.example.test:443"),
        ]
    );
}

#[test]
fn the_label_names_the_account_and_never_the_credential() {
    let label = label(&target());
    assert_eq!(
        label,
        "rBrowser Basic auth: 0123abcd/https://staging.example.test:443"
    );
}

#[test]
fn a_secret_carries_the_user_and_the_password_and_reads_back() {
    let credential = Credential::new("ada", "pw:with:colons").expect("valid");
    let secret = secret_of(&credential);
    assert_eq!(secret, b"ada:pw:with:colons");
    assert_eq!(credential_of(&secret), Some(credential));
}

#[test]
fn a_secret_that_is_no_credential_reads_as_none() {
    assert_eq!(credential_of(b"no-colon-at-all"), None);
    assert_eq!(credential_of(b":pw-1234"), None);
    assert_eq!(credential_of(b"ada:"), None);
    assert_eq!(credential_of(b"ada:pw\n1234"), None);
    assert_eq!(credential_of(&[0x61, 0x3a, 0xff, 0xfe, 0x61, 0x61]), None);
}

#[test]
fn a_missing_keychain_item_is_no_error() {
    assert_eq!(keychain_error(ERR_SEC_ITEM_NOT_FOUND), None);
}

#[test]
fn a_refused_or_cancelled_keychain_access_is_denied() {
    for status in [-128, -25293, -25308] {
        assert_eq!(keychain_error(status), Some(StoreError::Denied), "{status}");
    }
}

#[test]
fn any_other_keychain_status_is_named_by_its_code_only() {
    let error = keychain_error(-25299).expect("an error");
    assert_eq!(error, StoreError::Keychain(-25299));
    assert_eq!(
        error.to_string(),
        "the macOS Keychain failed (status -25299)"
    );
}

#[test]
fn no_session_bus_or_no_secret_service_on_it_says_so() {
    for name in [
        // libdbus without an address, with a bad one, no socket, nobody listening.
        "org.freedesktop.DBus.Error.NotSupported",
        "org.freedesktop.DBus.Error.BadAddress",
        "org.freedesktop.DBus.Error.FileNotFound",
        "org.freedesktop.DBus.Error.NoServer",
        // A bus without a Secret Service.
        "org.freedesktop.DBus.Error.ServiceUnknown",
        "org.freedesktop.DBus.Error.NameHasNoOwner",
        "org.freedesktop.DBus.Error.Spawn.ServiceNotFound",
        "org.freedesktop.DBus.Error.Spawn.ChildExited",
    ] {
        let error = dbus_error(name);
        assert_eq!(error, StoreError::NoSecretService, "{name}");
        assert_eq!(error.to_string(), "no secret service is running");
    }
}

#[test]
fn a_locked_collection_or_a_refusal_is_denied() {
    for name in [
        "org.freedesktop.Secret.Error.IsLocked",
        "org.freedesktop.DBus.Error.AccessDenied",
        "org.freedesktop.DBus.Error.AuthFailed",
    ] {
        assert_eq!(dbus_error(name), StoreError::Denied, "{name}");
    }
}

#[test]
fn a_bus_that_does_not_answer_is_named_so() {
    for name in [
        "org.freedesktop.DBus.Error.Timeout",
        "org.freedesktop.DBus.Error.NoReply",
    ] {
        assert_eq!(dbus_error(name), StoreError::SecretService("no answer"));
    }
    assert_eq!(
        dbus_error("org.freedesktop.DBus.Error.Disconnected"),
        StoreError::SecretService("disconnected")
    );
}

#[test]
fn any_other_dbus_error_names_no_value() {
    for name in [
        "org.example.Error.Whatever",
        "org.freedesktop.DBus.Error.Failed",
        "",
    ] {
        let error = dbus_error(name);
        assert_eq!(error, StoreError::SecretService("dbus"), "{name}");
        assert_eq!(error.to_string(), "the secret service failed (dbus)");
    }
}

#[test]
fn no_store_error_shows_a_value() {
    let credential = Credential::new("ada-admin", "hunter2-staging").expect("valid");
    let shown = [
        StoreError::Unsupported,
        StoreError::Failed(5),
        StoreError::Keychain(-25299),
        StoreError::NoSecretService,
        StoreError::NoCollection,
        StoreError::Denied,
        StoreError::SecretService("dbus"),
    ]
    .iter()
    .map(|error| format!("{error} {error:?}"))
    .collect::<String>();
    assert!(!shown.contains(credential.user()), "{shown}");
    assert!(!shown.contains(credential.password()), "{shown}");
}
