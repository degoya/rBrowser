use super::*;

fn entry(at: u64) -> Entry {
    Entry {
        at,
        id: Some("users/list.http".into()),
        method: "GET".into(),
        url: "{{baseUrl}}/users".into(),
        environment: Some("staging".into()),
        status: Some(200),
        total_ms: Some(12.5),
        error: None,
    }
}

#[test]
fn the_newest_send_comes_first_and_the_size_holds() {
    let dir = tempfile::tempdir().expect("temp dir");
    for at in 1..=4 {
        append(dir.path(), entry(at), 3).expect("written");
    }
    let entries = read(dir.path());
    assert_eq!(
        entries.iter().map(|entry| entry.at).collect::<Vec<_>>(),
        [4, 3, 2]
    );
    assert_eq!(entries[0], entry(4));
}

#[test]
fn a_size_of_zero_keeps_nothing() {
    let dir = tempfile::tempdir().expect("temp dir");
    append(dir.path(), entry(1), 5).expect("written");
    append(dir.path(), entry(2), 0).expect("removed");
    assert!(read(dir.path()).is_empty());
    append(dir.path(), entry(3), 0).expect("nothing to remove");
}

#[test]
fn a_broken_file_reads_as_none() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join(HISTORY_FILE);
    fs::create_dir_all(path.parent().expect("parent")).expect("dir");
    fs::write(&path, "not json").expect("written");
    assert!(read(dir.path()).is_empty());
    append(dir.path(), entry(1), 2).expect("written over");
    assert_eq!(read(dir.path()).len(), 1);
}
