use super::*;

fn origin(url: &str) -> AuthOrigin {
    AuthOrigin::parse(url).expect(url)
}

#[test]
fn the_debug_output_never_shows_the_user_or_the_password() {
    let credential = Credential::new("ada-admin", "hunter2-staging").expect("valid");
    let shown = format!("{credential:?}");
    assert!(!shown.contains("ada-admin"), "{shown}");
    assert!(!shown.contains("hunter2-staging"), "{shown}");
}

#[test]
fn a_user_with_a_colon_or_a_control_character_is_refused() {
    assert_eq!(
        Credential::new("ada:admin", "pw-1234"),
        Err(CredentialError::User)
    );
    assert_eq!(
        Credential::new("ada\nadmin", "pw-1234"),
        Err(CredentialError::User)
    );
    assert_eq!(Credential::new("", "pw-1234"), Err(CredentialError::User));
    assert_eq!(
        Credential::new("ada", "pw\r\n1234"),
        Err(CredentialError::Password)
    );
    assert_eq!(Credential::new("ada", ""), Err(CredentialError::Password));
    let long = "x".repeat(MAX_PASSWORD + 1);
    assert_eq!(
        Credential::new("ada", &long),
        Err(CredentialError::Password)
    );
    assert!(Credential::new("ada", "a password with spaces & umlauts \u{e4}").is_ok());
}

#[test]
fn the_target_names_the_project_and_the_origin() {
    let name = target_name("0123abcd", &origin("https://staging.example.test/"));
    assert_eq!(
        name,
        "rbrowser/basic-auth/0123abcd/https://staging.example.test:443"
    );
}

#[test]
fn one_projects_credential_is_not_anothers() {
    let store = MemoryStore::default();
    let staging = origin("https://staging.example.test/");
    let credential = Credential::new("ada", "pw-1234").expect("valid");
    store
        .write(&target_name("project-a", &staging), &credential)
        .expect("written");
    assert_eq!(
        store
            .read(&target_name("project-a", &staging))
            .expect("read"),
        Some(credential)
    );
    assert_eq!(
        store
            .read(&target_name("project-b", &staging))
            .expect("read"),
        None
    );
    assert_eq!(
        store
            .read(&target_name(
                "project-a",
                &origin("https://evil.example.test/")
            ))
            .expect("read"),
        None
    );
}

#[test]
fn removing_a_missing_credential_is_no_error() {
    let store = MemoryStore::default();
    let name = target_name("p", &origin("https://staging.example.test/"));
    assert_eq!(store.remove(&name), Ok(false));
    store
        .write(&name, &Credential::new("ada", "pw-1234").expect("valid"))
        .expect("written");
    assert_eq!(store.remove(&name), Ok(true));
    assert_eq!(store.read(&name), Ok(None));
}

#[cfg(not(windows))]
#[test]
fn without_a_credential_store_nothing_is_kept_anywhere() {
    let store = platform_store();
    let name = target_name("p", &origin("https://staging.example.test/"));
    let credential = Credential::new("ada", "pw-1234").expect("valid");
    assert_eq!(
        store.write(&name, &credential),
        Err(StoreError::Unsupported)
    );
    assert_eq!(store.read(&name), Ok(None));
}

/// Security review w28: `rb_diag` masks no secret shorter than `MIN_SECRET_LEN`, so such a
/// password could show in a log line; it is refused instead.
#[test]
fn a_password_too_short_to_be_masked_is_refused() {
    let min = rb_diag::redact::MIN_SECRET_LEN;
    let short = "p".repeat(min - 1);
    assert_eq!(
        Credential::new("ada", &short),
        Err(CredentialError::Password)
    );
    assert!(Credential::new("ada", &"p".repeat(min)).is_ok());
}
