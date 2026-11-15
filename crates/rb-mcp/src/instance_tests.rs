use super::{FILE_VERSION, Instance, file_path, probe, read, read_all, remove_own, write};

fn instance(port: u16) -> Instance {
    Instance {
        version: FILE_VERSION,
        pid: 42,
        port,
        token: "ab".repeat(32),
        project: "/home/me/site".to_owned(),
    }
}

#[test]
fn write_then_read_gives_the_same_instance() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = file_path(dir.path(), Some("0123abcd"));
    write(&path, &instance(4100)).expect("written");
    assert_eq!(read(&path), Some(instance(4100)));
    assert!(path.ends_with("mcp/0123abcd.json"));
}

#[cfg(unix)]
#[test]
fn the_file_is_the_users_own() {
    use std::os::unix::fs::PermissionsExt as _;
    let dir = tempfile::tempdir().expect("temp dir");
    let path = file_path(dir.path(), Some("k1"));
    write(&path, &instance(4100)).expect("written");
    let mode = std::fs::metadata(&path).expect("meta").permissions().mode();
    assert_eq!(mode & 0o777, 0o600);
    let folder = std::fs::metadata(path.parent().expect("folder"))
        .expect("meta")
        .permissions()
        .mode();
    assert_eq!(folder & 0o777, 0o700);
}

#[test]
fn a_key_that_leaves_the_folder_falls_back_to_default() {
    let dir = tempfile::tempdir().expect("temp dir");
    assert!(file_path(dir.path(), Some("../evil")).ends_with("mcp/default.json"));
    assert!(file_path(dir.path(), None).ends_with("mcp/default.json"));
}

#[test]
fn malformed_files_are_ignored() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("x.json");
    std::fs::write(&path, b"{}").expect("write");
    assert_eq!(read(&path), None);
    let mut bad = instance(4100);
    bad.token = "short".to_owned();
    std::fs::write(&path, serde_json::to_vec(&bad).expect("json")).expect("write");
    assert_eq!(read(&path), None);
    let mut zero = instance(0);
    zero.port = 0;
    std::fs::write(&path, serde_json::to_vec(&zero).expect("json")).expect("write");
    assert_eq!(read(&path), None);
    std::fs::write(&path, vec![b' '; 5000]).expect("write");
    assert_eq!(read(&path), None);
}

#[test]
fn remove_own_leaves_another_servers_file() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = file_path(dir.path(), Some("k2"));
    write(&path, &instance(4100)).expect("written");
    remove_own(&path, 4999);
    assert!(path.exists());
    remove_own(&path, 4100);
    assert!(!path.exists());
}

#[test]
fn read_all_lists_the_valid_files() {
    let dir = tempfile::tempdir().expect("temp dir");
    write(&file_path(dir.path(), Some("a")), &instance(4100)).expect("written");
    write(&file_path(dir.path(), Some("b")), &instance(4200)).expect("written");
    std::fs::write(dir.path().join("mcp").join("junk.json"), b"[]").expect("write");
    let found = read_all(&dir.path().join("mcp"));
    let ports: Vec<u16> = found.iter().map(|(_, instance)| instance.port).collect();
    assert_eq!(ports, vec![4100, 4200]);
}

#[test]
fn debug_hides_the_token() {
    let shown = format!("{:?}", instance(4100));
    assert!(!shown.contains(&"ab".repeat(32)), "{shown}");
}

#[test]
fn a_dead_instance_fails_the_probe() {
    // Bind and drop a listener: nobody answers on the port afterwards.
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .and_then(|listener| listener.local_addr())
        .expect("a free port")
        .port();
    assert!(!probe(&instance(port)));
}
