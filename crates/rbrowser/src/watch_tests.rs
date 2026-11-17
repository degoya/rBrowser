use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use super::*;

#[test]
fn only_the_config_files_count() {
    assert!(is_config_file(Path::new("/p/.rbrowser/config.json")));
    assert!(is_config_file(Path::new(
        "C:/p/.rbrowser/config.local.json"
    )));
    assert!(!is_config_file(Path::new(
        "/p/.rbrowser/config.json.bak-v1"
    )));
    assert!(!is_config_file(Path::new("/p/.rbrowser/notes.md")));
}

#[test]
fn a_burst_of_writes_reloads_once() {
    let root = tempfile::tempdir().expect("temp dir");
    let folder = root.path().join(CONFIG_DIR);
    std::fs::create_dir_all(&folder).expect("config dir");
    let calls = Arc::new(AtomicUsize::new(0));
    let counted = calls.clone();
    let _watch = watch_config(root.path(), move || {
        counted.fetch_add(1, Ordering::SeqCst);
    })
    .expect("watches");
    for columns in 1..=3 {
        std::fs::write(
            folder.join(CONFIG_FILE),
            format!("{{\"version\":1,\"layout\":{{\"columns\":{columns}}}}}"),
        )
        .expect("writes");
    }
    std::fs::write(folder.join("unrelated.txt"), "x").expect("writes");
    let deadline = Instant::now() + Duration::from_secs(5);
    while calls.load(Ordering::SeqCst) == 0 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    std::thread::sleep(DEBOUNCE * 2);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}
