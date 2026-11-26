//! The mapping of the Secret Service's errors only: no test here opens a bus connection, so none
//! reaches the keyring of the machine running it. The D-Bus error names are `store_codec`'s
//! tests.

use super::*;

#[test]
fn nobody_serving_the_secret_service_is_no_secret_service() {
    let error = store_error(&Error::Unavailable);
    assert_eq!(error, StoreError::NoSecretService);
    assert_eq!(error.to_string(), "no secret service is running");
}

#[test]
fn a_dismissed_prompt_or_a_locked_keyring_is_denied() {
    for error in [Error::Locked, Error::Prompt] {
        assert_eq!(store_error(&error), StoreError::Denied);
    }
}

#[test]
fn the_service_s_own_error_text_never_reaches_the_store_error() {
    let path = store_error(&Error::Path("/org/pw-1234".to_owned()));
    assert_eq!(path, StoreError::SecretService("message"));
    assert!(!path.to_string().contains("pw-1234"), "{path}");
    assert_eq!(
        store_error(&Error::NoResult),
        StoreError::SecretService("no result")
    );
    assert_eq!(
        store_error(&Error::Parse),
        StoreError::SecretService("message")
    );
}
