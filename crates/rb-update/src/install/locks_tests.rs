use super::*;

#[test]
fn one_updater_at_a_time() {
    let data = tempfile::tempdir().expect("tempdir");
    assert!(!updater_running(data.path()));
    let held = UpdaterLock::acquire(data.path())
        .expect("lock")
        .expect("free");
    assert!(updater_running(data.path()));
    assert!(UpdaterLock::acquire(data.path()).expect("lock").is_none());
    drop(held);
    assert!(!updater_running(data.path()));
}

/// The updater waits for the last window; windows share the lock among themselves.
#[test]
fn the_updater_waits_until_no_window_holds_the_instance_lock() {
    let data = tempfile::tempdir().expect("tempdir");
    let first = InstanceLock::share(data.path())
        .expect("lock")
        .expect("free");
    let second = InstanceLock::share(data.path())
        .expect("lock")
        .expect("shared");
    assert!(!InstanceLock::wait_until_free(data.path(), Duration::ZERO).expect("wait"));
    drop(first);
    assert!(!InstanceLock::wait_until_free(data.path(), Duration::ZERO).expect("wait"));
    drop(second);
    assert!(InstanceLock::wait_until_free(data.path(), Duration::ZERO).expect("wait"));
    // Let go again at once: the new window takes its share.
    assert!(InstanceLock::share(data.path()).expect("lock").is_some());
}

/// RB-090-14: a window sees how many other windows run, so it never starts an updater that would
/// wait for them in vain; a window that ended without letting go (a crash) is not counted.
#[test]
fn a_window_counts_the_other_windows_and_not_itself_or_a_crashed_one() {
    let data = tempfile::tempdir().expect("tempdir");
    let first = InstanceLock::share(data.path())
        .expect("lock")
        .expect("free");
    assert_eq!(first.others(), 0);
    let second = InstanceLock::share(data.path())
        .expect("lock")
        .expect("shared");
    assert_eq!(first.others(), 1);
    assert_eq!(second.others(), 1);
    // A marker nobody holds is what a crashed window leaves: removed, not counted.
    let crashed = update_dir(data.path())
        .join(WINDOWS_DIR)
        .join("999999-0.lock");
    fs::write(&crashed, b"").expect("marker");
    assert_eq!(first.others(), 1);
    assert!(!crashed.exists());
    drop(second);
    assert_eq!(first.others(), 0);
    drop(first);
    let left = fs::read_dir(update_dir(data.path()).join(WINDOWS_DIR))
        .expect("list")
        .count();
    assert_eq!(left, 0, "each window removes its marker");
}
