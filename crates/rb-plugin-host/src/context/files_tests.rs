//! The temporary file of `replace` (security review RB-110-13, finding 3).

use super::Temporary;

#[test]
fn a_temporary_file_dropped_before_its_rename_is_removed() {
    let folder = tempfile::tempdir().expect("tempdir");
    let path = folder.path().join(super::temporary_name(&[0; 8]));
    std::fs::write(&path, b"half").expect("write");
    drop(Temporary(Some(path.clone())));
    assert!(!path.exists());
}

#[test]
fn a_renamed_temporary_file_is_left_alone() {
    let folder = tempfile::tempdir().expect("tempdir");
    let path = folder.path().join("TODO.todo");
    std::fs::write(&path, b"new").expect("write");
    Temporary(Some(path.clone())).renamed();
    assert_eq!(std::fs::read(&path).expect("read"), b"new");
}

#[test]
fn a_temporary_name_is_short_hidden_and_no_todo_file() {
    let name = super::temporary_name(&[0xab; 8]);
    assert_eq!(name, ".abababababababab.rbrowser-tmp");
    // No write glob (`*.todo`, `*.taskpaper`) names it, whatever file it replaces.
    assert!(!name.ends_with(".todo") && !name.ends_with(".taskpaper"));
}
