use std::io::Write as _;
use std::time::Instant;

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

/// `count` numbered lines of 10 bytes each: `line 0000\n`, `line 0001\n`, ...
fn lines(from: usize, count: usize) -> String {
    (from..from + count)
        .map(|index| format!("line {index:04}\n"))
        .collect()
}

#[test]
fn cef_log_and_its_archives_are_recognised() {
    assert!(is_cef_log("cef.log"));
    assert!(is_cef_log("cef.1.log"));
    assert!(is_cef_log("cef.4.log"));
    assert!(!is_cef_log("rbrowser.log"));
    assert!(!is_cef_log("rbrowser.1.log"));
    assert!(!is_cef_log("cef.log.zip"));
    assert!(!is_cef_log("cef.01.log"));
}

#[test]
fn a_log_within_the_limit_is_left_alone() {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(dir.path().join(CEF_LOG_FILE_NAME), lines(0, 5)).expect("log");
    let trimmed = trim(dir.path(), CEF_LOG_FILE_NAME, policy(50, 3)).expect("trim");
    assert!(!trimmed);
    assert_eq!(read(dir.path(), CEF_LOG_FILE_NAME), lines(0, 5));
    assert!(!dir.path().join("cef.1.log").exists());
}

#[test]
fn a_missing_log_is_no_error() {
    let dir = tempfile::tempdir().expect("temp dir");
    let trimmed = trim(dir.path(), CEF_LOG_FILE_NAME, policy(50, 3)).expect("trim");
    assert!(!trimmed);
}

#[test]
fn a_log_past_the_limit_keeps_its_end_in_the_first_archive_and_starts_empty() {
    let dir = tempfile::tempdir().expect("temp dir");
    // 100 lines of 10 bytes; the archive takes at most 35 bytes, from a line start.
    std::fs::write(dir.path().join(CEF_LOG_FILE_NAME), lines(0, 100)).expect("log");
    let trimmed = trim(dir.path(), CEF_LOG_FILE_NAME, policy(35, 3)).expect("trim");
    assert!(trimmed);
    assert_eq!(read(dir.path(), CEF_LOG_FILE_NAME), "");
    assert_eq!(read(dir.path(), "cef.1.log"), lines(97, 3));
}

#[test]
fn older_archives_move_up_and_the_one_past_max_files_goes() {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(dir.path().join("cef.1.log"), "one\n").expect("archive 1");
    std::fs::write(dir.path().join("cef.2.log"), "two\n").expect("archive 2");
    std::fs::write(dir.path().join("cef.7.log"), "left by a larger setting\n").expect("archive 7");
    std::fs::write(dir.path().join(CEF_LOG_FILE_NAME), lines(0, 10)).expect("log");
    trim(dir.path(), CEF_LOG_FILE_NAME, policy(20, 3)).expect("trim");
    assert_eq!(read(dir.path(), "cef.1.log"), lines(8, 2));
    assert_eq!(read(dir.path(), "cef.2.log"), "one\n");
    assert!(!dir.path().join("cef.3.log").exists());
    assert!(!dir.path().join("cef.7.log").exists());
}

#[test]
fn one_file_kept_means_the_log_is_only_emptied() {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(dir.path().join("cef.1.log"), "old\n").expect("archive");
    std::fs::write(dir.path().join(CEF_LOG_FILE_NAME), lines(0, 10)).expect("log");
    trim(dir.path(), CEF_LOG_FILE_NAME, policy(20, 1)).expect("trim");
    assert_eq!(read(dir.path(), CEF_LOG_FILE_NAME), "");
    assert!(!dir.path().join("cef.1.log").exists());
}

#[test]
fn the_core_log_beside_it_is_never_touched() {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(dir.path().join("rbrowser.log"), lines(0, 10)).expect("core log");
    std::fs::write(dir.path().join("rbrowser.1.log"), "core archive\n").expect("core archive");
    std::fs::write(dir.path().join(CEF_LOG_FILE_NAME), lines(0, 10)).expect("log");
    trim(dir.path(), CEF_LOG_FILE_NAME, policy(20, 1)).expect("trim");
    assert_eq!(read(dir.path(), "rbrowser.log"), lines(0, 10));
    assert_eq!(read(dir.path(), "rbrowser.1.log"), "core archive\n");
}

/// CEF keeps its log open in append mode while the app runs: after the trim its next line lands
/// at the start of the emptied file, not past a hole where the old end was.
#[test]
fn a_writer_in_append_mode_goes_on_at_the_start_after_the_trim() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join(CEF_LOG_FILE_NAME);
    let mut writer = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .expect("writer");
    writer.write_all(lines(0, 10).as_bytes()).expect("write");
    trim(dir.path(), CEF_LOG_FILE_NAME, policy(20, 2)).expect("trim");
    writer.write_all(b"after\n").expect("write after");
    drop(writer);
    assert_eq!(read(dir.path(), CEF_LOG_FILE_NAME), "after\n");
    assert_eq!(read(dir.path(), "cef.1.log"), lines(8, 2));
}

#[test]
fn the_tail_starts_at_a_line_and_counts_what_it_leaves_out() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join(CEF_LOG_FILE_NAME);
    std::fs::write(&path, lines(0, 100)).expect("log");
    let (bytes, left_out) = tail(&path, 25).expect("tail");
    assert_eq!(String::from_utf8_lossy(&bytes), lines(98, 2));
    assert_eq!(left_out, 980);
}

#[test]
fn a_small_file_is_its_own_tail() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join(CEF_LOG_FILE_NAME);
    std::fs::write(&path, lines(0, 3)).expect("log");
    let (bytes, left_out) = tail(&path, 1_000).expect("tail");
    assert_eq!(String::from_utf8_lossy(&bytes), lines(0, 3));
    assert_eq!(left_out, 0);
}

#[test]
fn the_guard_trims_the_log_while_the_app_runs() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join(CEF_LOG_FILE_NAME);
    std::fs::write(&path, lines(0, 100)).expect("log");
    let rotation = SharedRotation::new(policy(50, 2));
    let guard = spawn_guard(
        dir.path().to_path_buf(),
        rotation,
        Duration::from_millis(10),
    )
    .expect("guard");
    // A give-up deadline, no bound on how fast it is.
    let deadline = Instant::now() + Duration::from_secs(30);
    while std::fs::metadata(&path).map_or(0, |meta| meta.len()) > 50 {
        assert!(Instant::now() < deadline, "the guard never trimmed the log");
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(read(dir.path(), "cef.1.log"), lines(95, 5));
    // The guard ends once its folder is gone: no thread outlives the test.
    drop(dir);
    let deadline = Instant::now() + Duration::from_secs(30);
    while !guard.is_finished() {
        assert!(Instant::now() < deadline, "the guard outlived its folder");
        std::thread::sleep(Duration::from_millis(10));
    }
}
