use super::{create_private_dir, user_only_sddl};

/// Protected (nothing inherited from the parent), one allow ACE with full file access for the
/// user, inherited by every file and folder inside.
#[test]
fn the_windows_descriptor_grants_the_user_alone() {
    assert_eq!(
        user_only_sddl("S-1-5-21-1-2-3-1001"),
        "D:P(A;OICI;FA;;;S-1-5-21-1-2-3-1001)"
    );
}

#[test]
fn the_folder_and_its_parents_are_created() {
    let root = tempfile::tempdir().expect("a temporary folder");
    let dir = root.path().join("data").join("mcp");
    create_private_dir(&dir).expect("created");
    assert!(dir.is_dir());
    // A second call on the existing folder is fine.
    create_private_dir(&dir).expect("again");
}

/// A folder another account could open before is the user's alone afterwards (security review
/// RB-090-06, finding 3).
#[cfg(unix)]
#[test]
fn on_unix_the_folder_is_the_users_alone() {
    use std::os::unix::fs::PermissionsExt as _;
    let root = tempfile::tempdir().expect("a temporary folder");
    let dir = root.path().join("cache");
    std::fs::create_dir(&dir).expect("created");
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o777)).expect("opened");
    create_private_dir(&dir).expect("restricted");
    let mode = std::fs::metadata(&dir)
        .expect("metadata")
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o700);
}
