use super::*;

fn policy(max_bytes: u64, max_files: usize) -> RotationPolicy {
    RotationPolicy {
        max_bytes,
        max_files,
    }
}

fn read(dir: &Path, name: &str) -> String {
    std::fs::read_to_string(dir.join(name)).unwrap_or_default()
}

#[test]
fn the_first_write_creates_the_log_file() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut file = RotatingFile::open(dir.path(), "rbrowser.log", policy(1024, 3)).expect("open");
    file.write_all(b"hello\n").expect("write");
    assert_eq!(read(dir.path(), "rbrowser.log"), "hello\n");
}

#[test]
fn a_write_past_the_limit_rotates_the_file() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut file = RotatingFile::open(dir.path(), "rbrowser.log", policy(100, 3)).expect("open");
    let line = format!("{}\n", "a".repeat(39));
    for _ in 0..3 {
        file.write_all(line.as_bytes()).expect("write");
    }
    assert_eq!(read(dir.path(), "rbrowser.log"), line);
    assert_eq!(read(dir.path(), "rbrowser.1.log"), line.repeat(2));
}

#[test]
fn rotation_keeps_at_most_max_files() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut file = RotatingFile::open(dir.path(), "rbrowser.log", policy(10, 3)).expect("open");
    for index in 0..10 {
        writeln!(file, "line {index:03}").expect("write");
    }
    let mut names: Vec<String> = std::fs::read_dir(dir.path())
        .expect("list")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    assert_eq!(names, ["rbrowser.1.log", "rbrowser.2.log", "rbrowser.log"]);
    assert_eq!(read(dir.path(), "rbrowser.log"), "line 009\n");
    assert_eq!(read(dir.path(), "rbrowser.1.log"), "line 008\n");
    assert_eq!(read(dir.path(), "rbrowser.2.log"), "line 007\n");
}

#[test]
fn one_file_only_truncates_instead_of_keeping_an_archive() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut file = RotatingFile::open(dir.path(), "rbrowser.log", policy(10, 1)).expect("open");
    writeln!(file, "first line").expect("write");
    writeln!(file, "second").expect("write");
    assert_eq!(read(dir.path(), "rbrowser.log"), "second\n");
    assert!(!dir.path().join("rbrowser.1.log").exists());
}

#[test]
fn an_existing_file_counts_towards_the_limit() {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(dir.path().join("rbrowser.log"), "x".repeat(90)).expect("seed");
    let mut file = RotatingFile::open(dir.path(), "rbrowser.log", policy(100, 2)).expect("open");
    file.write_all(&[b'y'; 20]).expect("write");
    assert_eq!(read(dir.path(), "rbrowser.1.log"), "x".repeat(90));
    assert_eq!(read(dir.path(), "rbrowser.log"), "y".repeat(20));
}

#[test]
fn a_single_write_larger_than_the_limit_is_kept_whole() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut file = RotatingFile::open(dir.path(), "rbrowser.log", policy(5, 2)).expect("open");
    file.write_all(b"0123456789\n").expect("write");
    assert_eq!(read(dir.path(), "rbrowser.log"), "0123456789\n");
}

#[test]
fn rotated_names_put_the_number_before_the_extension() {
    assert_eq!(rotated_name("rbrowser.log", 2), "rbrowser.2.log");
    assert_eq!(rotated_name("cef", 1), "cef.1");
}

#[test]
fn the_policy_from_settings_has_sane_floors() {
    let from = RotationPolicy::from_settings(0, 0);
    assert_eq!(from.max_files, 1);
    assert_eq!(from.max_bytes, 1024 * 1024);
    assert_eq!(
        RotationPolicy::from_settings(10, 5).max_bytes,
        10 * 1024 * 1024
    );
}
