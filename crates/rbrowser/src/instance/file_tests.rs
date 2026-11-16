use super::*;

fn sample(dir: &Path, root: &Path) -> InstanceFile {
    InstanceFile {
        version: FILE_VERSION,
        pid: 4242,
        endpoint: channel::endpoint_for(&instances_dir(dir), &key(root)),
        nonce: crate::instance::protocol::new_nonce().expect("random"),
        project: root.to_string_lossy().into_owned(),
    }
}

#[test]
fn each_project_has_its_own_file_and_case_does_not_matter() {
    let local = Path::new("/data/rBrowser");
    let a = file_path(local, Path::new("C:\\Work\\Site"));
    assert_eq!(a, file_path(local, Path::new("c:\\work\\site\\")));
    assert_ne!(a, file_path(local, Path::new("C:\\Work\\Other")));
    assert_eq!(a.parent(), Some(instances_dir(local).as_path()));
    assert!(a.to_string_lossy().ends_with(".json"));
    assert!(same_project("c:/work/site", Path::new("C:\\Work\\Site")));
    assert!(!same_project(
        "C:\\Work\\Other",
        Path::new("C:\\Work\\Site")
    ));
}

#[test]
fn the_file_is_written_and_read_back() {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path().join("site");
    let path = file_path(dir.path(), &root);
    let written = sample(dir.path(), &root);
    write(&path, &written).expect("writes");
    assert_eq!(read(&path, dir.path()), Some(written));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = std::fs::metadata(&path)
            .expect("exists")
            .permissions()
            .mode();
        assert_eq!(mode & 0o077, 0, "only the user reads the file");
    }
}

#[test]
fn a_missing_large_or_foreign_file_is_no_instance() {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path().join("site");
    let path = file_path(dir.path(), &root);
    assert_eq!(read(&path, dir.path()), None);
    std::fs::create_dir_all(instances_dir(dir.path())).expect("dir");
    std::fs::write(&path, vec![b' '; 8 * 1024]).expect("writes");
    assert_eq!(read(&path, dir.path()), None);
    std::fs::write(&path, "{\"version\":1}").expect("writes");
    assert_eq!(read(&path, dir.path()), None);

    let mut bad = sample(dir.path(), &root);
    bad.nonce = "short".to_owned();
    write(&path, &bad).expect("writes");
    assert_eq!(read(&path, dir.path()), None);

    let mut elsewhere = sample(dir.path(), &root);
    elsewhere.endpoint = "/tmp/other.sock".to_owned();
    write(&path, &elsewhere).expect("writes");
    assert_eq!(read(&path, dir.path()), None);

    let mut newer = sample(dir.path(), &root);
    newer.version = FILE_VERSION + 1;
    write(&path, &newer).expect("writes");
    assert_eq!(read(&path, dir.path()), None);
}

#[test]
fn the_nonce_never_appears_in_debug_output() {
    let dir = tempfile::tempdir().expect("temp dir");
    let file = sample(dir.path(), &dir.path().join("site"));
    assert!(!format!("{file:?}").contains(&file.nonce));
}
