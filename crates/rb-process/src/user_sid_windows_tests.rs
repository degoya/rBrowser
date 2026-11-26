//! The token's user SID on Windows (RB-099-17): read, its form, and the errors for a handle that
//! is no token and for a SID that is not valid. Runs natively on Windows only.

use super::*;

/// `S-1-<authority>-<sub authority>...`: decimal parts only, at least one sub authority.
fn is_sid_text(text: &str) -> bool {
    let mut parts = text.split('-');
    if parts.next() != Some("S") || parts.next() != Some("1") {
        return false;
    }
    let rest: Vec<&str> = parts.collect();
    rest.len() >= 2
        && rest
            .iter()
            .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
}

#[test]
fn the_form_check_takes_sids_only() {
    assert!(is_sid_text("S-1-5-21-1-2-3-1001"));
    assert!(is_sid_text("S-1-5-18"));
    assert!(!is_sid_text("S-1-5"));
    assert!(!is_sid_text("S-1-5-x"));
    assert!(!is_sid_text("S-1-5--1"));
    assert!(!is_sid_text("SY"));
}

/// The token's user, as `ConvertSidToStringSidW` writes it: numeric, never an SDDL alias.
#[test]
fn the_users_sid_is_numeric_text() {
    let sid = current_user_sid().expect("the token has a user");
    assert!(is_sid_text(&sid), "{sid}");
}

#[test]
fn every_read_gives_the_same_sid() {
    assert_eq!(
        current_user_sid().expect("first read"),
        current_user_sid().expect("second read")
    );
}

/// A handle of another kind (a file) is refused by the system, not read as a token.
#[test]
fn a_handle_that_is_no_token_is_an_error() {
    let file = tempfile::tempfile().expect("a temporary file");
    let error = user_sid_of(file.as_handle()).expect_err("a file has no user");
    assert!(error.raw_os_error().is_some(), "{error}");
}

/// Revision 0 and no sub authority: `ConvertSidToStringSidW` validates the SID and fails.
#[test]
#[allow(
    unsafe_code,
    reason = "sid_text is an unsafe fn: it reads the SID behind a raw pointer"
)]
fn a_sid_that_is_not_valid_is_an_error() {
    // The 8 bytes of a SID header (revision, count, authority), all zero; u32 items align it.
    let mut zeroed = [0_u32; 2];
    // SAFETY: `zeroed` is readable memory of a SID header's size, alive for the call; the
    // conversion validates the revision before it reads further.
    let result = unsafe { sid_text(zeroed.as_mut_ptr().cast()) };
    assert!(result.is_err(), "{result:?}");
}
