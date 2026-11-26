//! The private folder's DACL on Windows (RB-099-17), read back after the system applied it: one
//! allow entry for the token's user, protected from the parent, inherited by what the folder
//! holds. Runs natively on Windows only.

use windows_sys::Win32::Security::{CONTAINER_INHERIT_ACE, INHERITED_ACE, OBJECT_INHERIT_ACE};

use super::*;
use crate::dacl::{self, ACCESS_ALLOWED, Dacl};

/// `FILE_ALL_ACCESS` of `winnt.h`, `FA` in the SDDL of [`crate::private_dir::user_only_sddl`].
const FILE_ALL_ACCESS: u32 = 0x001F_01FF;

fn own_sid() -> String {
    crate::current_user_sid().expect("the token has a user")
}

/// Exactly one entry: full file access for the token's user, nobody else.
fn assert_user_alone(read: &Dacl) {
    assert_eq!(read.aces.len(), 1, "{read:?}");
    let ace = &read.aces[0];
    assert_eq!(ace.kind, ACCESS_ALLOWED, "{read:?}");
    assert_eq!(ace.sid, own_sid(), "{read:?}");
    assert_eq!(ace.mask, FILE_ALL_ACCESS, "{read:?}");
}

/// The temporary folder's own entries (SYSTEM, Administrators, ...) reach a folder below it by
/// inheritance; after the restriction none is left: one explicit entry, inherited by objects and
/// containers inside, and the DACL protected from the parent.
#[test]
fn the_folder_holds_one_protected_entry_for_the_user() {
    let root = tempfile::tempdir().expect("a temporary folder");
    let dir = root.path().join("private");
    crate::create_private_dir(&dir).expect("created and restricted");
    let read = dacl::of_path(&dir).expect("the DACL reads back");
    assert!(read.protected, "{read:?}");
    assert_user_alone(&read);
    let flags = u32::from(read.aces[0].flags);
    let inherit = OBJECT_INHERIT_ACE | CONTAINER_INHERIT_ACE;
    assert_eq!(flags & inherit, inherit, "{read:?}");
    assert_eq!(flags & INHERITED_ACE, 0, "an explicit entry: {read:?}");
    // A second restriction of the same folder leaves the same DACL.
    crate::create_private_dir(&dir).expect("restricted again");
    assert_eq!(dacl::of_path(&dir).expect("read again"), read);
}

/// A file and a folder made inside afterwards (the instance file, a cache folder) carry the one
/// entry, inherited.
#[test]
fn what_is_made_inside_inherits_the_one_entry() {
    let root = tempfile::tempdir().expect("a temporary folder");
    let dir = root.path().join("private");
    crate::create_private_dir(&dir).expect("created and restricted");
    let file = dir.join("instance.json");
    std::fs::write(&file, b"{}").expect("a file inside");
    let folder = dir.join("cache");
    std::fs::create_dir(&folder).expect("a folder inside");
    for path in [&file, &folder] {
        let read = dacl::of_path(path).expect("the DACL reads back");
        assert_user_alone(&read);
        assert_ne!(
            u32::from(read.aces[0].flags) & INHERITED_ACE,
            0,
            "{path:?}: {read:?}"
        );
    }
}

/// What the folder held before it was restricted gets the one entry too: the parent's entries
/// it inherited are replaced (security review RB-090-06, finding 3).
#[test]
fn what_the_folder_held_before_is_restricted_too() {
    let root = tempfile::tempdir().expect("a temporary folder");
    let dir = root.path().join("data");
    std::fs::create_dir(&dir).expect("an open folder");
    let file = dir.join("old.json");
    std::fs::write(&file, b"{}").expect("a file in it");
    crate::create_private_dir(&dir).expect("restricted");
    let read = dacl::of_path(&file).expect("the DACL reads back");
    assert_user_alone(&read);
    assert_ne!(u32::from(read.aces[0].flags) & INHERITED_ACE, 0, "{read:?}");
}

#[test]
fn a_missing_folder_is_an_error() {
    let root = tempfile::tempdir().expect("a temporary folder");
    let error = restrict(&root.path().join("missing")).expect_err("nothing to restrict");
    assert_eq!(error.kind(), io::ErrorKind::NotFound, "{error}");
}

#[test]
fn a_broken_descriptor_is_an_error() {
    assert!(Descriptor::from_sddl("D:P(A;;NOPE;;;nobody)").is_err());
    assert!(Descriptor::from_sddl(&crate::private_dir::user_only_sddl(&own_sid())).is_ok());
}
