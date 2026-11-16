use std::path::PathBuf;
use std::time::{Duration, Instant};

use super::*;

const KEY: &str = r"IO error for operation on \\wsl.localhost\d\p\worker.js: not found";

fn paths() -> Vec<PathBuf> {
    vec![PathBuf::from(r"\\wsl.localhost\d\p\worker.js")]
}

#[test]
fn an_error_is_new_once_and_quiet_while_it_repeats() {
    let mut errors = WatchErrors::default();
    let start = Instant::now();
    assert!(errors.on_error(KEY, &paths(), start));
    for pass in 1..100 {
        assert!(!errors.on_error(KEY, &paths(), start + Duration::from_secs(pass)));
    }
}

#[test]
fn another_error_is_new_too() {
    let mut errors = WatchErrors::default();
    let now = Instant::now();
    assert!(errors.on_error(KEY, &paths(), now));
    assert!(errors.on_error("IO error for operation on b: denied", &[], now));
    assert!(!errors.on_error(KEY, &paths(), now));
}

#[test]
fn an_error_that_stopped_is_logged_again_when_it_comes_back() {
    let mut errors = WatchErrors::default();
    let start = Instant::now();
    assert!(errors.on_error(KEY, &paths(), start));
    let later = start + FORGET_AFTER + Duration::from_secs(1);
    assert!(errors.on_error(KEY, &paths(), later));
}

#[test]
fn an_event_about_a_failing_path_ends_its_error() {
    let mut errors = WatchErrors::default();
    let now = Instant::now();
    assert!(errors.on_error(KEY, &paths(), now));
    assert!(errors.on_event(&[PathBuf::from("elsewhere")]).is_empty());
    assert_eq!(errors.on_event(&paths()), vec![KEY.to_owned()]);
    // It recovered: failing again is news.
    assert!(errors.on_error(KEY, &paths(), now));
}

/// The owner's project: two dangling links under public/media fail on every poll, in turn. Each
/// is logged once (its text names its path), not once per pass.
#[test]
fn two_failing_links_are_each_new_once_over_many_polls() {
    let first = PathBuf::from(r"\\wsl.localhost\d\p\public\media\a.mp4");
    let second = PathBuf::from(r"\\wsl.localhost\d\p\public\media\b.mp4");
    let key = |path: &PathBuf| format!("No such file or directory (os error 2) about [{path:?}]");
    let mut errors = WatchErrors::default();
    let start = Instant::now();
    let mut news = 0;
    for pass in 0..100u64 {
        let now = start + Duration::from_millis(pass * 2000);
        for path in [&first, &second] {
            if errors.on_error(&key(path), std::slice::from_ref(path), now) {
                news += 1;
            }
        }
    }
    assert_eq!(news, 2);
}
