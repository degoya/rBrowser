use super::*;

fn ledger() -> (tempfile::TempDir, PathBuf, Ledger) {
    let dir = tempfile::tempdir().expect("tempdir");
    let folder = dir.path().join("sub").join(FOLDER_NAME);
    let ledger = Ledger::at(Some(folder.clone()));
    (dir, folder, ledger)
}

fn own_file(folder: &Path) -> PathBuf {
    folder.join(format!("{}.json", std::process::id()))
}

fn other_file(folder: &Path, pid: u32, entries: &[(&str, u16)]) {
    std::fs::create_dir_all(folder).expect("folder");
    let reverses: Vec<Value> = entries
        .iter()
        .map(|(device, port)| serde_json::json!({ "device": device, "port": port }))
        .collect();
    let text = serde_json::json!({ "version": 1, "reverses": reverses }).to_string();
    std::fs::write(folder.join(format!("{pid}.json")), text).expect("written");
}

/// The file names a device only by a hash of its serial, never the serial itself; each process
/// writes a file of its own (the review: no read-modify-write of a file two processes share).
#[test]
fn an_entry_is_written_to_the_process_file_without_the_serial() {
    let (_dir, folder, ledger) = ledger();
    ledger.add("R58N123ABC", 3000);
    let text = std::fs::read_to_string(own_file(&folder)).expect("written");
    assert!(!text.contains("R58N123ABC"), "{text}");
    assert_eq!(
        ledger.entries(),
        [Entry {
            device: device_key("R58N123ABC"),
            port: 3000,
            pid: std::process::id(),
        }]
    );
    assert_eq!(device_key("R58N123ABC").len(), 16);
    assert_ne!(device_key("R58N123ABC"), device_key("R58N123ABD"));
}

#[test]
fn an_entry_is_added_once_and_removed_again_and_an_empty_file_goes() {
    let (_dir, folder, ledger) = ledger();
    ledger.add("S1", 3000);
    ledger.add("S1", 3000);
    ledger.add("S1", 8080);
    ledger.add("S2", 3000);
    assert_eq!(ledger.entries().len(), 3);
    ledger.remove("S1", 3000);
    let mut left: Vec<(String, u16)> = ledger
        .entries()
        .into_iter()
        .map(|entry| (entry.device, entry.port))
        .collect();
    left.sort();
    let mut expected = vec![(device_key("S1"), 8080), (device_key("S2"), 3000)];
    expected.sort();
    assert_eq!(left, expected);
    ledger.remove("S1", 8080);
    ledger.remove("S2", 3000);
    assert!(!own_file(&folder).exists());
}

/// Another process's file is read, never written: the two never race on one file.
#[test]
fn another_process_file_is_read_and_left_as_it_is() {
    let (_dir, folder, ledger) = ledger();
    let other = std::process::id().wrapping_add(7);
    other_file(&folder, other, &[("abcd", 4000)]);
    let before = std::fs::read(folder.join(format!("{other}.json"))).expect("there");
    ledger.add("S1", 3000);
    ledger.remove("S1", 3000);
    assert_eq!(
        std::fs::read(folder.join(format!("{other}.json"))).expect("there"),
        before
    );
    assert_eq!(ledger.entries(), [Entry::new("abcd", 4000, other)]);
}

/// The sweep (the engine's start): the files of processes that no longer run, and this
/// process's entries of a run before with the same pid, are dropped - their reverses are never
/// removed (whose they are now cannot be told, code review item 4). A running rBrowser's file
/// stays; a broken one of a gone process is moved aside, never overwritten.
#[test]
fn the_sweep_drops_what_no_running_rbrowser_noted_and_moves_a_broken_file_aside() {
    let (_dir, folder, ledger) = ledger();
    let me = std::process::id();
    let gone = me.wrapping_add(1);
    let running = me.wrapping_add(2);
    let broken = me.wrapping_add(3);
    other_file(&folder, gone, &[("aaaa", 3000)]);
    other_file(&folder, running, &[("bbbb", 4000)]);
    other_file(&folder, me, &[("cccc", 5000)]);
    std::fs::write(folder.join(format!("{broken}.json")), b"not json").expect("written");
    std::fs::write(folder.join("notes.txt"), b"left alone").expect("written");
    let mut dropped: Vec<u16> = ledger
        .sweep(|pid| pid == running)
        .iter()
        .map(|entry| entry.port)
        .collect();
    dropped.sort_unstable();
    assert_eq!(dropped, [3000, 5000]);
    assert_eq!(ledger.entries(), [Entry::new("bbbb", 4000, running)]);
    assert!(!folder.join(format!("{gone}.json")).exists());
    assert!(!own_file(&folder).exists());
    assert!(!folder.join(format!("{broken}.json")).exists());
    assert_eq!(
        std::fs::read(folder.join(format!("{broken}.json.broken"))).expect("moved aside"),
        b"not json"
    );
    assert!(folder.join("notes.txt").exists());
}

/// A running process's broken file is not this process's to touch.
#[test]
fn a_running_process_broken_file_is_left_alone() {
    let (_dir, folder, ledger) = ledger();
    let running = std::process::id().wrapping_add(4);
    std::fs::create_dir_all(&folder).expect("folder");
    std::fs::write(folder.join(format!("{running}.json")), b"{").expect("written");
    assert!(ledger.sweep(|pid| pid == running).is_empty());
    assert!(folder.join(format!("{running}.json")).exists());
    assert!(ledger.entries().is_empty());
}

/// The process's own entries are capped: past the cap a reverse is set but not noted.
#[test]
fn the_entries_are_capped() {
    let (_dir, _folder, ledger) = ledger();
    for port in 1..=u16::try_from(MAX_ENTRIES + 5).expect("small") {
        ledger.add("S1", port);
    }
    assert_eq!(ledger.entries().len(), MAX_ENTRIES);
}

#[test]
fn no_folder_is_an_empty_ledger() {
    let (_dir, _folder, ledger) = ledger();
    assert!(ledger.entries().is_empty());
    assert!(ledger.sweep(|_| false).is_empty());
    let none = Ledger::at(None);
    none.add("S1", 3000);
    assert!(none.entries().is_empty());
}
