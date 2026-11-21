use super::*;

/// Windows: the token's user, as `S-1-...`.
#[cfg(windows)]
#[test]
fn the_current_users_sid_is_read_as_text() {
    let sid = current_user_sid().expect("the token has a user");
    assert!(sid.starts_with("S-1-"), "{sid}");
}

/// Elsewhere there is no SID: the fallback says so instead of inventing one.
#[cfg(not(windows))]
#[test]
fn there_is_no_sid_outside_windows() {
    let error = current_user_sid().expect_err("no SID");
    assert_eq!(error.kind(), std::io::ErrorKind::Unsupported);
}
