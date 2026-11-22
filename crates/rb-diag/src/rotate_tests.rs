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

fn names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
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
    names
}

/// RB-094-28: a smaller size applies to the next write, without opening the file again.
#[test]
fn a_new_size_applies_to_the_next_write() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut file = RotatingFile::open(dir.path(), "rbrowser.log", policy(1000, 3)).expect("open");
    let line = format!("{}\n", "a".repeat(39));
    for _ in 0..3 {
        file.write_all(line.as_bytes()).expect("write");
    }
    assert!(!dir.path().join("rbrowser.1.log").exists());
    file.rotation().set(policy(100, 3));
    file.write_all(line.as_bytes()).expect("write");
    assert_eq!(read(dir.path(), "rbrowser.1.log"), line.repeat(3));
    assert_eq!(read(dir.path(), "rbrowser.log"), line);
}

/// RB-094-28: fewer files kept clears the archives past the new number at the next rotation -
/// not at once, and never the file being written.
#[test]
fn fewer_files_clear_the_older_archives_at_the_next_rotation() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut file = RotatingFile::open(dir.path(), "rbrowser.log", policy(10, 5)).expect("open");
    for index in 0..5 {
        writeln!(file, "line {index:03}").expect("write");
    }
    assert_eq!(
        names(dir.path()),
        [
            "rbrowser.1.log",
            "rbrowser.2.log",
            "rbrowser.3.log",
            "rbrowser.4.log",
            "rbrowser.log"
        ]
    );
    file.rotation().set(policy(10, 2));
    assert_eq!(
        names(dir.path()).len(),
        5,
        "nothing is deleted before the next rotation"
    );
    writeln!(file, "line 005").expect("write");
    assert_eq!(names(dir.path()), ["rbrowser.1.log", "rbrowser.log"]);
    assert_eq!(read(dir.path(), "rbrowser.log"), "line 005\n");
    assert_eq!(read(dir.path(), "rbrowser.1.log"), "line 004\n");
}

/// RB-094-28: one file only clears every archive and keeps writing the open file.
#[test]
fn one_file_after_more_clears_every_archive() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut file = RotatingFile::open(dir.path(), "rbrowser.log", policy(10, 3)).expect("open");
    for index in 0..3 {
        writeln!(file, "line {index:03}").expect("write");
    }
    file.rotation().set(policy(10, 1));
    writeln!(file, "line 003").expect("write");
    assert_eq!(names(dir.path()), ["rbrowser.log"]);
    assert_eq!(read(dir.path(), "rbrowser.log"), "line 003\n");
}

/// Review of wave 097-w33: the archives past the number kept go also beyond a gap in their
/// numbers (a file deleted by hand); other files of the folder stay.
#[test]
fn archives_past_a_gap_are_cleared_too() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut file = RotatingFile::open(dir.path(), "rbrowser.log", policy(10, 2)).expect("open");
    for name in [
        "rbrowser.3.log",
        "rbrowser.7.log",
        "rbrowser.x.log",
        "rbrowser.07.log",
        "other.5.log",
    ] {
        std::fs::write(dir.path().join(name), name).expect("write");
    }
    writeln!(file, "line 000").expect("write");
    writeln!(file, "line 001").expect("write");
    assert_eq!(
        names(dir.path()),
        [
            "other.5.log",
            "rbrowser.07.log",
            "rbrowser.1.log",
            "rbrowser.log",
            "rbrowser.x.log"
        ]
    );
    assert_eq!(read(dir.path(), "rbrowser.1.log"), "line 000\n");
}

#[test]
fn a_shared_policy_keeps_its_floors() {
    let shared = SharedRotation::new(policy(0, 0));
    assert_eq!(shared.get(), policy(1, 1));
    shared.set(policy(2048, 4));
    assert_eq!(shared.get(), policy(2048, 4));
    shared.set(policy(0, 0));
    assert_eq!(shared.get(), policy(1, 1));
}
