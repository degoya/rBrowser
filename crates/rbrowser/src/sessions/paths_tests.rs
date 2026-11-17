use std::path::Path;

use super::*;

#[test]
fn default_uses_the_global_context() {
    let profile = Path::new("C:/rb/profiles/shop-1a2b");
    assert_eq!(cache_path(profile, "default"), Ok(None));
}

#[test]
fn a_named_session_lives_under_the_profile() {
    let profile = Path::new("C:/rb/profiles/shop-1a2b");
    let path = cache_path(profile, "admin")
        .expect("valid")
        .expect("a folder of its own");
    assert_eq!(path, profile.join("sessions").join("admin"));
    assert!(path.starts_with(profile));
}

#[test]
fn a_session_never_shares_the_default_cache() {
    let profile = Path::new("/p");
    let admin = cache_path(profile, "admin").expect("valid").expect("own");
    assert_ne!(admin, profile.join(DEFAULT_CACHE_DIR));
}

#[test]
fn ids_that_leave_the_profile_are_refused() {
    let profile = Path::new("/p");
    for id in [
        "..", "../x", "a/b", "a\\b", "/etc", "C:\\x", "Admin", "con", "",
    ] {
        assert_eq!(
            cache_path(profile, id),
            Err(SessionError::InvalidId(id.to_owned())),
            "{id:?}"
        );
    }
}
